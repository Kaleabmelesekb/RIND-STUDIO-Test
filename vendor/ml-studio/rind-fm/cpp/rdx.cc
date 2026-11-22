// SPDX-License-Identifier: Apache-2.0 AND GPL-3.0-or-later
// Honeydew's glue around the msfa engine (Google's music-synthesizer-for-android, Apache-2.0): sixteen DX7 voices
// and the DX7's one LFO, behind a C interface. Voice handling follows msfa's `SynthUnit`; what differs is that voices
// stop computing once they have released into silence, and that notes and patches come as calls, not MIDI bytes.
#include <new>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#include "synth.h"
#include "aligned_buf.h"
#include "controllers.h"
#include "dx7note.h"
#include "exp2.h"
#include "freqlut.h"
#include "lfo.h"
#include "patch.h"
#include "pitchenv.h"
#include "sin.h"

namespace {

const int kVoices = 16;
// A released voice under this (of 2^28, full scale) for this many blocks is over: -96 dB.
const int32_t kSilent = 4096;
const int kQuietBlocks = 4;

struct Voice {
  Dx7Note note;
  int midi;
  bool keydown;
  bool sustained;
  bool live;
  int quiet;
};

struct Rdx {
  Voice voice[kVoices];
  Lfo lfo;
  Controllers controllers;
  char patch[156];
  bool sustain;
  int next;
  // The last block rendered, the part of it not handed out yet at `pos`.
  float block[N];
  int pos;
};

double g_rate = 0;

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
  g_rate = rate;
}

void* rdx_new() {
  void* memory = calloc(1, sizeof(Rdx));
  if (!memory) return 0;
  Rdx* r = new (memory) Rdx();
  r->controllers.values_[kControllerPitch] = 0x2000;
  r->pos = N;
  return r;
}

void rdx_free(void* handle) {
  Rdx* r = static_cast<Rdx*>(handle);
  if (!r) return;
  r->~Rdx();
  free(r);
}

// A voice as a sysex bank stores it (128 bytes) to the form the engine plays (156 bytes).
void rdx_unpack(const char* packed, char* unpacked) { UnpackPatch(packed, unpacked); }

// The patch the next notes play; the LFO takes its settings.
void rdx_set_patch(void* handle, const char* unpacked) {
  Rdx* r = static_cast<Rdx*>(handle);
  memcpy(r->patch, unpacked, 156);
  r->lfo.reset(r->patch + 137);
}

void rdx_note_on(void* handle, int midi, int velocity) {
  Rdx* r = static_cast<Rdx*>(handle);
  // The next voice that is not held down, else the one after the last taken.
  int index = -1;
  for (int i = 0; i < kVoices; i++) {
    int at = (r->next + i) % kVoices;
    if (!r->voice[at].keydown) {
      index = at;
      break;
    }
  }
  if (index < 0) index = r->next;
  r->next = (index + 1) % kVoices;
  Voice& v = r->voice[index];
  r->lfo.keydown();
  v.midi = midi;
  v.keydown = true;
  v.sustained = r->sustain;
  v.live = true;
  v.quiet = 0;
  v.note.init(r->patch, midi, velocity < 1 ? 1 : (velocity > 127 ? 127 : velocity));
}

void rdx_note_off(void* handle, int midi) {
  Rdx* r = static_cast<Rdx*>(handle);
  for (int i = 0; i < kVoices; i++) {
    Voice& v = r->voice[i];
    if (v.live && v.keydown && v.midi == midi) {
      v.keydown = false;
      if (r->sustain) {
        v.sustained = true;
      } else {
        v.note.keyup();
      }
    }
  }
}

void rdx_sustain(void* handle, int on) {
  Rdx* r = static_cast<Rdx*>(handle);
  r->sustain = on != 0;
  if (!r->sustain) {
    for (int i = 0; i < kVoices; i++) {
      Voice& v = r->voice[i];
      if (v.live && v.sustained && !v.keydown) {
        v.note.keyup();
        v.sustained = false;
      }
    }
  }
}

// 0..16383, 8192 at rest.
void rdx_pitch_bend(void* handle, int value) {
  static_cast<Rdx*>(handle)->controllers.values_[kControllerPitch] = value;
}

void rdx_all_off(void* handle) {
  Rdx* r = static_cast<Rdx*>(handle);
  for (int i = 0; i < kVoices; i++) {
    r->voice[i].live = false;
    r->voice[i].keydown = false;
    r->voice[i].sustained = false;
  }
  r->sustain = false;
  r->pos = N;
}

// One block of the engine's N frames, mono, in -1..1.
static void make_block(Rdx* r) {
  AlignedBuf<int32_t, N> mix;
  for (int j = 0; j < N; j++) mix.get()[j] = 0;
  int32_t lfo_value = r->lfo.getsample();
  int32_t lfo_delay = r->lfo.getdelay();
  for (int i = 0; i < kVoices; i++) {
    Voice& v = r->voice[i];
    if (!v.live) continue;
    AlignedBuf<int32_t, N> one;
    for (int j = 0; j < N; j++) one.get()[j] = 0;
    v.note.compute(one.get(), lfo_value, lfo_delay, &r->controllers);
    int32_t peak = 0;
    for (int j = 0; j < N; j++) {
      int32_t x = one.get()[j];
      mix.get()[j] += x;
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
  const float scale = 1.0f / 268435456.0f;  // 2^28 is full scale
  for (int j = 0; j < N; j++) {
    float x = mix.get()[j] * scale;
    r->block[j] = x > 1.0f ? 1.0f : (x < -1.0f ? -1.0f : x);
  }
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
}

}  // extern "C"
