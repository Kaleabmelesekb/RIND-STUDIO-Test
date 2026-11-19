// Stand-in for the JUCE umbrella header that Vital's synthesis engine, wavetable code and tuning code include.
//
// Vital's own sources use JUCE for only a little in those parts: a string class, a file reader for tuning files, base64,
// a leak-detector macro and one call that turns denormals off. This header is the part they use, written over the C++
// standard library, so the engine builds on any target with a C++ compiler (Android's NDK included) without JUCE's GUI,
// audio device and plugin-format modules. Nothing of Vital itself is changed.
#pragma once

#include <algorithm>
#include <array>
#include <atomic>
#include <cassert>
#include <cmath>
#include <complex>
#include <cstdint>
#include <cstdlib>
#include <cstring>
#include <deque>
#include <fstream>
#include <functional>
#include <list>
#include <map>
#include <memory>
#include <mutex>
#include <random>
#include <set>
#include <sstream>
#include <string>
#include <thread>
#include <utility>
#include <vector>

#if defined(__SSE__) || defined(__x86_64__) || defined(_M_X64)
#include <xmmintrin.h>
#endif

#define JUCE_LEAK_DETECTOR(x)
#define JUCE_DECLARE_NON_COPYABLE_WITH_LEAK_DETECTOR(x) \
  x(const x&) = delete;                                  \
  x& operator=(const x&) = delete;
#define JUCE_DECLARE_NON_COPYABLE(x) \
  x(const x&) = delete;              \
  x& operator=(const x&) = delete;

// The `synthesis` unity build includes phaser_filter.cpp before futils is declared where phaser_filter.h uses it; JUCE's
// own headers do not provide it either, the unity build's include order does. Nothing to add here.

class FloatVectorOperations {
public:
  /// Flushes denormals to zero on this thread, as JUCE's does (SSE's FTZ and DAZ bits; FPCR.FZ on 64-bit ARM).
  static void disableDenormalisedNumberSupport() noexcept {
#if defined(__SSE__) || defined(__x86_64__) || defined(_M_X64)
    _mm_setcsr(_mm_getcsr() | 0x8040);
#elif defined(__aarch64__)
    uint64_t fpcr;
    __asm__ __volatile__("mrs %0, fpcr" : "=r"(fpcr));
    fpcr |= (1ULL << 24);
    __asm__ __volatile__("msr fpcr, %0" : : "r"(fpcr));
#endif
  }
};

class StringArray;

class String {
public:
  String() = default;
  String(const char* text) : text_(text ? text : "") {}
  String(const std::string& text) : text_(text) {}
  String(char c) : text_(1, c) {}

  std::string toStdString() const { return text_; }
  const char* toRawUTF8() const { return text_.c_str(); }
  int length() const { return static_cast<int>(text_.size()); }
  bool isEmpty() const { return text_.empty(); }
  bool isNotEmpty() const { return !text_.empty(); }
  char operator[](int index) const {
    return index >= 0 && index < length() ? text_[static_cast<size_t>(index)] : '\0';
  }

  String operator+(const String& other) const { return String(text_ + other.text_); }
  String& operator+=(const String& other) { text_ += other.text_; return *this; }
  bool operator==(const String& other) const { return text_ == other.text_; }
  bool operator!=(const String& other) const { return text_ != other.text_; }
  bool operator<(const String& other) const { return text_ < other.text_; }

  String substring(int start) const {
    start = std::max(0, std::min(start, length()));
    return String(text_.substr(static_cast<size_t>(start)));
  }
  String substring(int start, int end) const {
    start = std::max(0, std::min(start, length()));
    end = std::max(start, std::min(end, length()));
    return String(text_.substr(static_cast<size_t>(start), static_cast<size_t>(end - start)));
  }
  String trim() const {
    size_t a = text_.find_first_not_of(" \t\r\n");
    if (a == std::string::npos)
      return String();
    size_t b = text_.find_last_not_of(" \t\r\n");
    return String(text_.substr(a, b - a + 1));
  }
  String toLowerCase() const {
    std::string out = text_;
    std::transform(out.begin(), out.end(), out.begin(), [](unsigned char c) { return std::tolower(c); });
    return String(out);
  }
  String removeCharacters(const String& unwanted) const {
    std::string out;
    for (char c : text_)
      if (unwanted.text_.find(c) == std::string::npos)
        out += c;
    return String(out);
  }
  String retainCharacters(const String& keep) const {
    std::string out;
    for (char c : text_)
      if (keep.text_.find(c) != std::string::npos)
        out += c;
    return String(out);
  }
  String upToFirstOccurrenceOf(const String& sub, bool include, bool ignoreCase) const {
    size_t at = find(sub, ignoreCase, false);
    if (at == std::string::npos)
      return *this;
    return String(text_.substr(0, at + (include ? sub.text_.size() : 0)));
  }
  String upToLastOccurrenceOf(const String& sub, bool include, bool ignoreCase) const {
    size_t at = find(sub, ignoreCase, true);
    if (at == std::string::npos)
      return *this;
    return String(text_.substr(0, at + (include ? sub.text_.size() : 0)));
  }
  String fromFirstOccurrenceOf(const String& sub, bool include, bool ignoreCase) const {
    size_t at = find(sub, ignoreCase, false);
    if (at == std::string::npos)
      return String();
    return String(text_.substr(at + (include ? 0 : sub.text_.size())));
  }
  String fromLastOccurrenceOf(const String& sub, bool include, bool ignoreCase) const {
    size_t at = find(sub, ignoreCase, true);
    if (at == std::string::npos)
      return *this;
    return String(text_.substr(at + (include ? 0 : sub.text_.size())));
  }
  bool startsWith(const String& prefix) const { return text_.compare(0, prefix.text_.size(), prefix.text_) == 0; }
  bool startsWithChar(char c) const { return !text_.empty() && text_[0] == c; }
  bool endsWith(const String& suffix) const {
    return text_.size() >= suffix.text_.size() &&
           text_.compare(text_.size() - suffix.text_.size(), suffix.text_.size(), suffix.text_) == 0;
  }
  bool contains(const String& sub) const { return text_.find(sub.text_) != std::string::npos; }
  bool containsChar(char c) const { return text_.find(c) != std::string::npos; }
  int indexOf(const String& sub) const {
    size_t at = text_.find(sub.text_);
    return at == std::string::npos ? -1 : static_cast<int>(at);
  }
  int indexOfChar(char c) const {
    size_t at = text_.find(c);
    return at == std::string::npos ? -1 : static_cast<int>(at);
  }
  String replace(const String& from, const String& to) const {
    if (from.text_.empty())
      return *this;
    std::string out;
    size_t at = 0;
    for (;;) {
      size_t next = text_.find(from.text_, at);
      if (next == std::string::npos) {
        out += text_.substr(at);
        break;
      }
      out += text_.substr(at, next - at);
      out += to.text_;
      at = next + from.text_.size();
    }
    return String(out);
  }
  int getIntValue() const { return static_cast<int>(std::strtol(text_.c_str(), nullptr, 10)); }
  float getFloatValue() const { return static_cast<float>(std::strtod(text_.c_str(), nullptr)); }
  double getDoubleValue() const { return std::strtod(text_.c_str(), nullptr); }
  bool containsOnly(const String& chars) const {
    for (char c : text_)
      if (chars.text_.find(c) == std::string::npos)
        return false;
    return true;
  }
  bool equalsIgnoreCase(const String& other) const { return toLowerCase() == other.toLowerCase(); }

  const std::string& str() const { return text_; }

private:
  size_t find(const String& sub, bool ignoreCase, bool last) const {
    const std::string hay = ignoreCase ? toLowerCase().text_ : text_;
    const std::string needle = ignoreCase ? sub.toLowerCase().text_ : sub.text_;
    return last ? hay.rfind(needle) : hay.find(needle);
  }

  std::string text_;
};

class StringArray {
public:
  StringArray() = default;
  int size() const { return static_cast<int>(strings_.size()); }
  void add(const String& s) { strings_.push_back(s); }
  void clear() { strings_.clear(); }
  String operator[](int index) const {
    return index >= 0 && index < size() ? strings_[static_cast<size_t>(index)] : String();
  }
  std::vector<String>::const_iterator begin() const { return strings_.begin(); }
  std::vector<String>::const_iterator end() const { return strings_.end(); }

  /// Splits at white space.
  int addTokens(const String& text, bool /*preserveQuotedStrings*/) { return addTokens(text, " \t\r\n", ""); }
  /// Splits at any of `breakCharacters`; empty tokens are not added.
  int addTokens(const String& text, const String& breakCharacters, const String& /*quoteCharacters*/) {
    int added = 0;
    std::string token;
    for (char c : text.str()) {
      if (breakCharacters.str().find(c) != std::string::npos) {
        if (!token.empty()) {
          strings_.push_back(String(token));
          ++added;
        }
        token.clear();
      } else {
        token += c;
      }
    }
    if (!token.empty()) {
      strings_.push_back(String(token));
      ++added;
    }
    return added;
  }

private:
  std::vector<String> strings_;
};

class File {
public:
  File() = default;
  File(const String& path) : path_(path) {}

  String getFileExtension() const {
    size_t dot = path_.str().rfind('.');
    size_t slash = path_.str().find_last_of("/\\");
    if (dot == std::string::npos || (slash != std::string::npos && dot < slash))
      return String();
    return String(path_.str().substr(dot));
  }
  String getFileNameWithoutExtension() const {
    std::string name = path_.str();
    size_t slash = name.find_last_of("/\\");
    if (slash != std::string::npos)
      name = name.substr(slash + 1);
    size_t dot = name.rfind('.');
    return String(dot == std::string::npos ? name : name.substr(0, dot));
  }
  /// Reads the file's lines into `lines`.
  void readLines(StringArray& lines) const {
    std::ifstream in(path_.str());
    std::string line;
    while (std::getline(in, line)) {
      if (!line.empty() && line.back() == '\r')
        line.pop_back();
      lines.add(String(line));
    }
  }
  bool existsAsFile() const { return std::ifstream(path_.str()).good(); }

private:
  String path_;
};

class MemoryOutputStream {
public:
  MemoryOutputStream() = default;
  explicit MemoryOutputStream(size_t initialSize) { data_.reserve(initialSize); }
  bool write(const void* source, size_t size) {
    const auto* bytes = static_cast<const uint8_t*>(source);
    data_.insert(data_.end(), bytes, bytes + size);
    return true;
  }
  const void* getData() const { return data_.data(); }
  size_t getDataSize() const { return data_.size(); }

private:
  std::vector<uint8_t> data_;
};

class Base64 {
public:
  /// JUCE's base64: the standard alphabet with no padding characters added.
  static String toBase64(const void* data, size_t size) {
    static const char kAlphabet[] = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    const auto* in = static_cast<const uint8_t*>(data);
    std::string out;
    out.reserve((size + 2) / 3 * 4);
    for (size_t i = 0; i < size; i += 3) {
      uint32_t chunk = static_cast<uint32_t>(in[i]) << 16;
      if (i + 1 < size)
        chunk |= static_cast<uint32_t>(in[i + 1]) << 8;
      if (i + 2 < size)
        chunk |= in[i + 2];
      out += kAlphabet[(chunk >> 18) & 63];
      out += kAlphabet[(chunk >> 12) & 63];
      if (i + 1 < size)
        out += kAlphabet[(chunk >> 6) & 63];
      if (i + 2 < size)
        out += kAlphabet[chunk & 63];
    }
    return String(out);
  }

  static bool convertFromBase64(MemoryOutputStream& out, const String& text) {
    uint32_t bits = 0;
    int count = 0;
    for (char c : text.str()) {
      int v;
      if (c >= 'A' && c <= 'Z') v = c - 'A';
      else if (c >= 'a' && c <= 'z') v = c - 'a' + 26;
      else if (c >= '0' && c <= '9') v = c - '0' + 52;
      else if (c == '+') v = 62;
      else if (c == '/') v = 63;
      else if (c == '=' || c == ' ' || c == '\n' || c == '\r') continue;
      else return false;
      bits = (bits << 6) | static_cast<uint32_t>(v);
      if (++count == 4) {
        uint8_t bytes[3] = {static_cast<uint8_t>(bits >> 16), static_cast<uint8_t>(bits >> 8), static_cast<uint8_t>(bits)};
        out.write(bytes, 3);
        bits = 0;
        count = 0;
      }
    }
    if (count == 2) {
      uint8_t byte = static_cast<uint8_t>(bits >> 4);
      out.write(&byte, 1);
    } else if (count == 3) {
      uint8_t bytes[2] = {static_cast<uint8_t>(bits >> 10), static_cast<uint8_t>(bits >> 2)};
      out.write(bytes, 2);
    }
    return true;
  }
};

// What `ProjectInfo` holds in Vital's build (Projucer writes it).
namespace ProjectInfo {
const char* const projectName = "Vital";
const char* const companyName = "Vital Audio";
const char* const versionString = "1.0.6";
const int versionNumber = 0x10006;
}  // namespace ProjectInfo
