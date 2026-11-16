# Rind: DSP notes

Rind is the virtual analog synthesizer inside Rind Studio (`crates/rind`). This page says what the audio engine does, in
which units, and what it does not do. Every number below is checked by a test in the crate; `cargo test -p rind` runs
them (the host app's CI does).

## Architecture

- **Voices.** 16 preallocated voices (`MAX_VOICES`); no allocation, lock or I/O on the audio thread. A note takes a free voice; at the polyphony limit it takes over the quietest released
  voice, else the oldest. Mono mode keeps one voice with legato and glide.
- **Signal path per voice.** Oscillators A1/A2 (part A) and B1/B2 (part B), each with up to 8 unison voices,
  summed per oscillator -> filters -> amplitude envelope -> pan -> effects.
- **Control rate.** Modulation, envelopes of the matrix and filter coefficients are computed every 16 samples (`STEP`);
  oscillators, the amplitude envelope and the filters run every sample.
- **Parameters.** One flat `Snapshot` of plain values. Each parameter is smoothed with a one-pole low pass of 5 ms per
  control step (`smooth_coef`), so a knob move never steps; integer parameters (waves, types, voices) jump.
  `ids.rs` is frozen: ids are never renamed, reordered or reused, only appended.
- **Modulation matrix.** 16 slots, 14 sources (LFO 1-2, envelopes 3-4, velocity, key, mod wheel, pitch bend, aftertouch,
  random, step velocity, part levels) and 40 targets. Pitch amounts scale by 24 semitones, cutoff by 60.

## Oscillators (`osc.rs`)

| Wave | Name | Notes |
|---|---|---|
| 0-7 | Sine, Saw, Triangle, Pulse 50, Pulse 25, Pulse 12.5, TriSaw, Noise | unchanged since the first release |
| 8 | PWM | pulse; width = 0.5 + 0.45 x (Width mod), clamped to 5-95 %, DC-free |
| 9 | Saw+Sub | saw plus a sine one octave down |
| 10 | Pulse+Sub | PWM pulse plus a square one octave down |
| 11 | SuperSaw | saw with the supersaw unison law |

- Saw and pulse use PolyBLEP, triangle uses PolyBLAMP; frequency is clamped below 0.45 of the sample rate.
- Pulse width is made safe at any pitch: edges are kept `2 dt` apart, and above `dt = 0.25` the width falls back to 50 %.
- **Unison**: 1-8 voices, detune in semitones at the outer voices, stereo spread symmetric around the pan, random or fixed
  start phase, equal-power normalisation (`1 / sqrt(sum w^2)`) so the level does not change with the voice count.
  SuperSaw detune offsets follow `sign(x)|x|^1.6` (a 7-voice stack sits at 0, 0.18, 0.52, 1 of the widest) and the
  centre voice is louder than the sides (weight `1 - 0.45|x|`).
- **Sub oscillator**: one per oscillator whatever the unison count, level 0.6 of the oscillator's own sum, phase restarts with the note.
- **Width targets** (`Width A1`, `A2`, `B1`, `B2`, `All`, mod targets 35-39): +-1 moves the pulse edge by 0.45 of the period.

## Filter (`filter.rs`)

Types: 0 Low, 1 Band, 2 High (state-variable, zero delay feedback, 12 or 24 dB) and **3 Ladder, 4 LadBP, 5 LadHP**.

The ladder is a Moog-style four-pole transistor ladder built from trapezoidal (TPT) one-pole stages with the feedback
loop solved in the same sample (no unit delay), and a saturation `tanh` in front of every stage.

- `g = tan(pi fc / fs)`, `G = g / (1 + g)`; stage `y = G x' + s / (1 + g)`, `s <- 2 y - s`.
- Feedback gain `k = 4.1 res`: `res = 1` self-oscillates at the cutoff (amplitude about 0.2, bounded by the saturation).
- The saturation is evaluated with the rational `tanh` of `util::soft`; the loop is solved twice (a linear prediction of each
  stage's input, then exactly for those saturation gains), so the response matches the discrete linear loop for small
  signals (tested against the analytic response within 4 %) and compresses loud ones.
- Slope switch: 24 dB uses the 4th stage, 12 dB the 2nd. LadBP is `4 (y2 - 2 y3 + y4)` (peak gain 1 at the cutoff), LadHP
  `u - 4 y1 + 6 y2 - 4 y3 + y4`.
- `drive` (0-1) multiplies the input by up to 6 into the stages and gives back part of the level.
- Passband make-up `1.25 (1 + 0.5 k)`: the `0.5 k` returns some of the low-frequency gain the feedback removes, and 1.25 puts the
  ladder at the level of the state-variable filter (the ladder was 2 dB quieter at the median over the 361 original patches; now -0.2 dB).
- Denormals: stage states below 1e-18 are flushed to zero.

Measured (48 kHz): 24 dB per octave well below Nyquist (23-25 dB measured for 800 -> 1600 Hz at `fc = 100`), -12 dB at the cutoff,
self-oscillation within 4 % of the cutoff from 200 Hz to 4 kHz, bounded output under audio-rate cutoff modulation from
20 Hz to 21.6 kHz at full resonance and drive.

### Limits

- **No oversampling.** A 2x oversampled run (31-tap half-band) lowers the aliasing of a driven sine by 27-33 dB
  (cutoff 12 kHz, drive 1: -27 dB at 1x, -55 dB at 2x; drive 0.5: -49 dB, -81 dB) at several times the filter cost on 16
  voices; it was measured in `filter::tests::oversampling_the_ladder_is_measured_before_it_is_chosen` and left off. Bright,
  hard-driven patches show the aliasing.
- The saturation prediction is one pass, not a converged Newton solve.
- The state-variable filters (types 0-2) behave exactly as before; patches that name one (the vowel-like band-pass patches) keep it.

## Envelopes (`env.rs`)

Exponential ADSR. Attack aims at 1.3 so it reaches 1 in exactly the attack time; decay and release cover 60 dB in their time;
sustain tracks its parameter. A new gate starts from the level it finds (no click). Times are in seconds, from 0.1 ms; the
coefficients are computed for the real rate, and tests check the stage times at 22.05, 44.1, 48, 96 and 192 kHz. The amplitude
envelope and the filter/mod envelopes (3, 4) are separate instances.

## Compatibility

Appended, never changed: wave values 8-11, filter types 3-5, mod targets 35-39 (and the wave/type/target ranges that name
them); ids and their order are frozen. **The default filter is now the Ladder** (`f1_type`, `f2_type` default 3, and `presets::BASE`
has `ft: 3`), so Init and every factory patch that does not name a state-variable filter plays through it: the original 361
patches sound different from the earlier release (median level -0.2 dB, 90 % within -1.1 to +0.2 dB). Projects that saved their
own filter type keep it. Patches 1-361 are the original library; 362-3049 are the genre library (generated by
`tools/gen_rind_presets.py` from `tools/rind_genres.py`).

## Tests

`cargo test -p rind` covers: oscillator waveform, pitch, aliasing against a naive saw, pulse width and DC, extreme widths,
sub level, supersaw symmetry and power; filter response against the analytic loop, slopes, resonance, self-oscillation, saturation,
stability, denormals, cost; envelope timing, retrigger and extremes; the engine with every wave and filter type at maximum
unison, resonance and drive; the preset library.
