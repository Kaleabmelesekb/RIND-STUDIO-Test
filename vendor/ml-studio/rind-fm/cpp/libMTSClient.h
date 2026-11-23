// SPDX-License-Identifier: GPL-3.0-or-later
// Dexed can follow an MTS-ESP master for its tuning. Honeydew has none: this stands in for the client library, and
// says there is no master, so the notes are tuned as the DX7 tunes them.
#pragma once

struct MTSClient {};

static inline bool MTS_HasMaster(MTSClient*) { return false; }
static inline double MTS_NoteToFrequency(MTSClient*, int, int) { return 0.0; }
