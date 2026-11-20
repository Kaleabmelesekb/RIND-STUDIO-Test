// Rind Studio: Vital's real engine and its original interface (JUCE, OpenGL) inside the app.
//
// `RindSynth` is Vital's `SynthBase` with the host's audio callback in place of a plugin's: the engine, the controls, the
// modulation bank and the preset code are Vital's own, so the interface (which is Vital's `FullInterface`, unchanged)
// edits the very synth the app plays. The interface draws windowless (EGL + OpenGL ES 3, see
// juce_opengl/native/juce_OpenGL_headless.h) and the app shows the picture and sends it the touches.
//
// Licence: GPL-3.0-or-later, like Vital.
#pragma once

#include "JuceHeader.h"
#include "synth_base.h"
#include "synth_gui_interface.h"

#include <atomic>
#include <mutex>
#include <string>
#include <utility>
#include <vector>

class RindSynth : public SynthBase {
 public:
  RindSynth() = default;

  const CriticalSection& getCriticalSection() override { return critical_section_; }
  void pauseProcessing(bool pause) override {
    if (pause)
      critical_section_.enter();
    else
      critical_section_.exit();
  }

  /** Renders `frames` (at most vital::kMaxBufferSize) stereo frames: the host's audio thread. */
  void renderBlock(float* left, float* right, int frames);

  /** Loads a preset's state (the audio thread, or any thread that is not the message thread). */
  bool loadState(const json& state) { return loadFromJson(state); }
  void clearModulationsNow() { clearModulations(); }

  /** The interface this synth is shown by, or null. Set and cleared on the message thread. */
  void setGui(SynthGuiInterface* gui) { gui_ = gui; }

  /** What the interface changed since the last call: Vital's parameter names and their new values. */
  void takeGuiChanges(std::vector<std::pair<std::string, float>>& out);

  void setValueNotifyHost(const std::string& name, vital::mono_float value) override;

 protected:
  SynthGuiInterface* getGuiInterface() override { return gui_.load(); }

 private:
  CriticalSection critical_section_;
  std::atomic<SynthGuiInterface*> gui_{nullptr};
  std::mutex changes_lock_;
  std::vector<std::pair<std::string, float>> changes_;
};

// The interface's side, as the bridge calls it (rind_gui.cpp).
bool rind_gui_open(RindSynth* synth, std::atomic<int>* state_version, int width, int height);
void rind_gui_close(RindSynth* synth);
bool rind_gui_is_open(RindSynth* synth);
void rind_gui_resize(RindSynth* synth, int width, int height);
void rind_gui_pointer(RindSynth* synth, int type, float x, float y, int buttons, float pressure, long long ms);
void rind_gui_wheel(RindSynth* synth, float x, float y, float dx, float dy, long long ms);
void rind_gui_key(RindSynth* synth, int special, unsigned text_char, int modifiers);
bool rind_gui_wants_text(RindSynth* synth);
unsigned long long rind_gui_take_frame(RindSynth* synth, unsigned char* dst, unsigned long long capacity, int* width,
                                       int* height);
void rind_gui_set_fps(int fps);
/** The modulation slots as the interface last saw them: 64 times `source\0destination\0`. */
bool rind_gui_slots(RindSynth* synth, std::string& out);
