// SPDX-License-Identifier: GPL-3.0-or-later
// What the engine takes from OB-Xf's Utils.h.
#pragma once

#include <core/Constants.h>
#include <engine/ParamScales.h>

inline static float getPitch(float index) { return 440.f * std::exp(mult * index); };
