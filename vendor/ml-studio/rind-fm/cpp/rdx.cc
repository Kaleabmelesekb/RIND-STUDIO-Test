// SPDX-License-Identifier: GPL-3.0-or-later
// Honeydew's glue around Dexed's engine (the msfa sources in this directory, which Dexed took from Google's
// music-synthesizer-for-android and went on to extend): sixteen voices with Dexed's voice stealing, mono mode and
// portamento, its controllers (pitch wheel, mod wheel, foot, breath, aftertouch), its three engines and its filter,
// behind a C interface. Voice handling follows Dexed's `PluginProcessor.cpp`, the filter its `PluginFx.cpp`.
#include <math.h>
#include <memory>
#include <new>
#include <stdint.h>
#include <string.h>

#include "synth.h"
#include "aligned_buf.h"
#include "controllers.h"
#include "dx7note.h"
#include "EngineMkI.h"
#include "EngineOpl.h"
#include "env.h"
#include "exp2.h"
#include "fm_core.h"
#include "freqlut.h"
#include "lfo.h"
#include "pitchenv.h"
#include "porta.h"
#include "sin.h"
#include "tuning.h"

namespace {

const int kVoices = 16;
// A released voice under this (of 2^28, full scale) for this many blocks is over: -96 dB.
const int32_t kSilent = 4096;
const int kQuietBlocks = 4;

const double kPi = 3.14159265358979323846;

// ---- Dexed's filter: a four pole low pass (the OBXd one) with a DC blocker in front -------------------------------

inline float tptpc(float& state, float inp, float cutoff) {
  double v = (inp - state) * cutoff / (1 + cutoff);
  double res = v + state;
  state = res + v;
  return res;
}

inline float tptlpupw(float& state, float inp, float cutoff, float sr_inv) {
  cutoff = (cutoff * sr_inv) * kPi;
  double v = (inp - state) * cutoff / (1 + cutoff);
  double res = v + state;
  state = res + v;
  return res;
}

float linsc(float param, float min, float max) { return param * (max - min) + min; }

float logsc(float param, float min, float max, float rolloff = 19.0f) {
  return ((expf(param * logf(rolloff + 1)) - 1.0f) / rolloff) * (max - min) + min;
}

struct Fx {
  float ui_cutoff = 1, ui_reso = 0, ui_gain = 1;
  float s1, s2, s3, s4, c, d, r24;
  float rate, rate_inv, rcor24, rcor24_inv, bright;
  float p_cutoff, p_reso, r_cutoff, r_reso;
  float smooth, s_cutoff, s_reso;
  float dc_r, dc_id, dc_od;
  bool bypassed;

  void init(double sr) {
    s1 = s2 = s3 = s4 = c = d = 0;
    r24 = 0;
    rate = (float)sr;
    rate_inv = 1 / rate;
    float rcrate = sqrt((44000 / rate));
    rcor24 = (970.0 / 44000) * rcrate;
    rcor24_inv = 1 / rcor24;
    bright = tan((rate * 0.5f - 10) * kPi * rate_inv);
    p_cutoff = -1;
    p_reso = -1;
    r_cutoff = r_reso = 0;
    // ~10 ms one-pole smoothing on the filter coefficients
    smooth = expf(-1.0f / (0.01f * rate));
    s_cutoff = tan(19000.0f * rate_inv * kPi);
    s_reso = 0;
    bypassed = true;
    dc_r = 1.0 - (126.0 / sr);
    dc_id = 0;
    dc_od = 0;
  }

  inline float nr24(float sample, float g, float lpc) {
    float ml = 1 / (1 + g);
    float s = (lpc * (lpc * (lpc * s1 + s2) + s3) + s4) * ml;
    float gg = lpc * lpc * lpc * lpc;
    float y = (sample - r24 * s) / (1 + r24 * gg);
    return y + 1e-8;
  }

  void process(float* work, int n) {
    if (n <= 0) return;
    // very basic DC filter
    float t_fd = work[0];
    work[0] = work[0] - dc_id + dc_r * dc_od;
    dc_id = t_fd;
    for (int i = 1; i < n; i++) {
      t_fd = work[i];
      work[i] = work[i] - dc_id + dc_r * work[i - 1];
      dc_id = t_fd;
    }
    dc_od = work[n - 1];

    if (ui_gain != 1) {
      for (int i = 0; i < n; i++) work[i] *= ui_gain;
    }

    if (ui_cutoff != p_cutoff || ui_reso != p_reso) {
      r_reso = (0.991 - logsc(1 - ui_reso, 0, 0.991));
      float cutoff_norm = logsc(ui_cutoff, 60, 19000);
      r_cutoff = (float)tan(cutoff_norm * rate_inv * kPi);
      p_cutoff = ui_cutoff;
      p_reso = ui_reso;
    }

    // Not applied while the cutoff is at maximum, once the smoothed coefficients have settled; the changes are
    // crossfaded so that switching the filter in and out of the path does not click.
    bool want_bypass = ui_cutoff == 1;
    if (want_bypass && bypassed) return;

    float fade = 1;
    float fade_step = 0;
    if (bypassed) {
      bypassed = false;
      fade = 0;
      fade_step = 1.0f / n;
    } else if (want_bypass && fabsf(s_cutoff - r_cutoff) < 0.01f * r_cutoff) {
      fade_step = -1.0f / n;
      bypassed = true;
    }

    for (int i = 0; i < n; i++) {
      s_cutoff = r_cutoff + smooth * (s_cutoff - r_cutoff);
      s_reso = r_reso + smooth * (s_reso - r_reso);
      r24 = 3.5 * s_reso;

      float g = s_cutoff;
      float lpc = g / (1 + g);

      float dry = work[i];
      float s = dry;
      s = s - 0.45 * tptlpupw(c, s, 15, rate_inv);
      s = tptpc(d, s, bright);

      float y0 = nr24(s, g, lpc);

      // first low pass in the cascade
      double v = (y0 - s1) * lpc;
      double res = v + s1;
      s1 = res + v;

      // damping
      s1 = atan(s1 * rcor24) * rcor24_inv;
      float y1 = res;
      float y2 = tptpc(s2, y1, g);
      float y3 = tptpc(s3, y2, g);
      float y4 = tptpc(s4, y3, g);

      // half volume compensation
      float out = y4 * (1 + r24 * 0.45);
      work[i] = out * fade + dry * (1 - fade);
      fade += fade_step;
    }
  }
};

// ---- Voices -------------------------------------------------------------------------------------------------------

struct Voice {
  Dx7Note* note;
  int midi;
  int velocity;
  bool keydown;
  bool sustained;
  bool live;
  int quiet;
  int32_t seq;
};

struct Rdx {
  Voice voice[kVoices];
  Lfo lfo;
  Controllers controllers;
  FmCore core_modern;
  EngineMkI core_mk1;
  EngineOpl core_opl;
  Fx fx;
  std::shared_ptr<TuningState> tuning;
  MTSClient mts;
  uint8_t patch[156];
  bool sustain;
  bool mono;
  int engine;
  int current;
  int last_active;
  int32_t next_seq;
  // The last block rendered, the part of it not handed out yet at `pos`.
  float block[N];
  int pos;
};

double g_rate = 0;

void set_engine(Rdx* r, int engine) {
  r->engine = engine;
  switch (engine) {
    case 1:
      r->controllers.core = &r->core_mk1;
      break;
    case 2:
      r->controllers.core = &r->core_opl;
      break;
    default:
      r->controllers.core = &r->core_modern;
      break;
  }
}

int shift_of(const Rdx* r) { return (int)r->patch[144] - 24; }

int clamp_note(int n) { return n < 0 ? 0 : (n > 127 ? 127 : n); }

void panic(Rdx* r) {
  for (int i = 0; i < kVoices; i++) {
    r->voice[i].midi = -1;
    r->voice[i].keydown = false;
    r->voice[i].sustained = false;
    r->voice[i].live = false;
    r->voice[i].note->oscSync();
  }
  r->pos = N;
}

}  // namespace

extern "C" {

// The engine's tables depend on the sample rate and are shared; they are made once per rate.
void rdx_init(double rate) {
  if (rate == g_rate) return;
  Freqlut::init(rate);
  Exp2::init();
  Tanh::init();
  Sin::init();
  Lfo::init(rate);
  PitchEnv::init(rate);
  Env::init_sr(rate);
  Porta::init_sr(rate);
  g_rate = rate;
}

void* rdx_new() {
  if (g_rate == 0) rdx_init(44100.0);
  Rdx* r = new (std::nothrow) Rdx();
  if (!r) return 0;
  r->tuning = createStandardTuning();
  for (int i = 0; i < kVoices; i++) {
    Voice& v = r->voice[i];
    v.note = new Dx7Note(r->tuning, &r->mts);
    v.midi = -1;
    v.velocity = 0;
    v.keydown = v.sustained = v.live = false;
    v.quiet = 0;
    v.seq = -1;
  }
  memset(r->patch, 0, sizeof r->patch);
  r->patch[155] = 0x3f;
  r->fx.init(g_rate);
  r->controllers.pitch_cc = 0x2000;
  r->controllers.pitch_range_up = 3;
  r->controllers.pitch_range_dn = 3;
  r->controllers.pitch_step = 0;
  r->controllers.masterTune = 0;
  r->controllers.aftertouch_cc = 0;
  r->controllers.breath_cc = 0;
  r->controllers.foot_cc = 0;
  r->controllers.modwheel_cc = 0;
  r->controllers.portamento_enable_cc = false;
  r->controllers.portamento_cc = 0;
  r->controllers.portamento_gliss_cc = false;
  r->controllers.mpeEnabled = false;
  r->controllers.refresh();
  r->sustain = false;
  r->mono = false;
  r->current = 0;
  r->last_active = 0;
  r->next_seq = 0;
  r->pos = N;
  set_engine(r, 0);
  r->lfo.reset(r->patch + 137);
  return r;
}

void rdx_free(void* handle) {
  Rdx* r = static_cast<Rdx*>(handle);
  if (!r) return;
  for (int i = 0; i < kVoices; i++) delete r->voice[i].note;
  delete r;
}

// A voice as a sysex bank stores it (128 bytes) to the form the engine plays (156 bytes).
void rdx_unpack(const char* bulk, char* patch) {
  for (int op = 0; op < 6; op++) {
    // eg rate and level, breakpoint, depths, curves
    memcpy(patch + op * 21, bulk + op * 17, 11);
    char curves = bulk[op * 17 + 11];
    patch[op * 21 + 11] = curves & 3;
    patch[op * 21 + 12] = (curves >> 2) & 3;
    char detune_rs = bulk[op * 17 + 12];
    patch[op * 21 + 13] = detune_rs & 7;
    patch[op * 21 + 20] = detune_rs >> 3;
    char kvs_ams = bulk[op * 17 + 13];
    patch[op * 21 + 14] = kvs_ams & 3;
    patch[op * 21 + 15] = kvs_ams >> 2;
    patch[op * 21 + 16] = bulk[op * 17 + 14];  // output level
    char fcoarse_mode = bulk[op * 17 + 15];
    patch[op * 21 + 17] = fcoarse_mode & 1;
    patch[op * 21 + 18] = fcoarse_mode >> 1;
    patch[op * 21 + 19] = bulk[op * 17 + 16];  // fine frequency
  }
  memcpy(patch + 126, bulk + 102, 9);  // pitch envelope, algorithm
  char oks_fb = bulk[111];
  patch[135] = oks_fb & 7;
  patch[136] = oks_fb >> 3;
  memcpy(patch + 137, bulk + 112, 4);  // LFO
  char lpms_lfw_lks = bulk[116];
  patch[141] = lpms_lfw_lks & 1;
  patch[142] = (lpms_lfw_lks >> 1) & 7;
  patch[143] = lpms_lfw_lks >> 4;
  memcpy(patch + 144, bulk + 117, 11);  // transpose, name
  patch[155] = 0x3f;                    // operators on
}

// The patch the next notes play and the notes sounding now follow; the LFO takes its settings. `unpacked[155]` is
// the operator switches, bit 5 for operator 1.
void rdx_set_patch(void* handle, const char* unpacked) {
  Rdx* r = static_cast<Rdx*>(handle);
  if (memcmp(r->patch, unpacked, 156) == 0) return;
  memcpy(r->patch, unpacked, 156);
  for (int i = 0; i < kVoices; i++) {
    Voice& v = r->voice[i];
    if (v.live) v.note->update(r->patch, v.midi, v.velocity, 0);
  }
  r->lfo.reset(r->patch + 137);
}

// The settings that are not part of the voice, as 20 integers:
//  0 mono (0/1)            1 engine (0 modern, 1 Mark I, 2 OPL)   2 cutoff 0..1000     3 resonance 0..1000
//  4 output 0..1000        5 tune -1000..1000 (a semitone)       6 bend up 0..48       7 bend down 0..48
//  8 bend step 0..12       9 portamento time 0..99               10 glissando 0/1
//  11..14 wheel, foot, breath, aftertouch ranges 0..99
//  15..18 wheel, foot, breath, aftertouch assignments (1 pitch, 2 amp, 4 EG bias)
void rdx_set_globals(void* handle, const int32_t* g) {
  Rdx* r = static_cast<Rdx*>(handle);
  Controllers& c = r->controllers;
  bool mono = g[0] != 0;
  if (mono != r->mono) {
    panic(r);
    r->mono = mono;
  }
  if (g[1] != r->engine) set_engine(r, g[1]);
  r->fx.ui_cutoff = g[2] / 1000.0f;
  r->fx.ui_reso = g[3] / 1000.0f;
  r->fx.ui_gain = g[4] / 1000.0f;
  int32_t tune = (int32_t)((g[5] / 1000.0) * 0x2000);
  c.masterTune = (int32_t)(((float)(tune << 11)) * (1.0 / 12));
  c.pitch_range_up = g[6];
  c.pitch_range_dn = g[7];
  c.pitch_step = g[8];
  c.portamento_cc = (int32_t)(g[9] * 127.0f / 99.0f);
  c.portamento_enable_cc = c.portamento_cc > 0;
  c.portamento_gliss_cc = g[10] != 0;
  FmMod* mods[4] = {&c.wheel, &c.foot, &c.breath, &c.at};
  for (int i = 0; i < 4; i++) {
    mods[i]->range = g[11 + i];
    mods[i]->pitch = (g[15 + i] & 1) != 0;
    mods[i]->amp = (g[15 + i] & 2) != 0;
    mods[i]->eg = (g[15 + i] & 4) != 0;
  }
  c.refresh();
}

void rdx_note_off(void* handle, int midi);

void rdx_note_on(void* handle, int midi, int velocity) {
  Rdx* r = static_cast<Rdx*>(handle);
  if (velocity < 1) {
    rdx_note_off(handle, midi);
    return;
  }
  if (velocity > 127) velocity = 127;
  int pitch = clamp_note(midi + shift_of(r));
  const int channel = 0;

  // Voice stealing: the best score wins, a tie goes to the voice that went down longest ago.
  //   +4 not playing, +2 key up, +1 same pitch.
  bool trigger_lfo = true;
  int best = r->current;
  int best_score = -1;
  int at = r->current;
  for (int i = 0; i < kVoices; i++) {
    const Voice& v = r->voice[at];
    if (v.keydown) trigger_lfo = false;
    int score = 0;
    if (!v.live || !v.note->isPlaying()) score += 4;
    if (!v.keydown) score += 2;
    if (v.midi == pitch) score += 1;
    if (score > best_score || (score == best_score && v.seq < r->voice[best].seq)) {
      best = at;
      best_score = score;
    }
    at = (at + 1) % kVoices;
  }
  if (trigger_lfo) r->lfo.keydown();

  int note = best;
  r->current = (note + 1) % kVoices;
  Voice& v = r->voice[note];
  v.midi = pitch;
  v.velocity = velocity;
  v.sustained = r->sustain;
  v.keydown = true;
  v.seq = r->next_seq++;
  v.quiet = 0;

  // no oscillator sync when stealing, it would click
  bool stolen = v.live && v.note->isPlaying();
  v.note->init(r->patch, pitch, velocity, channel, &r->controllers);
  if (r->patch[136] && !stolen) v.note->oscSync();

  if (r->voice[r->last_active].midi != -1 && r->controllers.portamento_enable_cc &&
      r->controllers.portamento_cc > 0) {
    v.note->initPortamento(*r->voice[r->last_active].note);
  }

  if (r->mono) {
    for (int i = 0; i < kVoices; i++) {
      if (r->voice[i].live) {
        // all keys are up: only the signal carries over
        if (!r->voice[i].keydown) {
          r->voice[i].live = false;
          v.note->transferSignal(*r->voice[i].note);
          break;
        }
        // a higher key is held: it takes over
        if (r->voice[i].midi > pitch) {
          r->voice[i].live = false;
          v.note->transferState(*r->voice[i].note);
          break;
        }
        // a lower key is held: the new note waits until it is the highest
        return;
      }
    }
  } else if (!r->patch[136]) {
    // another note of the same pitch: share its phase, or the two interfere unpredictably
    for (int i = 0; i < kVoices; i++) {
      if (i != note && r->voice[i].live && r->voice[i].note->isPlaying() && r->voice[i].midi == pitch) {
        v.note->transferPhase(*r->voice[i].note);
        break;
      }
    }
  }
  v.live = true;
  r->last_active = note;
}

void rdx_note_off(void* handle, int midi) {
  Rdx* r = static_cast<Rdx*>(handle);
  int pitch = clamp_note(midi + shift_of(r));
  int note;
  for (note = 0; note < kVoices; note++) {
    if (r->voice[note].midi == pitch && r->voice[note].keydown) {
      r->voice[note].keydown = false;
      break;
    }
  }
  if (note >= kVoices) return;

  if (r->mono) {
    int high = -1;
    int target = 0;
    for (int i = 0; i < kVoices; i++) {
      if (r->voice[i].keydown && r->voice[i].midi > high) {
        target = i;
        high = r->voice[i].midi;
      }
    }
    if (high != -1 && r->voice[note].live) {
      r->voice[note].live = false;
      r->voice[target].live = true;
      r->voice[target].quiet = 0;
      r->voice[target].note->transferState(*r->voice[note].note);
    }
  }

  if (r->sustain) {
    r->voice[note].sustained = true;
  } else {
    r->voice[note].note->keyup();
  }
}

void rdx_sustain(void* handle, int on) {
  Rdx* r = static_cast<Rdx*>(handle);
  r->sustain = on != 0;
  if (!r->sustain) {
    for (int i = 0; i < kVoices; i++) {
      Voice& v = r->voice[i];
      if (v.live && v.sustained && !v.keydown) {
        v.note->keyup();
        v.sustained = false;
      }
    }
  }
}

// 0..16383, 8192 at rest.
void rdx_pitch_bend(void* handle, int value) {
  static_cast<Rdx*>(handle)->controllers.pitch_cc = value;
}

// Mod wheel (1), breath (2), foot (4) and sustain (64), 0..127; all sound off (120) and all notes off (123).
void rdx_cc(void* handle, int cc, int value) {
  Rdx* r = static_cast<Rdx*>(handle);
  Controllers& c = r->controllers;
  switch (cc) {
    case 1:
      c.modwheel_cc = value;
      c.refresh();
      break;
    case 2:
      c.breath_cc = value;
      c.refresh();
      break;
    case 4:
      c.foot_cc = value;
      c.refresh();
      break;
    case 64:
      rdx_sustain(handle, value >= 64);
      break;
    case 120:
    case 123:
      panic(r);
      break;
  }
}

// Channel pressure, 0..127.
void rdx_pressure(void* handle, int value) {
  Rdx* r = static_cast<Rdx*>(handle);
  r->controllers.aftertouch_cc = value;
  r->controllers.refresh();
}

void rdx_all_off(void* handle) {
  Rdx* r = static_cast<Rdx*>(handle);
  panic(r);
  r->sustain = false;
}

// One block of the engine's N frames, mono, in -1..1.
static void make_block(Rdx* r) {
  AlignedBuf<int32_t, N> one;
  float sum[N];
  for (int j = 0; j < N; j++) sum[j] = 0;
  int32_t lfo_value = r->lfo.getsample();
  int32_t lfo_delay = r->lfo.getdelay();
  const float scale = 1.0f / 268435456.0f;  // 2^28 is full scale
  for (int i = 0; i < kVoices; i++) {
    Voice& v = r->voice[i];
    if (!v.live) continue;
    for (int j = 0; j < N; j++) one.get()[j] = 0;
    v.note->compute(one.get(), lfo_value, lfo_delay, &r->controllers);
    int32_t peak = 0;
    for (int j = 0; j < N; j++) {
      int32_t x = one.get()[j];
      float f = x * scale;
      sum[j] += f > 1.0f ? 1.0f : (f < -1.0f ? -1.0f : f);
      int32_t a = x < 0 ? -x : x;
      if (a > peak) peak = a;
    }
    if (!v.keydown && !v.sustained) {
      if (peak < kSilent) {
        if (++v.quiet >= kQuietBlocks) v.live = false;
      } else {
        v.quiet = 0;
      }
    }
  }
  for (int j = 0; j < N; j++) r->block[j] = sum[j];
  r->pos = 0;
}

void rdx_render(void* handle, float* out, int frames) {
  Rdx* r = static_cast<Rdx*>(handle);
  int done = 0;
  while (done < frames) {
    if (r->pos >= N) make_block(r);
    int take = N - r->pos;
    if (take > frames - done) take = frames - done;
    memcpy(out + done, r->block + r->pos, take * sizeof(float));
    r->pos += take;
    done += take;
  }
  r->fx.process(out, frames);
}

}  // extern "C"
