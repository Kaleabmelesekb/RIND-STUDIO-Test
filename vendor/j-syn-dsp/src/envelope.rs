#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdsrStage {
    Idle,
    Attack,
    Decay,
    Sustain,
    Release,
}

/// Parameters for an ADSR envelope generator.
#[derive(Debug, Clone, Copy)]
pub struct AdsrParams {
    pub attack_s: f32,
    pub decay_s: f32,
    pub sustain: f32,
    pub release_s: f32,
}

impl Default for AdsrParams {
    fn default() -> Self {
        Self {
            attack_s: 0.01,
            decay_s: 0.30,
            sustain: 0.50,
            release_s: 0.25,
        }
    }
}

/// Evaluates an approximate ADSR curve amplitude in `[0.0, 1.0]` over normalized preview time
/// `t_norm in 0.0..=1.0` for the UI envelope display.
pub fn evaluate_adsr_preview(params: &AdsrParams, t_norm: f32) -> f32 {
    let atk = params.attack_s.clamp(0.001, 5.0);
    let dec = params.decay_s.clamp(0.002, 5.0);
    let sus = params.sustain.clamp(0.0, 1.0);
    let rel = params.release_s.clamp(0.002, 8.0);
    let hold = 0.25 * (atk + dec + rel).clamp(0.05, 1.0);
    let total = (atk + dec + hold + rel).max(0.01);
    let t = t_norm.clamp(0.0, 1.0) * total;

    if t < atk {
        let x = t / atk;
        let curve = (1.0 - (-1.609_437_9 * x).exp()) / 0.8;
        curve.clamp(0.0, 1.0)
    } else if t < atk + dec {
        let x = (t - atk) / dec;
        let curve = (-6.907_755_4 * x).exp();
        (sus + (1.0 - sus) * curve).clamp(0.0, 1.0)
    } else if t < atk + dec + hold {
        sus
    } else {
        let x = (t - atk - dec - hold) / rel;
        (sus * (-9.210_34 * x).exp()).clamp(0.0, 1.0)
    }
}

/// Per-voice exponential/concave ADSR amplitude envelope.
#[derive(Debug, Clone, Copy)]
pub struct AmpAdsr {
    stage: AdsrStage,
    level: f32,
}

impl Default for AmpAdsr {
    fn default() -> Self {
        Self::new()
    }
}

impl AmpAdsr {
    pub fn new() -> Self {
        Self {
            stage: AdsrStage::Idle,
            level: 0.0,
        }
    }

    #[inline]
    pub fn trigger(&mut self) {
        self.stage = AdsrStage::Attack;
    }

    #[inline]
    pub fn reset_and_trigger(&mut self) {
        self.level = 0.0;
        self.stage = AdsrStage::Attack;
    }

    #[inline]
    pub fn release(&mut self) {
        if self.stage != AdsrStage::Idle {
            self.stage = AdsrStage::Release;
        }
    }

    #[inline]
    pub fn reset(&mut self) {
        self.stage = AdsrStage::Idle;
        self.level = 0.0;
    }

    #[inline]
    pub fn is_active(&self) -> bool {
        self.stage != AdsrStage::Idle
    }

    #[inline]
    pub fn level(&self) -> f32 {
        self.level
    }

    #[inline]
    pub fn stage(&self) -> AdsrStage {
        self.stage
    }

    /// Advances the envelope by 1 sample and returns the current amplitude in `[0.0, 1.0]`.
    #[inline]
    pub fn process(
        &mut self,
        sample_rate: f32,
        attack_s: f32,
        decay_s: f32,
        sustain: f32,
        release_s: f32,
    ) -> f32 {
        let sr = sample_rate.max(1000.0);
        let sus = sustain.clamp(0.0, 1.0);

        match self.stage {
            AdsrStage::Idle => {
                self.level = 0.0;
            }
            AdsrStage::Attack => {
                let atk_samples = (attack_s.max(0.001) * sr).max(1.0);
                // Concave/exponential rise targeting 1.25 so it crosses 1.0 at exact attack time.
                const TARGET: f32 = 1.25;
                // ln(1.25 / 0.25) = ln(5) ≈ 1.6094379
                let coeff = 1.0 - (-1.609_437_9 / atk_samples).exp();
                self.level += (TARGET - self.level) * coeff;
                if self.level >= 1.0 {
                    self.level = 1.0;
                    self.stage = AdsrStage::Decay;
                }
            }
            AdsrStage::Decay => {
                let dec_samples = (decay_s.max(0.002) * sr).max(1.0);
                // -60 dB decay constant: ln(1000) ≈ 6.907755
                let coeff = (-6.907_755_4 / dec_samples).exp();
                self.level = sus + (self.level - sus) * coeff;
                if (self.level - sus).abs() <= 1e-4 {
                    self.level = sus;
                    self.stage = AdsrStage::Sustain;
                }
            }
            AdsrStage::Sustain => {
                // Track sustain parameter changes smoothly.
                self.level += (sus - self.level) * 0.01;
            }
            AdsrStage::Release => {
                let rel_samples = (release_s.max(0.002) * sr).max(1.0);
                // -80 dB release constant: ln(10000) ≈ 9.21034
                let coeff = (-9.210_34 / rel_samples).exp();
                self.level *= coeff;
                if self.level <= 3e-5 {
                    self.level = 0.0;
                    self.stage = AdsrStage::Idle;
                }
            }
        }

        self.level
    }
}
