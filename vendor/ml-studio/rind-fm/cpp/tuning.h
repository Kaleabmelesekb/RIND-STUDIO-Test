// SPDX-License-Identifier: GPL-3.0-or-later
// The tuning the engine plays in. Dexed's version can load Scala/KBM files through the Surge tuning library; here it
// is the standard tuning (equal temperament, A = 440 Hz, moved by the master tune).
#ifndef __SYNTH_TUNING_H
#define __SYNTH_TUNING_H

#include <memory>
#include <string>

#include "synth.h"

class TuningState {
public:
    virtual ~TuningState() {}

    virtual int32_t midinote_to_logfreq(int midinote) = 0;
    virtual bool is_standard_tuning() { return true; }
    virtual int scale_length() { return 12; }
    virtual std::string display_tuning_str() { return "Standard Tuning"; }
};

std::shared_ptr<TuningState> createStandardTuning();

#endif
