// Honeydew's glue for Airwindows Consolidated (MIT, https://github.com/baconpaul/airwin2rack): a C ABI over the effects in
// registry.inc, which Rust calls from the audio thread (aw_process) and the editor (aw_display, on its own instance).
#include <cstdio>
#include <cstring>

#include "aw_entry.h"
#include "registry.inc"

namespace {
constexpr int COUNT = sizeof(ENTRIES) / sizeof(ENTRIES[0]);

struct Instance {
    std::unique_ptr<AirwinConsolidatedBase> fx;
    int params;
};
}  // namespace

extern "C" {
int aw_count() { return COUNT; }

void *aw_new(int index, float rate) {
    if (index < 0 || index >= COUNT) return nullptr;
    if (!(rate > 2000.f)) rate = 44100.f;
    AirwinConsolidatedBase::defaultSampleRate = rate;
    auto *o = new Instance{ENTRIES[index].make(), ENTRIES[index].params};
    o->fx->setSampleRate(rate);
    return o;
}

void aw_free(void *o) { delete static_cast<Instance *>(o); }

void aw_set(void *o, int p, float v) {
    auto *i = static_cast<Instance *>(o);
    if (i && p >= 0 && p < i->params) i->fx->setParameter(p, v < 0.f ? 0.f : (v > 1.f ? 1.f : v));
}

// The text of parameter `p`'s value and then its unit, as "value unit", into `out`.
void aw_display(void *o, int p, char *out, int n) {
    auto *i = static_cast<Instance *>(o);
    if (n <= 0) return;
    out[0] = 0;
    if (!i || p < 0 || p >= i->params) return;
    char value[64] = {0}, label[64] = {0};
    i->fx->getParameterDisplay(p, value);
    i->fx->getParameterLabel(p, label);
    // The values print with padding ("  0.5000"): trim it.
    const char *v = value;
    while (*v == ' ') ++v;
    if (label[0]) snprintf(out, n, "%s %s", v, label);
    else snprintf(out, n, "%s", v);
}

void aw_process(void *o, float *left, float *right, int frames) {
    auto *i = static_cast<Instance *>(o);
    if (!i || frames <= 0) return;
    float *in[2] = {left, right};
    float *out[2] = {left, right};
    i->fx->processReplacing(in, out, frames);
}
}
