// A C interface to Odin 2's synthesizer (Honeydew).
#include "PluginProcessor.h"

#include <cstdint>
#include <cstring>

namespace {
struct Synth {
	OdinAudioProcessor p;
	MidiBuffer midi;
	Synth(float rate) {
		p.prepareToPlay(rate, 512);
	}
};
} // namespace

extern "C" {
void *odin_new(float rate) {
	return new Synth(rate);
}
void odin_free(void *s) {
	delete static_cast<Synth *>(s);
}
/// 1 when the bytes were an Odin 2 patch.
int odin_load(void *s, const uint8_t *data, size_t size) {
	auto *o = static_cast<Synth *>(s);
	bool ok = o->p.loadPatchData(data, size);
	o->p.resetAudioEngine();
	return ok ? 1 : 0;
}
/// A MIDI message (status, data1, data2) `at` samples into the next block.
void odin_event(void *s, int status, int d1, int d2, int at) {
	auto *o = static_cast<Synth *>(s);
	o->midi.addEvent(MidiMessage(status, d1, d2), at);
}
void odin_render(void *s, float *l, float *r, int n) {
	auto *o = static_cast<Synth *>(s);
	std::memset(l, 0, sizeof(float) * (size_t)n);
	std::memset(r, 0, sizeof(float) * (size_t)n);
	o->p.processBlock(l, r, n, o->midi);
	o->midi.clear();
}
/// The parameter with this id, in its own range; 0 when there is none.
float odin_get(void *s, const char *id) {
	auto *o = static_cast<Synth *>(s);
	auto *v = o->p.m_value_tree.getRawParameterValue(id);
	return v ? v->load() : 0.f;
}
/// Sets a parameter (in its own range); 0 when there is no such parameter.
int odin_set(void *s, const char *id, float value) {
	auto *o = static_cast<Synth *>(s);
	auto *p = o->p.m_value_tree.getParameter(id);
	if (!p) {
		return 0;
	}
	p->setValueNotifyingHost(p->convertTo0to1(value));
	return 1;
}
/// Sets a property of the patch's tree `tree` ("osc", "misc", "mod", "lfo"), as the engine's own editor does: the engine
/// hears it at once. 0 when there is no such tree.
/// Tells the engine the host tempo (arpeggiator, synced LFOs and effects follow it).
void odin_tempo(void *s, float bpm) {
	static_cast<Synth *>(s)->p.setBPM(bpm);
}
int odin_set_prop(void *s, const char *tree, const char *key, double value, int as_int) {
	auto *o = static_cast<Synth *>(s);
	auto node = o->p.m_value_tree.state.getChildWithName(tree);
	if (!node.isValid()) {
		return 0;
	}
	if (as_int) {
		node.setProperty(key, (int)value, nullptr);
	} else {
		node.setProperty(key, value, nullptr);
	}
	return 1;
}
/// Parameter `i` (in id order) as one line: id, name, kind (f, i, b, c), min, max, skew, interval, default, choices.
int odin_param_info(void *s, int i, char *out, int cap) {
	auto *o = static_cast<Synth *>(s);
	if (i < 0 || i >= (int)o->p.m_value_tree.params.size()) {
		return 0;
	}
	auto it = o->p.m_value_tree.params.begin();
	std::advance(it, i);
	const RangedAudioParameter &p = *it->second;
	std::string line = p.paramID + "\t" + p.name + "\t";
	line += "fibc"[p.kind];
	char num[160];
	std::snprintf(num, sizeof num, "\t%.9g\t%.9g\t%.9g\t%.9g\t%.9g\t", p.range.start, p.range.end, p.range.skew, p.range.interval, p.defaultValue);
	line += num;
	if (auto *c = dynamic_cast<const AudioParameterChoice *>(&p)) {
		for (size_t k = 0; k < c->choices.size(); ++k) {
			line += (k ? "|" : "") + std::string(c->choices[k]);
		}
	}
	if ((int)line.size() + 1 > cap) {
		return 0;
	}
	std::memcpy(out, line.c_str(), line.size() + 1);
	return (int)line.size();
}
int odin_param_count(void *s) {
	return (int)static_cast<Synth *>(s)->p.m_value_tree.params.size();
}
}

#include "audio/ImpulseResponseCreator.h"
// Odin 2 can write the impulse response of its feedback delay network to a file while developing; Honeydew does not.
void ImpulseResponseCreator::saveImpulseResponse(std::function<float(float)>, size_t, const std::string &) {
}
