/*
 * NULL JSFX - Riser, framework-independent DSP core
 * Copyright (C) 2026 Christos Koulaxizis / NULL JSFX
 * SPDX-License-Identifier: MIT
 *
 * Port of DATA/Effects/null_jsfx/riser.jsfx (computed in double, like EEL2): a resonant
 * high-pass sweeping 20 Hz -> 2.5 kHz, a noise band an octave above it, side boost and a
 * cross-fed diffuse wash, all following a 30 ms smoothed intensity. Coefficients follow
 * the smoothed intensity once per block (the JSFX's @block), so process() is one block.
 */

#pragma once

#include <algorithm>
#include <cmath>
#include <cstdint>
#include <vector>

namespace nullriser {

static constexpr double kPi = 3.141592653589793; // EEL2 $pi

// Park-Miller step in exact double arithmetic, as in the JSFX
inline double nextSeed(double seed) noexcept
{
    seed *= 16807.0;
    return seed - std::floor(seed / 2147483647.0) * 2147483647.0;
}

// trapezoidal state-variable filter, as svf_set / svf_hp / svf_bp in the JSFX
struct Svf {
    double g = 0, k = 0, s1 = 0, s2 = 0;
    void set(double f, double q, double srate) noexcept
    {
        g = std::tan(kPi * std::fmin(f, 0.45 * srate) / srate);
        k = 1.0 / q;
    }
    double tick(double x, double& v1) noexcept
    {
        const double v3 = (x - s2 - (k + g) * s1) / (1.0 + g * (k + g));
        v1 = g * v3 + s1;
        const double v2 = g * v1 + s2;
        s1 = 2.0 * v1 - s1;
        s2 = 2.0 * v2 - s2;
        return v3;
    }
    double hp(double x) noexcept { double v1; return tick(x, v1); }
    double bp(double x) noexcept { double v1; tick(x, v1); return k * v1; }
};

// filter settings for an intensity t (0..1), as in @block
struct Settings {
    double a, fc, q, noise, width, wash, fb;
};
inline Settings settingsFor(double amt) noexcept
{
    const double a = amt * amt;
    return { a, 20.0 * std::pow(125.0, amt), 0.707 + 2.3 * a, 0.35 * a, std::pow(10.0, 6.0 * amt / 20.0),
             0.35 * amt, 0.55 * amt };
}

class RiserProcessor {
public:
    // @init (does not touch amt or the filter states, like the JSFX)
    void setSampleRate(double sr)
    {
        fSr = sr;
        fBs = int(std::ceil(sr * 0.12)) + 2;
        fBufL.assign(size_t(fBs), 0.0);
        fBufR.assign(size_t(fBs), 0.0);
        fDl = std::max(1, int(std::floor(0.083 * sr)));
        fDr = std::max(1, int(std::floor(0.097 * sr)));
        fPl = fPr = 0;
        fSm = std::exp(-1.0 / (0.03 * sr));
        fSeed = 12345.0;
    }

    void setAmount(double v) noexcept { fTarget = v / 100.0; } // @slider
    double smoothed() const noexcept { return fAmt; }

    template <typename T>
    void process(const T* inL, const T* inR, T* outL, T* outR, uint32_t frames) noexcept
    {
        // @block
        const Settings s = settingsFor(fAmt);
        fHl.set(s.fc, s.q, fSr);
        fHr.set(s.fc, s.q, fSr);
        fNl.set(s.fc * 2.0 + 200.0, 1.5, fSr);
        fNr.set(s.fc * 2.0 + 200.0, 1.5, fSr);
        const double noise = s.noise, width = s.width, wash = s.wash, fb = s.fb;
        const double sm = fSm, target = fTarget;

        // @sample
        for (uint32_t i = 0; i < frames; ++i)
        {
            double spl0 = inL[i], spl1 = inR[i];
            fAmt = sm * fAmt + (1.0 - sm) * target;
            if (fAmt < 0.0005 && std::fabs(target) < 0.00001)
                fAmt = 0.0;
            else
            {
                fSeed = nextSeed(fSeed);
                const double n1 = fSeed / 1073741824.0 - 1.0;
                fSeed = nextSeed(fSeed);
                const double n2 = fSeed / 1073741824.0 - 1.0;
                double l = fHl.hp(spl0);
                l = l + noise * fNl.bp(n1);
                double r = fHr.hp(spl1);
                r = r + noise * fNr.bp(n2);
                const double m = (l + r) * 0.5, sd = (l - r) * 0.5 * width;
                l = m + sd;
                r = m - sd;
                const double wl = fBufL[size_t(fPl)], wr = fBufR[size_t(fPr)];
                fBufL[size_t(fPl)] = l + wr * fb;
                fBufR[size_t(fPr)] = r + wl * fb;
                fPl = (fPl + 1) % fDl;
                fPr = (fPr + 1) % fDr;
                l += wash * wl;
                r += wash * wr;
                const double x = std::fmin(1.0, fAmt * 20.0);
                spl0 += x * (l - spl0);
                spl1 += x * (r - spl1);
            }
            outL[i] = static_cast<T>(spl0);
            outR[i] = static_cast<T>(spl1);
        }
    }

private:
    double fSr = 48000.0, fSm = 0.0, fSeed = 12345.0, fTarget = 0.0, fAmt = 0.0;
    int fBs = 1, fDl = 1, fDr = 1, fPl = 0, fPr = 0;
    std::vector<double> fBufL, fBufR;
    Svf fHl, fHr, fNl, fNr;
};

} // namespace nullriser
