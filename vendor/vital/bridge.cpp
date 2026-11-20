// A C face for Vital's SoundEngine, so Rust can play it. This is the part of Vital's `SynthBase` (src/common/synth_base.cpp)
// and `LoadSave` that matters to a host that owns its own audio device, MIDI and project format, written over the same
// engine calls: the engine is Vital's, unchanged. Everything here runs on the caller's thread (the audio thread).
//
// Licence: GPL-3.0-or-later, like Vital.

#include "JuceHeader.h"

#ifdef RIND_VITAL_UI
// The Android build: the engine is the one inside Vital's `SynthBase`, so that Vital's own interface (vendor/vital-ui)
// edits the synth the app plays. Everything below keeps its C face; only where the engine lives and how connections
// and presets are made differ (marked RIND_VITAL_UI).
#include "rind_gui.h"
#endif

#include "load_save.h"
#include "line_generator.h"
#include "sound_engine.h"
#include "synth_constants.h"
#include "synth_parameters.h"
#include "synth_types.h"
#include "modulation_connection_processor.h"
#include "tuning.h"
#include "wavetable_creator.h"
#include "sample_source.h"

#include <cmath>
#include <cstring>
#include <atomic>
#include <memory>
#include <mutex>
#include <string>
#include <unordered_map>
#include <vector>

namespace {

struct Core {
  vital::SoundEngine* engine = nullptr;
  WavetableCreator* creators[vital::kNumOscillators] = {};
#ifdef RIND_VITAL_UI
  std::unique_ptr<RindSynth> synth;
  std::atomic<int> state_version{0};  // bumped when the app changes more than a value, so an open interface follows
  unsigned long long key = 0;
  std::unordered_map<std::string, int> index_by_name;
#else
  std::unique_ptr<vital::SoundEngine> owned_engine;
  Tuning tuning;
  std::unique_ptr<WavetableCreator> owned_creators[vital::kNumOscillators];
#endif
  vital::control_map controls;
  std::vector<const vital::ValueDetails*> details;  // by index, with the control behind it
  std::vector<vital::Value*> values;
  std::vector<vital::ModulationConnection*> connected;
  vital::ModulationConnection* connections_by_slot[vital::kMaxModulationConnections] = {};
  vital::mono_float mod_wheel = 0.0f;
  int sample_rate = 44100;
};

vital::modulation_change make_change(Core& c, vital::ModulationConnection* connection) {
  vital::modulation_change change;
  change.source = c.engine->getModulationSource(connection->source_name);
  change.mono_destination = c.engine->getMonoModulationDestination(connection->destination_name);
  change.mono_modulation_switch = c.engine->getMonoModulationSwitch(connection->destination_name);
  change.destination_scale = vital::Parameters::getParameterRange(connection->destination_name);
  change.poly_modulation_switch = c.engine->getPolyModulationSwitch(connection->destination_name);
  change.poly_destination = c.engine->getPolyModulationDestination(connection->destination_name);
  change.modulation_processor = connection->modulation_processor.get();
  change.disconnecting = false;

  int num_audio_rate = 0;
  vital::ModulationConnectionBank& bank = c.engine->getModulationBank();
  for (int i = 0; i < vital::kMaxModulationConnections; ++i) {
    if (bank.atIndex(i)->source_name == connection->source_name &&
        bank.atIndex(i)->destination_name != connection->destination_name &&
        !bank.atIndex(i)->modulation_processor->isControlRate()) {
      num_audio_rate++;
    }
  }
  change.num_audio_rate = num_audio_rate;
  return change;
}

#ifdef RIND_VITAL_UI
void clear_modulations(Core& c) { c.synth->clearModulationsNow(); }

bool connect(Core& c, vital::ModulationConnection* connection) {
  c.synth->connectModulation(connection);
  return !connection->source_name.empty();
}

void init_engine(Core& c) {
  c.synth->initEngine();
  c.state_version++;
}
#else
void clear_modulations(Core& c) {
  for (vital::ModulationConnection* connection : c.connected) {
    vital::modulation_change change = make_change(c, connection);
    change.disconnecting = true;
    c.engine->disconnectModulation(change);
    connection->source_name = "";
    connection->destination_name = "";
  }
  c.connected.clear();
  vital::ModulationConnectionBank& bank = c.engine->getModulationBank();
  for (size_t i = 0; i < bank.numConnections(); ++i)
    bank.atIndex(static_cast<int>(i))->modulation_processor->lineMapGenerator()->initLinear();
  c.engine->disableUnnecessaryModSources();
}

bool connect(Core& c, vital::ModulationConnection* connection) {
  vital::modulation_change change = make_change(c, connection);
  if (change.poly_destination && change.poly_destination->router() == change.modulation_processor) {
    connection->destination_name = "";
    connection->source_name = "";
    return false;
  }
  for (auto* known : c.connected)
    if (known == connection)
      return true;
  c.connected.push_back(connection);
  c.engine->connectModulation(change);
  return true;
}

// `SynthBase::initEngine`
void init_engine(Core& c) {
  clear_modulations(c);
  for (int i = 0; i < vital::kNumOscillators; ++i)
    c.creators[i]->init();
  c.engine->getSample()->init();
  for (int i = 0; i < vital::kNumLfos; ++i)
    c.engine->getLfoSource(i)->initTriangle();
  for (auto& control : c.controls)
    control.second->set(vital::Parameters::getDetails(control.first).default_value);
  c.engine->checkOversampling();
}
#endif


// ---- wavetables ---------------------------------------------------------------------------------------------------
//
// Vital ships its wavetables as data files that are not part of its source. These are built with Vital's own wavetable
// classes (keyframes of single cycles, morphed by the engine), so every oscillator has a shelf to choose from.

constexpr int kWavetableCount = 12;
const char* const kWavetableNames[kWavetableCount] = {
    "Init Saw", "Basic Shapes", "Saw to Square", "Pulse Width", "Harmonic Sweep", "Odd Harmonics",
    "Vowels", "FM Bell", "Sine Fold", "Organ", "Sync Sweep", "Digital Steps",
};

typedef double (*CycleFunction)(double phase, double amount);

double saw_wave(double t) { return 1.0 - 2.0 * t; }
double square_wave(double t) { return t < 0.5 ? 1.0 : -1.0; }

double additive(double t, int harmonics, double rolloff, int step, int first) {
  double sum = 0.0;
  for (int h = first; h <= harmonics; h += step)
    sum += std::sin(2.0 * M_PI * h * t) / std::pow(h, rolloff);
  return sum;
}

// The cycle of `kind` at `amount` (0 .. 1 across the table).
double cycle(int kind, double t, double a) {
  switch (kind) {
    case 2: return saw_wave(t) * (1.0 - a) + square_wave(t) * a;
    case 3: return t < 0.5 - 0.45 * a ? 1.0 : -1.0;
    case 4: return additive(t, 1 + static_cast<int>(a * 47.0), 1.0, 1, 1);
    case 5: return additive(t, 1 + 2 * static_cast<int>(a * 23.0), 1.0, 2, 1);
    case 6: {
      // Five vowels (A E I O U): the harmonics of a 110 Hz voice, lifted around each vowel's first two formants.
      static const double kFormants[5][2] = {{800, 1200}, {400, 2000}, {300, 2500}, {500, 900}, {320, 800}};
      const double position = a * 4.0;
      const int low = std::min(3, static_cast<int>(position));
      const double mix = position - low;
      double sum = 0.0;
      for (int h = 1; h <= 60; ++h) {
        const double hz = 110.0 * h;
        double gain = 0.0;
        for (int f = 0; f < 2; ++f) {
          const double centre = kFormants[low][f] * (1.0 - mix) + kFormants[low + 1][f] * mix;
          const double z = (hz - centre) / (f == 0 ? 130.0 : 220.0);
          gain += std::exp(-z * z) * (f == 0 ? 1.0 : 0.6);
        }
        sum += gain * std::sin(2.0 * M_PI * h * t) / std::sqrt(static_cast<double>(h));
      }
      return sum;
    }
    case 7: return std::sin(2.0 * M_PI * t + a * 9.0 * std::sin(2.0 * M_PI * 3.0 * t));
    case 8: {
      double x = std::sin(2.0 * M_PI * t) * (1.0 + a * 7.0);
      while (x > 1.0 || x < -1.0)
        x = x > 1.0 ? 2.0 - x : -2.0 - x;
      return x;
    }
    case 9: {
      // Drawbars (8', 2⅔', 4', 2', 1⅓', 1' and up) as harmonics of the cycle, faded from soft to full.
      static const int kHarmonics[9] = {1, 3, 2, 4, 6, 8, 10, 12, 16};
      static const double kSoft[9] = {1.0, 0.6, 0.8, 0.4, 0.0, 0.1, 0.0, 0.0, 0.0};
      static const double kFull[9] = {1.0, 1.0, 1.0, 0.9, 0.7, 0.8, 0.6, 0.5, 0.6};
      double sum = 0.0;
      for (int i = 0; i < 9; ++i)
        sum += (kSoft[i] * (1.0 - a) + kFull[i] * a) * std::sin(2.0 * M_PI * kHarmonics[i] * t);
      return sum;
    }
    case 10: {
      const double ratio = 1.0 + a * 7.0;
      const double x = t * ratio;
      return saw_wave(x - std::floor(x));
    }
    case 11: {
      const double steps = 64.0 * std::pow(0.0625, a);  // 64 steps down to 4
      return std::floor(std::sin(2.0 * M_PI * t) * steps + 0.5) / steps;
    }
    default: return saw_wave(t);
  }
}

void build_wavetable(Core& c, int osc, int kind) {
  WavetableCreator* creator = c.creators[osc];
  if (kind <= 0 || kind >= kWavetableCount) {
    creator->init();
    return;
  }
  if (kind == 1) {
    creator->initPredefinedWaves();
    creator->setName(kWavetableNames[kind]);
    return;
  }
  creator->clear();
  WavetableGroup* group = new WavetableGroup();
  WaveSource* source = new WaveSource();
  const int frames = 16;
  for (int f = 0; f < frames; ++f) {
    const double amount = f / (frames - 1.0);
    source->insertNewKeyframe((vital::kNumOscillatorWaveFrames - 1) * f / (frames - 1));
    vital::WaveFrame* frame = source->getKeyframe(f)->wave_frame();
    for (int i = 0; i < vital::WaveFrame::kWaveformSize; ++i)
      frame->time_domain[i] = static_cast<float>(cycle(kind, static_cast<double>(i) / vital::WaveFrame::kWaveformSize, amount));
    frame->toFrequencyDomain();
  }
  group->addComponent(source);
  creator->addGroup(group);
  creator->setName(kWavetableNames[kind]);
  creator->render();
}

}  // namespace

extern "C" {

#ifdef RIND_VITAL_UI
std::mutex g_registry;
std::unordered_map<unsigned long long, Core*> g_cores;

Core* core_for_key(unsigned long long key) {
  auto it = g_cores.find(key);
  return it == g_cores.end() ? nullptr : it->second;
}
#endif

void* vital_new_keyed(double sample_rate, unsigned long long key) {
  auto* c = new Core();
#ifdef RIND_VITAL_UI
  c->synth = std::make_unique<RindSynth>();
  c->engine = c->synth->getEngine();
  for (int i = 0; i < vital::kNumOscillators; ++i)
    c->creators[i] = c->synth->getWavetableCreator(i);
  c->key = key;
#else
  c->owned_engine = std::make_unique<vital::SoundEngine>();
  c->engine = c->owned_engine.get();
  c->engine->setTuning(&c->tuning);
  for (int i = 0; i < vital::kNumOscillators; ++i) {
    vital::Wavetable* wavetable = c->engine->getWavetable(i);
    c->owned_creators[i] = std::make_unique<WavetableCreator>(wavetable);
    c->creators[i] = c->owned_creators[i].get();
    c->creators[i]->init();
  }
#endif
  c->controls = c->engine->getControls();
  const int count = vital::Parameters::getNumParameters();
  for (int i = 0; i < count; ++i) {
    const vital::ValueDetails* d = vital::Parameters::getDetails(i);
    auto it = c->controls.find(d->name);
    c->details.push_back(d);
    c->values.push_back(it == c->controls.end() ? nullptr : it->second);
  }
  c->sample_rate = static_cast<int>(sample_rate + 0.5);
  c->engine->setSampleRate(c->sample_rate);
  for (int i = 0; i < vital::kNumLfos; ++i)
    c->engine->getLfoSource(i)->initTriangle();
  for (auto& control : c->controls)
    control.second->set(vital::Parameters::getDetails(control.first).default_value);
  c->engine->checkOversampling();
  c->engine->updateAllModulationSwitches();
#ifdef RIND_VITAL_UI
  for (size_t i = 0; i < c->details.size(); ++i)
    c->index_by_name[c->details[i]->name] = static_cast<int>(i);
  if (key != 0) {
    const std::lock_guard<std::mutex> guard(g_registry);
    g_cores[key] = c;
  }
#endif
  return c;
}

void* vital_new(double sample_rate) {
  return vital_new_keyed(sample_rate, 0);
}

void vital_free(void* handle) {
  auto* c = static_cast<Core*>(handle);
#ifdef RIND_VITAL_UI
  {
    const std::lock_guard<std::mutex> guard(g_registry);
    auto it = g_cores.find(c->key);
    if (c->key != 0 && it != g_cores.end() && it->second == c)
      g_cores.erase(it);
    rind_gui_close(c->synth.get());
  }
#endif
  delete c;
}

void vital_set_sample_rate(void* handle, double sample_rate) {
  auto* c = static_cast<Core*>(handle);
  c->sample_rate = static_cast<int>(sample_rate + 0.5);
  c->engine->setSampleRate(c->sample_rate);
  c->engine->updateAllModulationSwitches();
}

void vital_set_bpm(void* handle, double bpm) {
  static_cast<Core*>(handle)->engine->setBpm(static_cast<vital::mono_float>(bpm));
}

void vital_init_patch(void* handle) {
  auto* c = static_cast<Core*>(handle);
  c->engine->allSoundsOff();
  init_engine(*c);
}


int vital_wavetable_count(void) {
  return kWavetableCount;
}

const char* vital_wavetable_name(int kind) {
  return kind >= 0 && kind < kWavetableCount ? kWavetableNames[kind] : nullptr;
}

// Builds wavetable `kind` into oscillator `osc` (0 .. 2). This renders every frame, so it is for a patch change, not a
// sweep: the host sends it when the choice changes.
void vital_set_wavetable(void* handle, int osc, int kind) {
  if (osc < 0 || osc >= vital::kNumOscillators)
    return;
  build_wavetable(*static_cast<Core*>(handle), osc, kind);
#ifdef RIND_VITAL_UI
  static_cast<Core*>(handle)->state_version++;
#endif
}

// ---- parameters, in the order of Vital's own list -------------------------------------------------------------

int vital_param_count(void) {
  return vital::Parameters::getNumParameters();
}

const char* vital_param_name(int index) {
  return vital::Parameters::getDetails(index)->name.c_str();
}

const char* vital_param_display_name(int index) {
  return vital::Parameters::getDetails(index)->display_name.c_str();
}

const char* vital_param_units(int index) {
  return vital::Parameters::getDetails(index)->display_units.c_str();
}

// min, max, default, scale (0 indexed .. 6 exponential), post_offset, display_multiply, display_invert, version_added
void vital_param_info(int index, double* out) {
  const vital::ValueDetails* d = vital::Parameters::getDetails(index);
  out[0] = d->min;
  out[1] = d->max;
  out[2] = d->default_value;
  out[3] = d->value_scale;
  out[4] = d->post_offset;
  out[5] = d->display_multiply;
  out[6] = d->display_invert ? 1.0 : 0.0;
  out[7] = d->version_added;
}

// The names of a menu parameter's items, when it has any (`kIndexed` with a string lookup): `item` 0 .. max (the items below a menu's minimum are there but not offered).
const char* vital_param_item(int index, int item) {
  const vital::ValueDetails* d = vital::Parameters::getDetails(index);
  if (d->string_lookup == nullptr || item < 0 || item > static_cast<int>(d->max))
    return nullptr;
  return d->string_lookup[item].c_str();
}

void vital_set_param(void* handle, int index, double value) {
  auto* c = static_cast<Core*>(handle);
  if (index < 0 || index >= static_cast<int>(c->values.size()) || c->values[index] == nullptr)
    return;
  const vital::ValueDetails* d = c->details[index];
  c->values[index]->set(static_cast<vital::mono_float>(std::min<double>(std::max<double>(value, d->min), d->max)));
}

double vital_get_param(void* handle, int index) {
  auto* c = static_cast<Core*>(handle);
  if (index < 0 || index >= static_cast<int>(c->values.size()) || c->values[index] == nullptr)
    return 0.0;
  return c->values[index]->value();
}

// ---- notes ------------------------------------------------------------------------------------------------------

void vital_note_on(void* handle, int note, float velocity, int channel) {
  static_cast<Core*>(handle)->engine->noteOn(note, velocity, 0, channel);
}

void vital_note_off(void* handle, int note, float lift, int channel) {
  static_cast<Core*>(handle)->engine->noteOff(note, lift, 0, channel);
}

void vital_all_notes_off(void* handle) {
  static_cast<Core*>(handle)->engine->allNotesOff(0);
}

void vital_all_sound_off(void* handle) {
  static_cast<Core*>(handle)->engine->allSoundsOff();
}

void vital_pitch_wheel(void* handle, float value) {  // -1 .. 1
  static_cast<Core*>(handle)->engine->setZonedPitchWheel(value, 0, vital::kNumMidiChannels - 1);
}

void vital_mod_wheel(void* handle, float value) {  // 0 .. 1
  static_cast<Core*>(handle)->engine->setModWheelAllChannels(value);
}

void vital_sustain(void* handle, int on) {
  auto* e = static_cast<Core*>(handle)->engine;
  if (on)
    e->sustainOnRange(0, vital::kNumMidiChannels - 1);
  else
    e->sustainOffRange(0, 0, vital::kNumMidiChannels - 1);
}

void vital_aftertouch(void* handle, float value) {
  static_cast<Core*>(handle)->engine->setChannelRangeAftertouch(0, vital::kNumMidiChannels - 1, value, 0);
}

int vital_active_voices(void* handle) {
  return static_cast<Core*>(handle)->engine->getNumActiveVoices();
}

// ---- modulation ---------------------------------------------------------------------------------------------------

// Connects `source` to `destination` (Vital's own names, e.g. "lfo_1" and "filter_1_cutoff"). The slot's index, or -1.
int vital_mod_connect(void* handle, const char* source, const char* destination) {
  auto* c = static_cast<Core*>(handle);
  if (c->engine->getModulationSource(source) == nullptr || c->engine->getMonoModulationDestination(destination) == nullptr)
    return -1;
  vital::ModulationConnectionBank& bank = c->engine->getModulationBank();
  for (int i = 0; i < vital::kMaxModulationConnections; ++i) {
    vital::ModulationConnection* connection = bank.atIndex(i);
    if (connection->source_name == source && connection->destination_name == destination)
      return i;
  }
  for (int i = 0; i < vital::kMaxModulationConnections; ++i) {
    vital::ModulationConnection* connection = bank.atIndex(i);
    if (connection->source_name.empty() && connection->destination_name.empty()) {
      connection->source_name = source;
      connection->destination_name = destination;
      return connect(*c, connection) ? i : -1;
    }
  }
  return -1;
}

// The names of the modulation sources (`lfo_1`, `env_2`, `velocity` …) and of the destinations (every parameter a
// connection can reach), in the engine's own (sorted) order.
int vital_mod_source_count(void* handle) {
  return static_cast<int>(static_cast<Core*>(handle)->engine->getModulationSources().size());
}

const char* vital_mod_source_name(void* handle, int index) {
  auto& sources = static_cast<Core*>(handle)->engine->getModulationSources();
  if (index < 0 || index >= static_cast<int>(sources.size()))
    return nullptr;
  auto it = sources.begin();
  std::advance(it, index);
  return it->first.c_str();
}

int vital_mod_destination_count(void* handle) {
  return static_cast<int>(static_cast<Core*>(handle)->engine->getMonoModulationDestinations().size());
}

const char* vital_mod_destination_name(void* handle, int index) {
  auto& destinations = static_cast<Core*>(handle)->engine->getMonoModulationDestinations();
  if (index < 0 || index >= static_cast<int>(destinations.size()))
    return nullptr;
  auto it = destinations.begin();
  std::advance(it, index);
  return it->first.c_str();
}

// Puts slot `slot` (0 .. 63) on `source` -> `destination`, first disconnecting what it held; an empty or unknown name
// leaves the slot free. Its amount, power, bipolar, stereo and bypass are the `modulation_<slot + 1>_*` parameters.
// 1 when the slot is connected afterwards.
int vital_mod_set_slot(void* handle, int slot, const char* source, const char* destination) {
  auto* c = static_cast<Core*>(handle);
  if (slot < 0 || slot >= vital::kMaxModulationConnections)
    return 0;
  vital::ModulationConnection* connection = c->engine->getModulationBank().atIndex(slot);
#ifdef RIND_VITAL_UI
  const bool unchanged = connection->source_name == (source ? source : "") &&
                         connection->destination_name == (destination ? destination : "");
  if (unchanged)
    return connection->source_name.empty() ? 0 : 1;
  c->synth->disconnectModulation(connection);
  connection->source_name = "";
  connection->destination_name = "";
  c->state_version++;
  if (source == nullptr || destination == nullptr || source[0] == 0 || destination[0] == 0)
    return 0;
  if (c->engine->getModulationSource(source) == nullptr ||
      c->engine->getMonoModulationDestination(destination) == nullptr) {
    return 0;
  }
  connection->source_name = source;
  connection->destination_name = destination;
  return connect(*c, connection) ? 1 : 0;
#endif
  for (size_t i = 0; i < c->connected.size(); ++i) {
    if (c->connected[i] == connection) {
      vital::modulation_change change = make_change(*c, connection);
      change.disconnecting = true;
      c->engine->disconnectModulation(change);
      c->connected.erase(c->connected.begin() + static_cast<std::ptrdiff_t>(i));
      break;
    }
  }
  connection->source_name = "";
  connection->destination_name = "";
  if (source == nullptr || destination == nullptr || source[0] == 0 || destination[0] == 0)
    return 0;
  if (c->engine->getModulationSource(source) == nullptr ||
      c->engine->getMonoModulationDestination(destination) == nullptr) {
    return 0;
  }
  connection->source_name = source;
  connection->destination_name = destination;
  return connect(*c, connection) ? 1 : 0;
}

void vital_mod_clear(void* handle) {
  clear_modulations(*static_cast<Core*>(handle));
#ifdef RIND_VITAL_UI
  static_cast<Core*>(handle)->state_version++;
#endif
}

// ---- presets ------------------------------------------------------------------------------------------------------

// A Vital preset (`.vital`, JSON) of this version or any older one that already used the same state layout (1.0.0 and up).
// 0 when it loaded, 1 when it is not valid JSON, 2 when it was made by a newer Vital.
int vital_load_json(void* handle, const char* text, size_t len) {
  auto* c = static_cast<Core*>(handle);
  json data = json::parse(text, text + len, nullptr, false);
  if (data.is_discarded() || !data.is_object() || !data.count("settings") || !data.count("synth_version"))
    return 1;
  std::string version = data["synth_version"];
  if (LoadSave::compareVersionStrings(version, "1.0.0") < 0)
    return 1;
  // Vital's feature version (its minor part) must not be newer than ours.
  String feature = String(version).upToLastOccurrenceOf(".", false, true);
  if (LoadSave::compareVersionStrings(feature, String(ProjectInfo::versionString).upToLastOccurrenceOf(".", false, true)) > 0)
    return 2;

#ifdef RIND_VITAL_UI
  try {
    if (!c->synth->loadState(data))
      return 2;
  } catch (const std::exception&) {
    return 1;
  }
  c->state_version++;
  return 0;
#endif
  c->engine->allSoundsOff();
  const json& settings = data["settings"];
  clear_modulations(*c);
  for (auto& control : c->controls) {
    const std::string& name = control.first;
    if (settings.count(name))
      control.second->set(static_cast<vital::mono_float>(settings[name]));
    else
      control.second->set(vital::Parameters::getDetails(name).default_value);
  }
  if (settings.count("modulations")) {
    vital::ModulationConnectionBank& bank = c->engine->getModulationBank();
    int index = 0;
    for (const json& modulation : settings["modulations"]) {
      if (index >= vital::kMaxModulationConnections)
        break;
      std::string source = modulation["source"];
      std::string destination = modulation["destination"];
      vital::ModulationConnection* connection = bank.atIndex(index);
      index++;
      if (c->engine->getModulationSource(source) == nullptr ||
          c->engine->getMonoModulationDestination(destination) == nullptr) {
        continue;
      }
      if (source.length() && destination.length()) {
        connection->source_name = source;
        connection->destination_name = destination;
        connect(*c, connection);
      }
      if (modulation.count("line_mapping"))
        connection->modulation_processor->lineMapGenerator()->jsonToState(modulation["line_mapping"]);
      else
        connection->modulation_processor->lineMapGenerator()->initLinear();
    }
  }
  if (settings.count("sample"))
    c->engine->getSample()->jsonToState(settings["sample"]);
  if (settings.count("wavetables")) {
    int i = 0;
    for (const json& wavetable : settings["wavetables"]) {
      if (i >= vital::kNumOscillators)
        break;
      c->creators[i]->jsonToState(wavetable);
      c->creators[i]->render();
      i++;
    }
  }
  if (settings.count("lfos")) {
    int i = 0;
    for (const json& lfo : settings["lfos"]) {
      if (i >= vital::kNumLfos)
        break;
      LineGenerator* source = c->engine->getLfoSource(i);
      source->jsonToState(lfo);
      source->render();
      i++;
    }
  }
  c->engine->checkOversampling();
  return 0;
}

// ---- audio --------------------------------------------------------------------------------------------------------

// Renders `frames` stereo frames (any number) into `left` and `right`.
void vital_render(void* handle, float* left, float* right, unsigned frames) {
  auto* c = static_cast<Core*>(handle);
  unsigned done = 0;
  while (done < frames) {
    const int n = static_cast<int>(std::min<unsigned>(frames - done, vital::kMaxBufferSize));
#ifdef RIND_VITAL_UI
    c->synth->renderBlock(left + done, right + done, n);
    done += static_cast<unsigned>(n);
    continue;
#endif
    c->engine->process(n);
    const vital::mono_float* out = reinterpret_cast<const vital::mono_float*>(c->engine->output(0)->buffer);
    for (int i = 0; i < n; ++i) {
      left[done + i] = out[vital::poly_float::kSize * i];
      right[done + i] = out[vital::poly_float::kSize * i + 1];
    }
    done += static_cast<unsigned>(n);
  }
}

#ifdef RIND_VITAL_UI
// ---- Vital's own interface ------------------------------------------------------------------------------------------
//
// The synth of the module `key` (the app's channel and instance ids) is found in the registry the instruments fill: the
// engine is rebuilt now and then, and the interface follows to whichever synth holds the key.

#define WITH_CORE(key, fallback)                           \
  const std::lock_guard<std::mutex> guard(g_registry);     \
  Core* c = core_for_key(key);                             \
  if (c == nullptr)                                        \
    return fallback;

// 1 when the interface is open for `key` at `width` x `height` pixels.
int vital_gui_open(unsigned long long key, int width, int height) {
  WITH_CORE(key, 0)
  return rind_gui_open(c->synth.get(), &c->state_version, width, height) ? 1 : 0;
}

void vital_gui_close(unsigned long long key) {
  WITH_CORE(key, )
  rind_gui_close(c->synth.get());
}

int vital_gui_is_open(unsigned long long key) {
  WITH_CORE(key, 0)
  return rind_gui_is_open(c->synth.get()) ? 1 : 0;
}

void vital_gui_resize(unsigned long long key, int width, int height) {
  WITH_CORE(key, )
  rind_gui_resize(c->synth.get(), width, height);
}

// A touch: `buttons` is 16 while a finger is down (JUCE's left button flag), else 0.
void vital_gui_pointer(unsigned long long key, int type, float x, float y, int buttons, float pressure, long long ms) {
  WITH_CORE(key, )
  rind_gui_pointer(c->synth.get(), type, x, y, buttons, pressure, ms);
}

void vital_gui_wheel(unsigned long long key, float x, float y, float dx, float dy, long long ms) {
  WITH_CORE(key, )
  rind_gui_wheel(c->synth.get(), x, y, dx, dy, ms);
}

void vital_gui_key(unsigned long long key, int special, unsigned text_char, int modifiers) {
  WITH_CORE(key, )
  rind_gui_key(c->synth.get(), special, text_char, modifiers);
}

int vital_gui_wants_text(unsigned long long key) {
  WITH_CORE(key, 0)
  return rind_gui_wants_text(c->synth.get()) ? 1 : 0;
}

// Copies the newest frame (RGBA) into `dst` if there is a newer one than the last taken: its id, else 0.
unsigned long long vital_gui_take_frame(unsigned long long key, unsigned char* dst, unsigned long long capacity,
                                        int* width, int* height) {
  WITH_CORE(key, 0)
  return rind_gui_take_frame(c->synth.get(), dst, capacity, width, height);
}

// What the interface changed since the last call, as Vital's parameter numbers and values.
int vital_gui_changes(unsigned long long key, int* indices, float* values, int capacity) {
  WITH_CORE(key, 0)
  std::vector<std::pair<std::string, float>> changes;
  c->synth->takeGuiChanges(changes);
  int n = 0;
  for (auto& change : changes) {
    auto it = c->index_by_name.find(change.first);
    if (it == c->index_by_name.end() || n >= capacity)
      continue;
    indices[n] = it->second;
    values[n] = change.second;
    ++n;
  }
  return n;
}

// The modulation slots as the interface has them: `source\0destination\0` for each of the 64; the bytes written, or 0.
int vital_gui_slots(unsigned long long key, char* out, int capacity) {
  WITH_CORE(key, 0)
  std::string packed;
  if (!rind_gui_slots(c->synth.get(), packed) || static_cast<int>(packed.size()) > capacity)
    return 0;
  std::memcpy(out, packed.data(), packed.size());
  return static_cast<int>(packed.size());
}

void vital_gui_set_fps(int fps) {
  rind_gui_set_fps(fps);
}

// The folder Vital keeps its presets, wavetables and settings in (its `~/.config/vital`, `~/Documents/Vital`).
void vital_set_home(const char* path) {
  setenv("HOME", path, 1);
}
#endif

}  // extern "C"
