// A C face for one Dragonfly DSP (`DragonflyReverbDSP`, from the plugin's own DSP.hpp), built once per plugin with
// DF_PREFIX naming its entry points and DragonflyReverbDSP renamed per plugin (they share a class name).
#include "DistrhoPlugin.hpp"
#include "DistrhoPluginInfo.h"
#include "DSP.hpp"

#define DF_CAT2(a, b) a##b
#define DF_CAT(a, b) DF_CAT2(a, b)
#define DF_FN(name) DF_CAT(DF_PREFIX, name)

extern "C" {

void* DF_FN(_new)(double sample_rate) { return new DragonflyReverbDSP(sample_rate); }

void DF_FN(_free)(void* dsp) { delete static_cast<DragonflyReverbDSP*>(dsp); }

void DF_FN(_set)(void* dsp, uint32_t index, float value) {
  static_cast<DragonflyReverbDSP*>(dsp)->setParameterValue(index, value);
}

void DF_FN(_sample_rate)(void* dsp, double sample_rate) {
  static_cast<DragonflyReverbDSP*>(dsp)->sampleRateChanged(sample_rate);
}

void DF_FN(_mute)(void* dsp) { static_cast<DragonflyReverbDSP*>(dsp)->mute(); }

void DF_FN(_run)(void* dsp, const float* in_l, const float* in_r, float* out_l, float* out_r, uint32_t frames) {
  const float* inputs[2] = {in_l, in_r};
  float* outputs[2] = {out_l, out_r};
  static_cast<DragonflyReverbDSP*>(dsp)->run(inputs, outputs, frames);
}

}
