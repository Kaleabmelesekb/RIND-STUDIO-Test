// SPDX-License-Identifier: GPL-3.0-or-later

use os_engine::effects::eq::HalfBand;

pub(crate) const OVERSAMPLING: [&str; 3] = ["1x", "2x", "4x"];

#[derive(Default)]
pub(crate) struct Oversampler {
    stages: usize,
    up: [[HalfBand; 2]; 2],
    down: [[HalfBand; 2]; 2],
}

impl Oversampler {
    pub fn set(&mut self, value: i32) {
        let stages = value.clamp(0, 2) as usize;
        if self.stages != stages {
            self.stages = stages;
            self.reset();
        }
    }

    pub fn reset(&mut self) {
        self.up = Default::default();
        self.down = Default::default();
    }

    pub fn process(&mut self, left: &mut [f32], right: &mut [f32], shape: impl Fn(f32) -> f32) {
        for (channel, samples) in [left, right].into_iter().enumerate() {
            match self.stages {
                0 => samples.iter_mut().for_each(|x| *x = shape(*x)),
                1 => {
                    let (up, down) = (&mut self.up[channel][0], &mut self.down[channel][0]);
                    for x in samples {
                        let [a, b] = up.up(*x);
                        *x = down.down(shape(a), shape(b));
                    }
                }
                _ => {
                    let [up, up_high] = &mut self.up[channel];
                    let [down, down_high] = &mut self.down[channel];
                    for x in samples {
                        let [a, b] = up.up(*x).map(|v| {
                            let [a, b] = up_high.up(v);
                            down_high.down(shape(a), shape(b))
                        });
                        *x = down.down(a, b);
                    }
                }
            }
        }
        if self.stages > 0 {
            for channel in 0..2 {
                for stage in 0..self.stages {
                    self.up[channel][stage].tidy();
                    self.down[channel][stage].tidy();
                }
            }
        }
    }
}
