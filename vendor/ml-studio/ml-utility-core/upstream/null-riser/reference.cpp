#include "RiserDSP.hpp"
#include <cstdio>
int main() {
    for (double rate : {8000., 48000., 192000.}) {
        nullriser::RiserProcessor dsp;
        dsp.setSampleRate(rate);
        int sample = 0;
        for (int block = 0; block < 240; ++block) {
            double amount = block < 8 ? 0. : block < 48 ? 25. : block < 96 ? 100. : block < 128 ? 40. : 0.;
            dsp.setAmount(amount);
            float l[512], r[512];
            for (int i = 0; i < 512; ++i, ++sample) {
                l[i] = float(sample % 97 - 48) / 256.f;
                r[i] = float(sample % 73 - 36) / 384.f;
            }
            dsp.process(l, r, l, r, 512);
            for (int i : {0, 71, 255, 511})
                std::printf("%.0f,%d,%d,%.9g,%.9g\n", rate, block, i, l[i], r[i]);
        }
    }
}

