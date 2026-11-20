// Rind Studio: see rind_gui.h. The interface lives on a thread of its own that runs JUCE's message loop; everything the
// app calls from other threads is queued to it, and the frames it draws come back through a mutex-guarded buffer.
//
// Licence: GPL-3.0-or-later, like Vital.

#include "rind_gui.h"

#include "full_interface.h"
#include "default_look_and_feel.h"
#include "synth_constants.h"
#include "sound_engine.h"
#include "synth_parameters.h"

#include <condition_variable>
#include <cstdio>
#include <cstring>
#include <deque>
#include <functional>
#include <memory>
#include <thread>
#include <unordered_map>

namespace juce {
extern std::function<void(int, int, const uint8*)> juce_headlessFrameCallback;
extern std::atomic<int> juce_headlessMaxFps;
extern int juce_headlessDisplayWidth, juce_headlessDisplayHeight;
void juce_headlessPointer(Component& root, int type, float x, float y, int modifierFlags, float pressure, int64 timeMs);
void juce_headlessWheel(Component& root, float x, float y, float dx, float dy, int modifierFlags, int64 timeMs);
}  // namespace juce

void RindSynth::renderBlock(float* left, float* right, int frames) {
  const ScopedLock lock(critical_section_);
  processModulationChanges();

  // The interface's own keyboard (and the mod/pitch wheels it draws) reach the engine as MIDI.
  MidiBuffer midi;
  processKeyboardEvents(midi, frames);
  if (!midi.isEmpty())
    processMidi(midi);

  engine_->process(frames);
  const vital::mono_float* out = reinterpret_cast<const vital::mono_float*>(engine_->output(0)->buffer);
  for (int i = 0; i < frames; ++i) {
    left[i] = out[vital::poly_float::kSize * i];
    right[i] = out[vital::poly_float::kSize * i + 1];
  }
  updateMemoryOutput(frames, engine_->output(0)->buffer);
}

void RindSynth::setValueNotifyHost(const std::string& name, vital::mono_float value) {
  const std::lock_guard<std::mutex> guard(changes_lock_);
  changes_.emplace_back(name, value);
}

void RindSynth::takeGuiChanges(std::vector<std::pair<std::string, float>>& out) {
  const std::lock_guard<std::mutex> guard(changes_lock_);
  out.insert(out.end(), changes_.begin(), changes_.end());
  changes_.clear();
}

namespace {

// Vital's window: its `FullInterface` in a component the (windowless) desktop shows.
class Editor : public Component, public SynthGuiInterface {
 public:
  explicit Editor(SynthBase* synth) : SynthGuiInterface(synth) {
    setLookAndFeel(DefaultLookAndFeel::instance());
    gui_->reset();
    gui_->setOscilloscopeMemory(synth->getOscilloscopeMemory());
    gui_->setAudioMemory(synth->getAudioMemory());
    gui_->animate(true);
    addAndMakeVisible(gui_.get());
  }
  ~Editor() override { setLookAndFeel(nullptr); }
  void resized() override { gui_->setBounds(getLocalBounds()); }
  FullInterface* full() { return gui_.get(); }
};

// One interface at a time (the card on screen), on its own thread.
struct Host {
  std::thread thread;
  std::mutex lock;
  std::condition_variable wake;
  std::deque<std::function<void()>> tasks;
  bool started = false;

  // Owned by the GUI thread.
  RindSynth* synth = nullptr;
  std::unique_ptr<Editor> editor;
  std::vector<float> shadow;  // the value of each parameter the interface was last told
  std::vector<std::string> names;
  std::vector<vital::Value*> values;
  std::atomic<int>* state_version = nullptr;
  int seen_version = 0;

  // Shared with the GL thread and the app.
  std::mutex frame_lock;
  std::vector<uint8_t> frame;
  int frame_w = 0, frame_h = 0;
  uint64_t frame_id = 0;
  uint64_t taken_id = 0;
  RindSynth* frame_owner = nullptr;
  std::atomic<bool> wants_text{false};
  std::string slots;  // refreshed by the GUI thread
  RindSynth* slots_owner = nullptr;
  juce::uint32 slots_at = 0;
};

Host& host() {
  static Host h;
  return h;
}

void post(std::function<void()> task) {
  Host& h = host();
  {
    const std::lock_guard<std::mutex> guard(h.lock);
    h.tasks.push_back(std::move(task));
  }
  h.wake.notify_one();
}

// Runs `task` on the GUI thread and waits for it.
void post_and_wait(std::function<void()> task) {
  std::mutex m;
  std::condition_variable cv;
  bool done = false;
  post([&] {
    task();
    const std::lock_guard<std::mutex> guard(m);
    done = true;
    cv.notify_all();
  });
  std::unique_lock<std::mutex> guard(m);
  cv.wait(guard, [&] { return done; });
}

void close_editor() {
  Host& h = host();
  if (h.editor) {
    if (h.synth)
      h.synth->setGui(nullptr);
    h.editor->removeFromDesktop();
    h.editor.reset();
  }
  h.synth = nullptr;
  const std::lock_guard<std::mutex> guard(h.frame_lock);
  h.frame_owner = nullptr;
  h.frame.clear();
  h.frame_w = h.frame_h = 0;
}

void snapshot_slots() {
  Host& h = host();
  std::string packed;
  vital::ModulationConnectionBank& bank = h.synth->getModulationBank();
  for (int i = 0; i < vital::kMaxModulationConnections; ++i) {
    vital::ModulationConnection* connection = bank.atIndex(i);
    packed += connection->source_name;
    packed.push_back('\0');
    packed += connection->destination_name;
    packed.push_back('\0');
  }
  const std::lock_guard<std::mutex> guard(h.frame_lock);
  h.slots = std::move(packed);
  h.slots_owner = h.synth;
}

void tick() {
  Host& h = host();
  if (!h.editor || !h.synth)
    return;
  Editor& editor = *h.editor;
  h.wants_text = dynamic_cast<juce::TextEditor*>(juce::Component::getCurrentlyFocusedComponent()) != nullptr;
  const juce::uint32 now_ms = juce::Time::getMillisecondCounter();
  if (now_ms - h.slots_at > 120) {
    h.slots_at = now_ms;
    snapshot_slots();
  }
  if (h.state_version && h.state_version->load() != h.seen_version) {
    // The app loaded a preset, changed a modulation slot or a wavetable: the whole interface follows.
    h.seen_version = h.state_version->load();
    editor.updateFullGui();
    editor.notifyFresh();
    for (size_t i = 0; i < h.values.size(); ++i)
      if (h.values[i])
        h.shadow[i] = h.values[i]->value();
    return;
  }
  // A value the app changed (automation, a preset's param): the knob moves.
  for (size_t i = 0; i < h.values.size(); ++i) {
    if (!h.values[i])
      continue;
    const float now = h.values[i]->value();
    if (now != h.shadow[i]) {
      h.shadow[i] = now;
      editor.updateGuiControl(h.names[i], now);
    }
  }
}

void loop() {
  juce::ScopedJuceInitialiser_GUI init;
  Host& h = host();
  juce::MessageManager::getInstance();  // this thread is the message thread from here on
  juce::juce_headlessFrameCallback = [](int w, int hgt, const juce::uint8* rgba) {
    Host& host_ = host();
    const std::lock_guard<std::mutex> guard(host_.frame_lock);
    host_.frame.assign(rgba, rgba + static_cast<size_t>(w) * static_cast<size_t>(hgt) * 4);
    host_.frame_w = w;
    host_.frame_h = hgt;
    ++host_.frame_id;
  };
  for (;;) {
    std::deque<std::function<void()>> todo;
    {
      std::unique_lock<std::mutex> guard(h.lock);
      todo.swap(h.tasks);
    }
    bool quit = false;
    for (auto& task : todo) {
      if (!task) {
        quit = true;
        break;
      }
      task();
    }
    if (quit)
      break;
    tick();
    juce::MessageManager::getInstance()->runDispatchLoopUntil(8);
  }
  close_editor();
}

void ensure_thread() {
  Host& h = host();
  const std::lock_guard<std::mutex> guard(h.lock);
  if (!h.started) {
    h.started = true;
    h.thread = std::thread(loop);
    h.thread.detach();
  }
}

}  // namespace

// ---- what bridge.cpp calls ----------------------------------------------------------------------------------------

extern "C++" {

// Opens the interface for `synth` at `width` x `height`; false when it cannot be made.
bool rind_gui_open(RindSynth* synth, std::atomic<int>* state_version, int width, int height) {
  ensure_thread();
  bool ok = false;
  post_and_wait([&] {
    close_editor();
    Host& h = host();
    try {
      h.synth = synth;
      h.editor = std::make_unique<Editor>(synth);
      synth->setGui(h.editor.get());
      h.editor->setSize(width, height);
      h.editor->addToDesktop(0);
      h.editor->setVisible(true);
      {
        const std::lock_guard<std::mutex> guard(h.frame_lock);
        h.frame_owner = synth;
        h.taken_id = h.frame_id;
      }
      h.state_version = state_version;
      h.seen_version = state_version->load();
      const int count = vital::Parameters::getNumParameters();
      h.names.assign(static_cast<size_t>(count), "");
      h.values.assign(static_cast<size_t>(count), nullptr);
      h.shadow.assign(static_cast<size_t>(count), 0.0f);
      auto& controls = synth->getControls();
      for (int i = 0; i < count; ++i) {
        const std::string name = vital::Parameters::getDetails(i)->name;
        auto it = controls.find(name);
        h.names[static_cast<size_t>(i)] = name;
        if (it != controls.end()) {
          h.values[static_cast<size_t>(i)] = it->second;
          h.shadow[static_cast<size_t>(i)] = it->second->value();
        }
      }
      ok = true;
    } catch (...) {
      close_editor();
    }
  });
  return ok;
}

void rind_gui_close(RindSynth* synth) {
  Host& h = host();
  if (!h.started)
    return;
  post_and_wait([&] {
    if (h.synth == synth)
      close_editor();
  });
}

bool rind_gui_is_open(RindSynth* synth) {
  Host& h = host();
  if (!h.started)
    return false;
  bool open = false;
  post_and_wait([&] { open = h.synth == synth && h.editor != nullptr; });
  return open;
}

void rind_gui_resize(RindSynth* synth, int width, int height) {
  Host& h = host();
  if (!h.started)
    return;
  post([=, &h] {
    if (h.synth == synth && h.editor && (h.editor->getWidth() != width || h.editor->getHeight() != height)) {
      h.editor->setSize(width, height);
    }
  });
}

void rind_gui_pointer(RindSynth* synth, int type, float x, float y, int buttons, float pressure, long long ms) {
  Host& h = host();
  if (!h.started)
    return;
  post([=, &h] {
    if (h.synth == synth && h.editor)
      juce::juce_headlessPointer(*h.editor, type, x, y, buttons, pressure, ms);
  });
}

void rind_gui_wheel(RindSynth* synth, float x, float y, float dx, float dy, long long ms) {
  Host& h = host();
  if (!h.started)
    return;
  post([=, &h] {
    if (h.synth == synth && h.editor)
      juce::juce_headlessWheel(*h.editor, x, y, dx, dy, 0, ms);
  });
}

// `special` is 0 for a character (`text_char`), else 1 + the order of honeydew_audio::vital::gui::Key.
void rind_gui_key(RindSynth* synth, int special, unsigned text_char, int modifiers) {
  Host& h = host();
  if (!h.started)
    return;
  post([=, &h] {
    if (h.synth != synth || !h.editor)
      return;
    auto* peer = h.editor->getPeer();
    if (peer == nullptr)
      return;
    static const int kSpecial[] = {0,
                                   juce::KeyPress::backspaceKey,
                                   juce::KeyPress::deleteKey,
                                   juce::KeyPress::returnKey,
                                   juce::KeyPress::escapeKey,
                                   juce::KeyPress::leftKey,
                                   juce::KeyPress::rightKey,
                                   juce::KeyPress::upKey,
                                   juce::KeyPress::downKey,
                                   juce::KeyPress::tabKey,
                                   juce::KeyPress::homeKey,
                                   juce::KeyPress::endKey};
    if (special > 0 && special < static_cast<int>(sizeof(kSpecial) / sizeof(kSpecial[0]))) {
      peer->handleKeyPress(juce::KeyPress(kSpecial[special], juce::ModifierKeys(modifiers), 0));
    } else if (text_char != 0) {
      peer->handleKeyPress(juce::KeyPress(static_cast<int>(text_char), juce::ModifierKeys(modifiers),
                                          static_cast<juce::juce_wchar>(text_char)));
    }
  });
}

bool rind_gui_wants_text(RindSynth* synth) {
  Host& h = host();
  return h.wants_text.load() && h.frame_owner == synth;
}

// The newest frame (RGBA, top row first) if it is newer than the last one taken: its id, else 0.
unsigned long long rind_gui_take_frame(RindSynth* synth, unsigned char* dst, unsigned long long capacity, int* width,
                                       int* height) {
  Host& h = host();
  const std::lock_guard<std::mutex> guard(h.frame_lock);
  if (h.frame_owner != synth || h.frame.empty() || h.frame_id == h.taken_id)
    return 0;
  const unsigned long long bytes = h.frame.size();
  if (dst == nullptr || bytes > capacity) {
    *width = h.frame_w;
    *height = h.frame_h;
    return 0;
  }
  std::memcpy(dst, h.frame.data(), bytes);
  *width = h.frame_w;
  *height = h.frame_h;
  h.taken_id = h.frame_id;
  return h.frame_id;
}

bool rind_gui_slots(RindSynth* synth, std::string& out) {
  Host& h = host();
  const std::lock_guard<std::mutex> guard(h.frame_lock);
  if (h.slots_owner != synth || h.slots.empty())
    return false;
  out = h.slots;
  return true;
}

void rind_gui_set_fps(int fps) {
  juce::juce_headlessMaxFps.store(fps < 1 ? 1 : (fps > 60 ? 60 : fps));
}

}  // extern "C++"
