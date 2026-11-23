// SPDX-License-Identifier: GPL-3.0-or-later
#pragma once

#include <juce_core/juce_core.h>

namespace juce::dsp::FastMathApproximations
{
// JUCE's polynomial sine for -pi..pi.
template <typename FloatType> inline FloatType sin(FloatType x) noexcept
{
    auto x2 = x * x;
    auto numerator = -x * (FloatType(-11511339840) + x2 * (FloatType(1640635920) + x2 * (FloatType(-52785432) + x2 * FloatType(479249))));
    auto denominator = FloatType(11511339840) + x2 * (FloatType(277920720) + x2 * (FloatType(3177720) + x2 * FloatType(18361)));
    return numerator / denominator;
}
} // namespace juce::dsp::FastMathApproximations
