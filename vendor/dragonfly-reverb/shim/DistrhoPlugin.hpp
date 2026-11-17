// Stand-in for DPF's DistrhoPlugin.hpp, which the original DSP.cpp files include only for `d_isNotEqual`. The rest of
// DPF (plugin formats, the OpenGL UI) is not used: Rind Studio hosts the DSP directly.
#ifndef DRAGONFLY_SHIM_DISTRHO_PLUGIN_HPP_INCLUDED
#define DRAGONFLY_SHIM_DISTRHO_PLUGIN_HPP_INCLUDED

#include <cmath>
#include <cstdint>
#include <limits>

template <typename T>
static inline bool d_isNotEqual(const T& v1, const T& v2) {
  return std::abs(v1 - v2) >= std::numeric_limits<T>::epsilon();
}

template <typename T>
static inline bool d_isEqual(const T& v1, const T& v2) {
  return std::abs(v1 - v2) < std::numeric_limits<T>::epsilon();
}

#endif
