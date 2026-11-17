// Stand-in for DPF's extra/ScopedDenormalDisable.hpp: flush denormals to zero while a scope is alive, as DPF does
// (SSE's FTZ and DAZ bits on x86, FPCR.FZ on 64-bit ARM; nothing elsewhere).
#ifndef DRAGONFLY_SHIM_SCOPED_DENORMAL_DISABLE_HPP_INCLUDED
#define DRAGONFLY_SHIM_SCOPED_DENORMAL_DISABLE_HPP_INCLUDED

#include <cstdint>

#if defined(__SSE__) || defined(__x86_64__) || defined(_M_X64)
#include <xmmintrin.h>
#endif

class ScopedDenormalDisable {
public:
  ScopedDenormalDisable() noexcept {
#if defined(__SSE__) || defined(__x86_64__) || defined(_M_X64)
    fSaved = _mm_getcsr();
    _mm_setcsr(fSaved | 0x8040); // FTZ | DAZ
#elif defined(__aarch64__)
    uint64_t fpcr;
    __asm__ __volatile__("mrs %0, fpcr" : "=r"(fpcr));
    fSaved = fpcr;
    fpcr |= (1ULL << 24); // FZ
    __asm__ __volatile__("msr fpcr, %0" : : "r"(fpcr));
#endif
  }
  ~ScopedDenormalDisable() noexcept {
#if defined(__SSE__) || defined(__x86_64__) || defined(_M_X64)
    _mm_setcsr(static_cast<unsigned int>(fSaved));
#elif defined(__aarch64__)
    __asm__ __volatile__("msr fpcr, %0" : : "r"(fSaved));
#endif
  }
  ScopedDenormalDisable(const ScopedDenormalDisable&) = delete;
  ScopedDenormalDisable& operator=(const ScopedDenormalDisable&) = delete;

private:
  uint64_t fSaved = 0;
};

#endif
