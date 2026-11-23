// SPDX-License-Identifier: GPL-3.0-or-later
// The few JUCE things OB-Xf's engine uses, in plain C++.
#pragma once

#include <algorithm>
#include <atomic>
#include <cmath>
#include <cstdint>
#include <memory>
#include <random>
#include <iostream>
#include <sstream>
#include <string>
#include <unordered_map>
#include <unordered_set>
#include <vector>
#include <array>
#include <set>
#include <map>
#include <functional>
#include <cassert>

namespace juce
{
template <typename T> constexpr T jlimit(T lo, T hi, T v) { return v < lo ? lo : (v > hi ? hi : v); }
template <typename T> constexpr T jmax(T a, T b) { return a > b ? a : b; }
template <typename T> constexpr T jmin(T a, T b) { return a < b ? a : b; }
template <typename T> int roundToInt(T v) { return static_cast<int>(std::floor(v + static_cast<T>(0.5))); }

template <typename T> struct MathConstants
{
    static constexpr T pi = static_cast<T>(3.141592653589793238L);
    static constexpr T twoPi = static_cast<T>(6.283185307179586477L);
    static constexpr T halfPi = static_cast<T>(1.570796326794896619L);
};

class Random
{
  public:
    Random() : engine(std::random_device{}()) {}
    explicit Random(int64_t seed) : engine(static_cast<uint64_t>(seed)) {}
    void setSeed(int64_t seed) { engine.seed(static_cast<uint64_t>(seed)); }
    static Random &getSystemRandom()
    {
        static thread_local Random r;
        return r;
    }
    float nextFloat() { return static_cast<float>(engine() >> 40) * (1.0f / 16777216.0f); }
    double nextDouble() { return static_cast<double>(engine() >> 11) * (1.0 / 9007199254740992.0); }
    int nextInt() { return static_cast<int>(engine()); }
    bool nextBool() { return (engine() & 1) != 0; }
    int64_t nextInt64() { return static_cast<int64_t>(engine()); }

  private:
    std::mt19937_64 engine;
};

// The cursor arithmetic of a single-reader, single-writer ring buffer.
class AbstractFifo
{
  public:
    explicit AbstractFifo(int capacity) : size(capacity) {}
    int getFreeSpace() const { return size - getNumReady() - 1; }
    int getNumReady() const
    {
        const int vs = validStart.load(), ve = validEnd.load();
        return ve >= vs ? ve - vs : size - (vs - ve);
    }
    void reset()
    {
        validEnd = 0;
        validStart = 0;
    }
    void prepareToWrite(int num, int &start1, int &size1, int &start2, int &size2) const
    {
        const int ve = validEnd.load();
        const int free = getFreeSpace();
        num = std::min(num, free);
        start1 = ve;
        if (num <= 0)
        {
            size1 = size2 = start2 = 0;
            return;
        }
        size1 = std::min(num, size - ve);
        size2 = num - size1;
        start2 = 0;
    }
    void finishedWrite(int num) { validEnd = (validEnd.load() + num) % size; }
    void prepareToRead(int num, int &start1, int &size1, int &start2, int &size2) const
    {
        const int vs = validStart.load();
        num = std::min(num, getNumReady());
        start1 = vs;
        if (num <= 0)
        {
            size1 = size2 = start2 = 0;
            return;
        }
        size1 = std::min(num, size - vs);
        size2 = num - size1;
        start2 = 0;
    }
    void finishedRead(int num) { validStart = (validStart.load() + num) % size; }

    struct ScopedWrite
    {
        ScopedWrite(AbstractFifo &f, int n) : fifo(f)
        {
            fifo.prepareToWrite(n, startIndex1, blockSize1, startIndex2, blockSize2);
            count = blockSize1 + blockSize2;
        }
        ~ScopedWrite() { fifo.finishedWrite(count); }
        AbstractFifo &fifo;
        int startIndex1{0}, blockSize1{0}, startIndex2{0}, blockSize2{0}, count{0};
    };
    struct ScopedRead
    {
        ScopedRead(AbstractFifo &f, int n) : fifo(f)
        {
            fifo.prepareToRead(n, startIndex1, blockSize1, startIndex2, blockSize2);
            count = blockSize1 + blockSize2;
        }
        ~ScopedRead() { fifo.finishedRead(count); }
        AbstractFifo &fifo;
        int startIndex1{0}, blockSize1{0}, startIndex2{0}, blockSize2{0}, count{0};
    };
    ScopedWrite write(int n) { return ScopedWrite(*this, n); }
    ScopedRead read(int n) { return ScopedRead(*this, n); }

  private:
    int size;
    std::atomic<int> validStart{0}, validEnd{0};
};
} // namespace juce

#define JUCE_DECLARE_NON_COPYABLE(Class) \
    Class(const Class &) = delete;       \
    Class &operator=(const Class &) = delete;
#define JUCE_DECLARE_NON_MOVEABLE(Class) \
    Class(Class &&) = delete;            \
    Class &operator=(Class &&) = delete;
