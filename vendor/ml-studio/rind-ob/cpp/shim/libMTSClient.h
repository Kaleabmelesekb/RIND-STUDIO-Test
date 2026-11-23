// SPDX-License-Identifier: GPL-3.0-or-later
// OB-Xf can follow an MTS-ESP master for its tuning. Honeydew has none: this stands in for the client library and
// says there is no master, so the notes play in twelve-tone equal temperament.
#pragma once

struct MTSClient {};

static inline MTSClient *MTS_RegisterClient() { return nullptr; }
static inline void MTS_DeregisterClient(MTSClient *) {}
static inline bool MTS_HasMaster(MTSClient *) { return false; }
static inline const char *MTS_GetScaleName(MTSClient *) { return ""; }
static inline double MTS_RetuningInSemitones(MTSClient *, char, signed char) { return 0.0; }
