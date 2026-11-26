// A stand-in for the few JUCE things Odin 2's DSP uses (Honeydew: JUCE itself is not part of the app).
#pragma once
#include <initializer_list>
#include <algorithm>
#include <atomic>
#include <cmath>
#include <cstdint>
#include <cstring>
#include <functional>
#include <map>
#include <memory>
#include <string>
#include <vector>
#include <list>
#include <iostream>
#include <sstream>
#include <iomanip>
#include <ctime>

#define DBG(x) do { } while (false)
#define jassert(x) do { } while (false)
#define jassertfalse do { } while (false)
#define JUCE_DECLARE_NON_COPYABLE_WITH_LEAK_DETECTOR(C) C(const C &) = delete; C &operator=(const C &) = delete;
#define JUCE_DECLARE_NON_COPYABLE(C) C(const C &) = delete; C &operator=(const C &) = delete;

namespace juce {
namespace dsp {
struct FastMathApproximations {
	template <typename T> static T exp(T x) { return std::exp(x); }
	template <typename T> static T tan(T x) { return std::tan(x); }
	template <typename T> static T sin(T x) { return std::sin(x); }
	template <typename T> static T cos(T x) { return std::cos(x); }
	template <typename T> static T tanh(T x) { return std::tanh(x); }
};
} // namespace dsp
struct Decibels {
	template <typename T> static T decibelsToGain(T db, T minusInfinityDb = T(-100)) { return db > minusInfinityDb ? std::pow(T(10), db * T(0.05)) : T(); }
	template <typename T> static T gainToDecibels(T g, T minusInfinityDb = T(-100)) { return g > T() ? std::max(minusInfinityDb, std::log10(g) * T(20)) : minusInfinityDb; }
};
template <typename T> inline T jlimit(T lo, T hi, T v) { return v < lo ? lo : (v > hi ? hi : v); }
template <typename T> inline T jmin(T a, T b) { return a < b ? a : b; }
template <typename T> inline T jmax(T a, T b) { return a > b ? a : b; }
template <typename T> struct Atomic {
	std::atomic<T> v;
	Atomic(T x = T()) : v(x) {}
	void set(T x) { v.store(x); }
	T get() const { return v.load(); }
	operator T() const { return v.load(); }
	Atomic &operator=(T x) { v.store(x); return *this; }
};
struct ScopedNoDenormals {};
} // namespace juce
using namespace juce;

// ---------------------------------------------------------------------------------------------------------------------
// Strings, values and trees: just what Odin 2's processor uses of juce::String, var, ValueTree and
// AudioProcessorValueTreeState, with the same behaviour (a property that does not change sends no message, listeners of the
// parents hear a child's change, parameters are children named PARAM with `id` and `value`).
// ---------------------------------------------------------------------------------------------------------------------
namespace juce {

class String : public std::string {
  public:
	String() = default;
	String(const std::string &s) : std::string(s) {}
	String(const char *s) : std::string(s ? s : "") {}
	String(int v) : std::string(std::to_string(v)) {}
	String(long v) : std::string(std::to_string(v)) {}
	String(long long v) : std::string(std::to_string(v)) {}
	String(unsigned v) : std::string(std::to_string(v)) {}
	String(float v) : std::string(trim(v)) {}
	String(double v) : std::string(trim(v)) {}
	String toString() const { return *this; }
	std::string toStdString() const { return *this; }
	bool isEmpty() const { return empty(); }
	bool isNotEmpty() const { return !empty(); }
	bool contains(const String &s) const { return find(s) != npos; }
	bool startsWith(const String &s) const { return rfind(s, 0) == 0; }
	bool endsWith(const String &s) const { return size() >= s.size() && compare(size() - s.size(), s.size(), s) == 0; }
	int getIntValue() const { return std::atoi(c_str()); }
	float getFloatValue() const { return (float)std::atof(c_str()); }
	double getDoubleValue() const { return std::atof(c_str()); }
	String substring(int start, int end) const { return start >= (int)size() ? String() : String(substr(start, end - start)); }
	String substring(int start) const { return start >= (int)size() ? String() : String(substr(start)); }
	int length() const { return (int)size(); }
	int compareIgnoreCase(const String &o) const {
		std::string a = *this, b = o;
		for (auto &c : a) c = (char)std::tolower((unsigned char)c);
		for (auto &c : b) c = (char)std::tolower((unsigned char)c);
		return a.compare(b);
	}
	const char *toRawUTF8() const { return c_str(); }
	static String fromUTF8(const char *s) { return String(s); }

  private:
	template <typename T> static std::string trim(T v) {
		char buf[64];
		std::snprintf(buf, sizeof buf, "%.7g", (double)v);
		return buf;
	}
};
inline String operator+(const String &a, const String &b) { return String(static_cast<const std::string &>(a) + static_cast<const std::string &>(b)); }
inline String operator+(const String &a, const char *b) { return a + String(b); }
inline String operator+(const char *a, const String &b) { return String(a) + b; }
inline String operator+(const String &a, int b) { return a + String(b); }
inline String operator+(const String &a, const std::string &b) { return a + String(b); }
inline String operator+(const std::string &a, const String &b) { return String(a) + b; }

class StringArray : public std::vector<String> {
  public:
	StringArray() = default;
	StringArray(std::initializer_list<String> l) : std::vector<String>(l) {}
	int size() const { return (int)std::vector<String>::size(); }
	void add(const String &s) { push_back(s); }
	int indexOf(const String &s) const {
		for (size_t i = 0; i < std::vector<String>::size(); ++i)
			if ((*this)[i] == s) return (int)i;
		return -1;
	}
};

class Identifier {
  public:
	Identifier() = default;
	Identifier(const char *s) : name(s) {}
	Identifier(const String &s) : name(s) {}
	Identifier(const std::string &s) : name(s) {}
	String toString() const { return name; }
	bool isValid() const { return !name.empty(); }
	bool operator==(const Identifier &o) const { return name == o.name; }
	bool operator!=(const Identifier &o) const { return name != o.name; }
	bool operator==(const char *o) const { return name == o; }
	bool operator==(const String &o) const { return name == o; }
	operator const String &() const { return name; }
	String name;
};

class StringRef {
  public:
	StringRef(const String &s) : s(s) {}
	StringRef(const char *c) : s(c) {}
	StringRef(const Identifier &i) : s(i.name) {}
	bool operator==(const Identifier &i) const { return s == i.name; }
	bool operator!=(const Identifier &i) const { return s != i.name; }
	bool operator==(const char *c) const { return s == c; }
	bool operator==(const String &o) const { return s == o; }
	operator String() const { return s; }
	String s;
};

class var {
  public:
	enum Kind { Void, Int, Int64, Double, Bool, Str } kind = Void;
	var() = default;
	var(int v) : kind(Int), i(v), d(v) {}
	var(long v) : kind(Int64), i(v), d((double)v) {}
	var(long long v) : kind(Int64), i(v), d((double)v) {}
	var(float v) : kind(Double), d(v) {}
	var(double v) : kind(Double), d(v) {}
	var(bool v) : kind(Bool), i(v), d(v) {}
	var(const char *v) : kind(Str), s(v) {}
	var(const String &v) : kind(Str), s(v) {}
	operator int() const { return kind == Str ? s.getIntValue() : kind == Double ? (int)d : (int)i; }
	operator int64_t() const { return kind == Str ? (int64_t)s.getIntValue() : kind == Double ? (int64_t)d : i; }
	operator float() const { return (float)(double)*this; }
	operator double() const { return kind == Str ? s.getDoubleValue() : kind == Double ? d : (double)i; }
	operator bool() const { return kind == Str ? s.getIntValue() != 0 : kind == Double ? d != 0. : i != 0; }
	operator String() const { return toString(); }
	String toString() const {
		switch (kind) {
		case Void: return String();
		case Int: case Int64: case Bool: return String((long long)i);
		case Double: return String(d);
		case Str: return s;
		}
		return String();
	}
	bool isVoid() const { return kind == Void; }
	bool operator==(const var &o) const {
		if (kind == Str || o.kind == Str) return toString() == o.toString();
		if (kind == Void || o.kind == Void) return kind == o.kind;
		return (double)*this == (double)o;
	}
	bool operator!=(const var &o) const { return !(*this == o); }
	int64_t i = 0;
	double d = 0.;
	String s;
};

class UndoManager;
class ValueTree {
  public:
	class Listener {
	  public:
		virtual ~Listener() = default;
		virtual void valueTreePropertyChanged(ValueTree &, const Identifier &) {}
		virtual void valueTreeChildAdded(ValueTree &, ValueTree &) {}
		virtual void valueTreeChildRemoved(ValueTree &, ValueTree &, int) {}
		virtual void valueTreeChildOrderChanged(ValueTree &, int, int) {}
		virtual void valueTreeParentChanged(ValueTree &) {}
	};
	ValueTree() = default;
	explicit ValueTree(const Identifier &type) : o(std::make_shared<Obj>()) { o->type = type; }
	bool isValid() const { return o != nullptr; }
	bool operator==(const ValueTree &x) const { return o == x.o; }
	bool operator!=(const ValueTree &x) const { return o != x.o; }
	Identifier getType() const { return o ? o->type : Identifier(); }
	int getNumProperties() const { return o ? (int)o->props.size() : 0; }
	Identifier getPropertyName(int i) const { return o->props[(size_t)i].first; }
	bool hasProperty(const Identifier &id) const { return find(id) != nullptr; }
	const var &getProperty(const Identifier &id) const {
		static const var none;
		auto *p = find(id);
		return p ? *p : none;
	}
	var getProperty(const Identifier &id, const var &fallback) const {
		auto *p = find(id);
		return p ? *p : fallback;
	}
	var operator[](const Identifier &id) const { return getProperty(id); }
	ValueTree &setProperty(const Identifier &id, const var &v, UndoManager * = nullptr) {
		if (!o) return *this;
		for (auto &p : o->props) {
			if (p.first == id) {
				if (p.second == v && p.second.kind == v.kind) return *this;
				p.second = v;
				send(id);
				return *this;
			}
		}
		o->props.emplace_back(id, v);
		send(id);
		return *this;
	}
	void sendPropertyChangeMessage(const Identifier &id) { send(id); }
	void removeProperty(const Identifier &id, UndoManager * = nullptr) {
		if (!o) return;
		for (size_t k = 0; k < o->props.size(); ++k)
			if (o->props[k].first == id) { o->props.erase(o->props.begin() + (long)k); send(id); return; }
	}
	int getNumChildren() const { return o ? (int)o->children.size() : 0; }
	ValueTree getChild(int i) const { return o && i >= 0 && i < getNumChildren() ? o->children[(size_t)i] : ValueTree(); }
	ValueTree getChildWithName(const Identifier &t) const {
		if (o) for (auto &c : o->children) if (c.getType() == t) return c;
		return ValueTree();
	}
	ValueTree getParent() const { return o && !o->parent.expired() ? ValueTree(o->parent.lock()) : ValueTree(); }
	void addChild(const ValueTree &c, int index, UndoManager * = nullptr) {
		if (!o || !c.o) return;
		c.o->parent = o;
		if (index < 0 || index >= getNumChildren()) o->children.push_back(c);
		else o->children.insert(o->children.begin() + index, c);
		ValueTree me(*this), ch(c);
		notify([&](Listener *l) { l->valueTreeChildAdded(me, ch); });
	}
	void appendChild(const ValueTree &c, UndoManager *u = nullptr) { addChild(c, -1, u); }
	ValueTree createCopy() const {
		if (!o) return ValueTree();
		ValueTree t(o->type);
		t.o->props = o->props;
		for (auto &c : o->children) {
			ValueTree cc = c.createCopy();
			cc.o->parent = t.o;
			t.o->children.push_back(cc);
		}
		return t;
	}
	void copyPropertiesFrom(const ValueTree &s, UndoManager * = nullptr) {
		for (auto &p : s.o->props) setProperty(p.first, p.second);
	}
	void addListener(Listener *l) { if (o) o->listeners.push_back(l); }
	void removeListener(Listener *l) { if (o) o->listeners.erase(std::remove(o->listeners.begin(), o->listeners.end(), l), o->listeners.end()); }

	/// JUCE's binary form (what `ValueTree::writeToStream` writes), as in Odin's patch files.
	static ValueTree readFromData(const uint8_t *data, size_t size) {
		size_t at = 0;
		bool ok = true;
		ValueTree t = read(data, size, at, ok);
		return ok ? t : ValueTree();
	}

  private:
	struct Obj {
		Identifier type;
		std::vector<std::pair<Identifier, var>> props;
		std::vector<ValueTree> children;
		std::weak_ptr<Obj> parent;
		std::vector<Listener *> listeners;
	};
	explicit ValueTree(std::shared_ptr<Obj> p) : o(std::move(p)) {}
	std::shared_ptr<Obj> o;
	const var *find(const Identifier &id) const {
		if (o) for (auto &p : o->props) if (p.first == id) return &p.second;
		return nullptr;
	}
	template <typename F> void notify(F f) {
		auto at = o;
		while (at) {
			auto ls = at->listeners;
			for (auto *l : ls) f(l);
			at = at->parent.lock();
		}
	}
	void send(const Identifier &id) {
		ValueTree me(*this);
		notify([&](Listener *l) { l->valueTreePropertyChanged(me, id); });
	}
	static int cint(const uint8_t *d, size_t n, size_t &at, bool &ok) {
		if (at >= n) { ok = false; return 0; }
		unsigned h = d[at++], num = h & 0x7f;
		if (num > 4 || at + num > n) { ok = false; return 0; }
		unsigned v = 0;
		for (unsigned k = 0; k < num; ++k) v |= (unsigned)d[at++] << (8 * k);
		return h & 0x80 ? -(int)v : (int)v;
	}
	static String str(const uint8_t *d, size_t n, size_t &at, bool &ok) {
		size_t e = at;
		while (e < n && d[e]) ++e;
		if (e >= n) { ok = false; return String(); }
		String s(std::string((const char *)d + at, e - at));
		at = e + 1;
		return s;
	}
	static ValueTree read(const uint8_t *d, size_t n, size_t &at, bool &ok) {
		String type = str(d, n, at, ok);
		if (!ok) return ValueTree();
		ValueTree t{Identifier(type)};
		int np = cint(d, n, at, ok);
		for (int k = 0; ok && k < np; ++k) {
			String name = str(d, n, at, ok);
			int len = cint(d, n, at, ok);
			var v;
			if (ok && len > 0) {
				if (at + (size_t)len > n) { ok = false; break; }
				int m = d[at];
				const uint8_t *p = d + at + 1;
				if (m == 1 && len == 5) { int32_t x; std::memcpy(&x, p, 4); v = var(x); }
				else if (m == 2) v = var(true);
				else if (m == 3) v = var(false);
				else if (m == 4 && len == 9) { double x; std::memcpy(&x, p, 8); v = var(x); }
				else if (m == 5) v = var(String(std::string((const char *)p, (size_t)len - 2)));
				else if (m == 6 && len == 9) { int64_t x; std::memcpy(&x, p, 8); v = var((long long)x); }
				at += (size_t)len;
			}
			if (ok) t.o->props.emplace_back(Identifier(name), v);
		}
		int nc = ok ? cint(d, n, at, ok) : 0;
		for (int k = 0; ok && k < nc; ++k) {
			ValueTree c = read(d, n, at, ok);
			if (ok) { c.o->parent = t.o; t.o->children.push_back(c); }
		}
		return t;
	}
};

template <typename T> struct NormalisableRange {
	T start{}, end{T(1)}, interval{}, skew{T(1)};
	bool symmetricSkew = false;
	NormalisableRange() = default;
	NormalisableRange(T s, T e, T i, T sk, bool sym = false) : start(s), end(e), interval(i), skew(sk), symmetricSkew(sym) {}
	NormalisableRange(T s, T e, T i) : start(s), end(e), interval(i) {}
	NormalisableRange(T s, T e) : start(s), end(e) {}
	T convertFrom0to1(T v) const {
		if (skew != T(1) && v > T(0)) v = std::exp(std::log(v) / skew);
		return start + (end - start) * v;
	}
	T convertTo0to1(T v) const {
		v = (v - start) / (end - start);
		if (v < T(0)) v = T(0);
		if (v > T(1)) v = T(1);
		if (skew != T(1) && v > T(0)) v = std::pow(v, skew);
		return v;
	}
	T snapToLegalValue(T v) const {
		if (interval > T(0)) v = start + interval * std::floor((v - start) / interval + T(0.5));
		return v < start ? start : (v > end ? end : v);
	}
};

class AudioProcessorValueTreeState;
class RangedAudioParameter {
  public:
	enum Kind { Float, Int, Bool, Choice };
	RangedAudioParameter(Kind k, const String &id, const String &name, NormalisableRange<float> r, float def) : kind(k), paramID(id), name(name), range(r), defaultValue(def) { value.store(def); }
	virtual ~RangedAudioParameter() = default;
	String getParameterID() const { return paramID; }
	String getName(int = 0) const { return name; }
	float convertTo0to1(float v) const { return range.convertTo0to1(v); }
	float convertFrom0to1(float v) const {
		float r = range.convertFrom0to1(v < 0.f ? 0.f : (v > 1.f ? 1.f : v));
		if (kind == Int || kind == Choice || kind == Bool) r = std::round(r);
		return range.snapToLegalValue(r);
	}
	float getValue() const { return convertTo0to1(value.load()); }
	float getDefaultValue() const { return convertTo0to1(defaultValue); }
	const NormalisableRange<float> &getNormalisableRange() const { return range; }
	void setValueNotifyingHost(float normalised);
	void setValue(float normalised) { value.store(convertFrom0to1(normalised)); }
	Kind kind;
	String paramID, name;
	NormalisableRange<float> range;
	float defaultValue;
	std::atomic<float> value{0.f};
	AudioProcessorValueTreeState *owner = nullptr;
};
struct AudioParameterFloat : RangedAudioParameter {
	AudioParameterFloat(const String &id, const String &name, float lo, float hi, float def) : RangedAudioParameter(Float, id, name, NormalisableRange<float>(lo, hi), def) {}
	AudioParameterFloat(const String &id, const String &name, NormalisableRange<float> r, float def) : RangedAudioParameter(Float, id, name, r, def) {}
};
struct AudioParameterInt : RangedAudioParameter {
	AudioParameterInt(const String &id, const String &name, int lo, int hi, int def) : RangedAudioParameter(Int, id, name, NormalisableRange<float>((float)lo, (float)hi, 1.f), (float)def) {}
};
struct AudioParameterBool : RangedAudioParameter {
	AudioParameterBool(const String &id, const String &name, bool def) : RangedAudioParameter(Bool, id, name, NormalisableRange<float>(0.f, 1.f, 1.f), def ? 1.f : 0.f) {}
};
struct AudioParameterChoice : RangedAudioParameter {
	AudioParameterChoice(const String &id, const String &name, const StringArray &c, int def) : RangedAudioParameter(Choice, id, name, NormalisableRange<float>(0.f, (float)(c.size() - 1), 1.f), (float)def), choices(c) {}
	StringArray choices;
};

class AudioProcessor;
struct UndoManager {};
struct Value {
	var v;
	var getValue() const { return v; }
};

class AudioProcessorValueTreeState {
  public:
	class Listener {
	  public:
		virtual ~Listener() = default;
		virtual void parameterChanged(const String &parameterID, float newValue) = 0;
	};
	struct ParameterLayout {
		std::vector<std::unique_ptr<RangedAudioParameter>> items;
		ParameterLayout() = default;
		template <typename... Ts> ParameterLayout(std::unique_ptr<Ts>... ts) { (void)std::initializer_list<int>{(items.push_back(std::unique_ptr<RangedAudioParameter>(std::move(ts))), 0)...}; }
	};
	template <typename P> AudioProcessorValueTreeState(P &, UndoManager *, const Identifier &type, ParameterLayout layout) : state(type) {
		for (auto &p : layout.items) {
			p->owner = this;
			ValueTree n{Identifier("PARAM")};
			n.setProperty("id", p->paramID);
			n.setProperty("value", var(p->defaultValue));
			state.appendChild(n);
			params[p->paramID] = std::move(p);
		}
	}
	RangedAudioParameter *getParameter(const String &id) const {
		auto it = params.find(id);
		return it == params.end() ? nullptr : it->second.get();
	}
	std::atomic<float> *getRawParameterValue(const String &id) const {
		auto *p = getParameter(id);
		return p ? &p->value : nullptr;
	}
	Value getParameterAsValue(const String &id) const {
		Value v;
		if (auto *p = getParameter(id)) v.v = var(p->value.load());
		return v;
	}
	void addParameterListener(const String &id, Listener *l) { listeners[id].push_back(l); }
	void removeParameterListener(const String &id, Listener *l) {
		auto &v = listeners[id];
		v.erase(std::remove(v.begin(), v.end(), l), v.end());
	}
	ValueTree copyState() { return state.createCopy(); }
	// Called by a parameter whose value changed.
	void changed(RangedAudioParameter &p) {
		for (auto n = 0; n < state.getNumChildren(); ++n) {
			auto c = state.getChild(n);
			if (c.getProperty("id").toString() == p.paramID) { c.setProperty("value", var(p.value.load())); break; }
		}
		auto it = listeners.find(p.paramID);
		if (it != listeners.end()) {
			auto copy = it->second;
			for (auto *l : copy) l->parameterChanged(p.paramID, p.value.load());
		}
	}
	ValueTree state;
	std::map<String, std::unique_ptr<RangedAudioParameter>> params;
	std::map<String, std::vector<Listener *>> listeners;
};
inline void RangedAudioParameter::setValueNotifyingHost(float normalised) {
	value.store(convertFrom0to1(normalised));
	if (owner) owner->changed(*this);
}

} // namespace juce

namespace juce {
class MidiMessage {
  public:
	MidiMessage() = default;
	MidiMessage(int b0, int b1, int b2 = 0) : d{(uint8_t)b0, (uint8_t)b1, (uint8_t)b2} {}
	static MidiMessage noteOn(int ch, int note, uint8_t vel) { return MidiMessage(0x90 | ((ch - 1) & 15), note, vel); }
	static MidiMessage noteOff(int ch, int note, uint8_t vel = 0) { return MidiMessage(0x80 | ((ch - 1) & 15), note, vel); }
	static MidiMessage controllerEvent(int ch, int num, int val) { return MidiMessage(0xb0 | ((ch - 1) & 15), num, val); }
	static MidiMessage pitchWheel(int ch, int v) { return MidiMessage(0xe0 | ((ch - 1) & 15), v & 127, (v >> 7) & 127); }
	static MidiMessage channelPressureChange(int ch, int v) { return MidiMessage(0xd0 | ((ch - 1) & 15), v); }
	int kind() const { return d[0] & 0xf0; }
	bool isNoteOn() const { return kind() == 0x90 && d[2] > 0; }
	bool isNoteOff() const { return kind() == 0x80 || (kind() == 0x90 && d[2] == 0); }
	int getNoteNumber() const { return d[1]; }
	uint8_t getVelocity() const { return d[2]; }
	bool isController() const { return kind() == 0xb0; }
	int getControllerNumber() const { return d[1]; }
	int getControllerValue() const { return d[2]; }
	bool isPitchWheel() const { return kind() == 0xe0; }
	int getPitchWheelValue() const { return d[1] | (d[2] << 7); }
	bool isChannelPressure() const { return kind() == 0xd0; }
	int getChannelPressureValue() const { return d[1]; }
	bool isAftertouch() const { return kind() == 0xa0; }
	int getAfterTouchValue() const { return d[2]; }
	bool isSustainPedalOn() const { return isController() && d[1] == 64 && d[2] >= 64; }
	bool isSustainPedalOff() const { return isController() && d[1] == 64 && d[2] < 64; }
	bool isSoftPedalOn() const { return isController() && d[1] == 67 && d[2] >= 64; }
	bool isSoftPedalOff() const { return isController() && d[1] == 67 && d[2] < 64; }
	bool isAllNotesOff() const { return isController() && d[1] == 123; }
	bool isAllSoundOff() const { return isController() && d[1] == 120; }
	bool isMidiClock() const { return d[0] == 0xf8; }
	String getDescription() const { return String(); }
	uint8_t d[3] = {0, 0, 0};
};
class MidiBuffer {
  public:
	struct Event {
		MidiMessage message;
		int samplePosition;
		MidiMessage getMessage() const { return message; }
	};
	using Iter = std::vector<Event>::const_iterator;
	void addEvent(const MidiMessage &m, int pos) { events.push_back({m, pos}); }
	bool isEmpty() const { return events.empty(); }
	void clear() { events.clear(); }
	Iter begin() const { return events.begin(); }
	Iter end() const { return events.end(); }
	std::vector<Event> events;
};
using MidiBufferIterator = MidiBuffer::Iter;
} // namespace juce
