use crate::envelope::{AdsrParams, AmpAdsr};
use crate::filter::{
    cutoff_hz_to_norm, cutoff_norm_to_hz, FilterParams, FilterState, FilterSvfCore,
};
use crate::fx::{FxBusParams, FxBusTopology, FxSlotParams, SubBusParams};
use crate::insert::{
    DirectDest, FilterBusDest, FilterRouting, InsertPosition, InsertShaperState, RoutingParams,
    VoiceFeedbackState,
};
use crate::mod_matrix::{
    ModDestination, ModMatrixState, NUM_ACTIVE_SOURCES, NUM_MOD_DESTS,
};
use crate::modseq::{ModSeqParams, ModSeqRetrig, ModSeqState, ModSeqStepBank};
use crate::noise::{NoiseParams, NoiseState};
use crate::oscillator::{
    osc_hz_offset_from_norm, osc_hz_offset_to_norm, EngineMode, ModSource, OscParams,
    OscillatorState,
};
use crate::wavetable::Wavetable;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolyMode {
    Poly = 0,
    MonoLegato = 1,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct VoiceBusOutput {
    pub bus_a_l: f32,
    pub bus_a_r: f32,
    pub bus_b_l: f32,
    pub bus_b_r: f32,
    pub sub_l: f32,
    pub sub_r: f32,
}

impl VoiceBusOutput {
    #[inline]
    pub fn scale(self, gain: f32) -> Self {
        Self {
            bus_a_l: self.bus_a_l * gain,
            bus_a_r: self.bus_a_r * gain,
            bus_b_l: self.bus_b_l * gain,
            bus_b_r: self.bus_b_r * gain,
            sub_l: self.sub_l * gain,
            sub_r: self.sub_r * gain,
        }
    }
}

/// Snapshot of all synthesizer DSP parameters for a processing block.
#[derive(Debug, Clone, Copy)]
pub struct SynthParams {
    pub master_gain: f32,
    pub engine_mode: EngineMode,
    pub poly_mode: PolyMode,
    pub glide_ms: f32,
    pub pitch_bend_range: f32,
    pub mpe_enabled: bool,
    pub mpe_pitch_bend_range: f32,
    pub amp_attack_s: f32,
    pub amp_decay_s: f32,
    pub amp_sustain: f32,
    pub amp_release_s: f32,
    pub oscs: [OscParams; 4],
    pub noise: NoiseParams,
    pub filters: [FilterParams; 2],
    pub routing: RoutingParams,
    pub mod_envs: [AdsrParams; 3],
    pub modseqs: [ModSeqParams; 4],
    pub modseq_steps: ModSeqStepBank,
    pub mod_matrix: ModMatrixState,
    pub macros: [f32; 8],
    pub fx_bus_a: FxBusParams,
    pub fx_bus_b: FxBusParams,
    pub fx_bus_c: FxBusParams,
    pub fx_bus_topology: FxBusTopology,
    pub sub_bus: SubBusParams,
    pub tempo_bpm: f32,
    pub dc_blocker: bool,
    pub master_limiter: bool,
}

impl Default for SynthParams {
    fn default() -> Self {
        let mut oscs = [OscParams::default(); 4];
        oscs[0].level = 0.75;
        let init_slots = std::array::from_fn(FxSlotParams::init_patch_slot);
        Self {
            master_gain: 0.707_106_77, // -3 dB
            engine_mode: EngineMode::Vintage2007,
            poly_mode: PolyMode::Poly,
            glide_ms: 0.0,
            pitch_bend_range: 2.0,
            mpe_enabled: false,
            mpe_pitch_bend_range: 48.0,
            amp_attack_s: 0.005,
            amp_decay_s: 0.25,
            amp_sustain: 1.0,
            amp_release_s: 0.20,
            oscs,
            noise: NoiseParams::default(),
            filters: [
                FilterParams::default_filter_1(),
                FilterParams::default_filter_2(),
            ],
            routing: RoutingParams::default(),
            mod_envs: [AdsrParams::default(); 3],
            modseqs: [ModSeqParams::default(); 4],
            modseq_steps: ModSeqStepBank::default(),
            mod_matrix: ModMatrixState::default(),
            macros: [0.0; 8],
            fx_bus_a: FxBusParams {
                slots: init_slots,
                level: 1.0,
                pan: 0.0,
            },
            fx_bus_b: FxBusParams {
                slots: init_slots,
                level: 1.0,
                pan: 0.0,
            },
            fx_bus_c: FxBusParams {
                slots: init_slots,
                level: 1.0,
                pan: 0.0,
            },
            fx_bus_topology: FxBusTopology::Parallel,
            sub_bus: SubBusParams::default(),
            tempo_bpm: 120.0,
            dc_blocker: true,
            master_limiter: false,
        }
    }
}

trait StereoFilterStage {
    fn process_filter(
        &mut self,
        in_l: f32,
        in_r: f32,
        params: &FilterParams,
        midi_note: f32,
        sample_rate: f32,
        engine_mode: EngineMode,
    ) -> (f32, f32);
}

impl StereoFilterStage for FilterState {
    #[inline]
    fn process_filter(
        &mut self,
        in_l: f32,
        in_r: f32,
        params: &FilterParams,
        midi_note: f32,
        sample_rate: f32,
        engine_mode: EngineMode,
    ) -> (f32, f32) {
        self.process_stereo(in_l, in_r, params, midi_note, sample_rate, engine_mode)
    }
}

impl StereoFilterStage for FilterSvfCore {
    #[inline]
    fn process_filter(
        &mut self,
        in_l: f32,
        in_r: f32,
        params: &FilterParams,
        midi_note: f32,
        sample_rate: f32,
        engine_mode: EngineMode,
    ) -> (f32, f32) {
        self.process_core_stereo(in_l, in_r, params, midi_note, sample_rate, engine_mode)
    }
}

#[inline]
fn split_filter_send(l: f32, r: f32, balance: f32, f1_l: &mut f32, f1_r: &mut f32, f2_l: &mut f32, f2_r: &mut f32) {
    let t = (balance.clamp(-1.0, 1.0) + 1.0) * 0.5;
    let w1 = 1.0 - t;
    let w2 = t;
    *f1_l += l * w1;
    *f1_r += r * w1;
    *f2_l += l * w2;
    *f2_r += r * w2;
}

#[inline]
fn run_inserts_at(
    pos: InsertPosition,
    inserts: &mut [InsertShaperState; 2],
    routing: &RoutingParams,
    engine_mode: EngineMode,
    sample_rate: f32,
    mut l: f32,
    mut r: f32,
) -> (f32, f32) {
    for idx in 0..2 {
        if routing.inserts[idx].position == pos {
            (l, r) = inserts[idx].process_stereo(
                l,
                r,
                &routing.inserts[idx],
                engine_mode,
                sample_rate,
            );
        }
    }
    (l, r)
}

#[inline]
fn apply_osc_modulation(
    osc_p: &OscParams,
    idx: usize,
    mod_offsets: &[f32; NUM_MOD_DESTS],
) -> OscParams {
    let mut out = *osc_p;
    out.wt_pos = (out.wt_pos + mod_offsets[ModDestination::osc_wt_pos(idx).index()]).clamp(0.0, 1.0);
    out.warp_amount =
        (out.warp_amount + mod_offsets[ModDestination::osc_warp(idx).index()]).clamp(0.0, 1.0);
    out.mod_depth =
        (out.mod_depth + mod_offsets[ModDestination::osc_mod_depth(idx).index()]).clamp(0.0, 1.0);
    out.pan = (out.pan + mod_offsets[ModDestination::osc_pan(idx).index()] * 2.0).clamp(-1.0, 1.0);
    out.level = (out.level + mod_offsets[ModDestination::osc_level(idx).index()]).clamp(0.0, 1.0);
    out.pitch_semitones = (out.pitch_semitones
        + mod_offsets[ModDestination::osc_pitch(idx).index()] * 96.0)
        .clamp(-48.0, 48.0);
    out.ratio = (out.ratio + mod_offsets[ModDestination::osc_ratio(idx).index()] * 32.0).max(0.0);
    let d_hz = mod_offsets[ModDestination::osc_hz_offset(idx).index()];
    if d_hz.abs() > 1e-6 {
        let norm = osc_hz_offset_to_norm(out.hz_offset);
        out.hz_offset = osc_hz_offset_from_norm((norm + d_hz).clamp(0.0, 1.0));
    }
    out.phase_offset =
        (out.phase_offset + mod_offsets[ModDestination::osc_phase(idx).index()]).rem_euclid(1.0);
    out.detune =
        (out.detune + mod_offsets[ModDestination::osc_detune(idx).index()]).clamp(0.0, 1.0);
    out.spread =
        (out.spread + mod_offsets[ModDestination::osc_spread(idx).index()]).clamp(0.0, 1.0);
    out
}

#[inline]
fn apply_noise_modulation(
    noise: &NoiseParams,
    mod_offsets: &[f32; NUM_MOD_DESTS],
) -> NoiseParams {
    let mut out = *noise;
    out.color =
        (out.color + mod_offsets[ModDestination::NoiseColor.index()] * 2.0).clamp(-1.0, 1.0);
    out.stereo = (out.stereo + mod_offsets[ModDestination::NoiseStereo.index()]).clamp(0.0, 1.0);
    out.level = (out.level + mod_offsets[ModDestination::NoiseLevel.index()]).clamp(0.0, 1.0);
    out
}

#[inline]
pub fn apply_filter_modulation(
    flt: &FilterParams,
    idx: usize,
    mod_offsets: &[f32; NUM_MOD_DESTS],
) -> FilterParams {
    let mut out = *flt;
    let d_cut = mod_offsets[ModDestination::filter_cutoff(idx).index()];
    if d_cut.abs() > 1e-6 {
        let norm = cutoff_hz_to_norm(out.cutoff_hz);
        out.cutoff_hz = cutoff_norm_to_hz((norm + d_cut).clamp(0.0, 1.0));
    }
    out.resonance =
        (out.resonance + mod_offsets[ModDestination::filter_res(idx).index()]).clamp(0.0, 1.0);
    out.drive =
        (out.drive + mod_offsets[ModDestination::filter_drive(idx).index()]).clamp(0.0, 1.0);
    out.aux = (out.aux + mod_offsets[ModDestination::filter_aux(idx).index()]).clamp(0.0, 1.0);
    out
}

#[inline]
fn apply_routing_modulation(
    routing: &RoutingParams,
    mod_offsets: &[f32; NUM_MOD_DESTS],
) -> RoutingParams {
    let mut out = *routing;
    for i in 0..4 {
        out.osc_filter_balance[i] = (out.osc_filter_balance[i]
            + mod_offsets[ModDestination::osc_filter_send(i).index()] * 2.0)
            .clamp(-1.0, 1.0);
    }
    out.noise_filter_balance = (out.noise_filter_balance
        + mod_offsets[ModDestination::NoiseFilterSend.index()] * 2.0)
        .clamp(-1.0, 1.0);
    out.filter_mix =
        (out.filter_mix + mod_offsets[ModDestination::FilterMix.index()] * 2.0).clamp(-1.0, 1.0);
    out.feedback_amount =
        (out.feedback_amount + mod_offsets[ModDestination::FeedbackAmount.index()]).clamp(0.0, 1.0);
    for idx in 0..2 {
        out.inserts[idx].drive =
            (out.inserts[idx].drive + mod_offsets[ModDestination::insert_drive(idx).index()])
                .clamp(0.0, 1.0);
        out.inserts[idx].mix =
            (out.inserts[idx].mix + mod_offsets[ModDestination::insert_mix(idx).index()])
                .clamp(0.0, 1.0);
    }
    out
}

#[inline]
fn route_direct_to_bus(out: &mut VoiceBusOutput, dest: DirectDest, l: f32, r: f32) {
    match dest {
        DirectDest::Off => {}
        DirectDest::BusA => {
            out.bus_a_l += l;
            out.bus_a_r += r;
        }
        DirectDest::BusB => {
            out.bus_b_l += l;
            out.bus_b_r += r;
        }
        DirectDest::Sub => {
            out.sub_l += l;
            out.sub_r += r;
        }
    }
}

#[inline]
fn route_filter_to_bus(
    out: &mut VoiceBusOutput,
    dest: FilterBusDest,
    topology: FxBusTopology,
    l: f32,
    r: f32,
) {
    let (want_a, mut want_b, mut want_c) = match dest {
        FilterBusDest::BusA => (true, false, false),
        FilterBusDest::BusB => (false, true, false),
        FilterBusDest::Both => (true, true, false),
        FilterBusDest::Sub => (false, false, true),
        FilterBusDest::AC => (true, false, true),
        FilterBusDest::BC => (false, true, true),
        FilterBusDest::All => (true, true, true),
    };
    let a_target = topology.bus_a_target();
    let b_target = topology.bus_b_target();
    if want_a && want_b && a_target == 1 {
        want_b = false;
    }
    if want_c
        && ((want_a && a_target == 2)
            || (want_b && b_target == 2)
            || (want_a && a_target == 1 && b_target == 2))
    {
        want_c = false;
    }
    if want_a {
        out.bus_a_l += l;
        out.bus_a_r += r;
    }
    if want_b {
        out.bus_b_l += l;
        out.bus_b_r += r;
    }
    if want_c {
        out.sub_l += l;
        out.sub_r += r;
    }
}

#[inline]
#[allow(clippy::too_many_arguments)]
fn process_voice_bus<F: StereoFilterStage>(
    oscillators: &mut [OscillatorState; 4],
    osc_out: &mut [f32; 4],
    prev_osc_out: &mut [f32; 4],
    noise: &mut NoiseState,
    filters: &mut [F; 2],
    inserts: &mut [InsertShaperState; 2],
    inserts_aux: &mut [InsertShaperState; 2],
    feedback: &mut VoiceFeedbackState,
    params: &SynthParams,
    mod_offsets: &[f32; NUM_MOD_DESTS],
    tables: [&Wavetable; 4],
    effective_note: f32,
    sample_rate: f32,
) -> VoiceBusOutput {
    // C3 (MIDI 60) = 440 * 2^(-9/12) Hz
    const C3_HZ: f32 = 261.625_58;
    let base_hz = 440.0 * (2.0_f32).powf((effective_note - 69.0) / 12.0);
    let mod_routing = apply_routing_modulation(&params.routing, mod_offsets);
    let mut f1_l = 0.0_f32;
    let mut f1_r = 0.0_f32;
    let mut f2_l = 0.0_f32;
    let mut f2_r = 0.0_f32;
    let mut bus_out = VoiceBusOutput::default();

    let mod_oscs: [OscParams; 4] =
        std::array::from_fn(|i| apply_osc_modulation(&params.oscs[i], i, mod_offsets));
    let mut osc_active = [false; 4];
    for i in 0..4 {
        if mod_oscs[i].enabled && mod_oscs[i].level > 0.0001 {
            osc_active[i] = true;
        }
    }
    for _ in 0..3 {
        for i in 0..4 {
            if osc_active[i] {
                if let Some(src_idx) = mod_oscs[i].mod_source.osc_index() {
                    if mod_oscs[src_idx].enabled {
                        osc_active[src_idx] = true;
                    }
                }
            }
        }
    }

    // Evaluate oscillators 1 -> 4 sequentially for zero-latency forward / 1-sample feedback cross-PM.
    for a in 0..4 {
        if !osc_active[a] {
            oscillators[a].fb_lp_state = 0.0;
            oscillators[a].fb_dc_x1 = 0.0;
            oscillators[a].fb_dc_y1 = 0.0;
            osc_out[a] = 0.0;
            continue;
        }
        let osc_p = mod_oscs[a];

        let (mod_sig, is_fb) = match osc_p.mod_source {
            ModSource::Off => (0.0, false),
            ModSource::Osc1 => {
                let b = 0;
                if b < a { (osc_out[b], false) } else { (prev_osc_out[b], true) }
            }
            ModSource::Osc2 => {
                let b = 1;
                if b < a { (osc_out[b], false) } else { (prev_osc_out[b], true) }
            }
            ModSource::Osc3 => {
                let b = 2;
                if b < a { (osc_out[b], false) } else { (prev_osc_out[b], true) }
            }
            ModSource::Osc4 => {
                let b = 3;
                if b < a { (osc_out[b], false) } else { (prev_osc_out[b], true) }
            }
        };

        let osc_hz = if osc_p.keytrack { base_hz } else { C3_HZ };
        let (raw, l, r) = oscillators[a].process_sample(
            a,
            &osc_p,
            tables[a],
            osc_hz,
            sample_rate,
            params.engine_mode,
            mod_sig,
            is_fb,
        );
        osc_out[a] = raw;
        if osc_p.level > 0.0001 {
            if mod_routing.osc_direct[a] != DirectDest::Off {
                route_direct_to_bus(&mut bus_out, mod_routing.osc_direct[a], l, r);
            } else {
                split_filter_send(
                    l,
                    r,
                    mod_routing.osc_filter_balance[a],
                    &mut f1_l,
                    &mut f1_r,
                    &mut f2_l,
                    &mut f2_r,
                );
            }
        }
    }

    *prev_osc_out = *osc_out;

    // Noise Generator
    let mod_noise = apply_noise_modulation(&params.noise, mod_offsets);
    let (nl, nr) = noise.process_sample(&mod_noise, sample_rate);
    if mod_routing.noise_direct != DirectDest::Off {
        route_direct_to_bus(&mut bus_out, mod_routing.noise_direct, nl, nr);
    } else {
        split_filter_send(
            nl,
            nr,
            mod_routing.noise_filter_balance,
            &mut f1_l,
            &mut f1_r,
            &mut f2_l,
            &mut f2_r,
        );
    }

    // Voice Feedback Bus (1-sample delayed post-filter output back into pre-filter sum)
    let (fb_l, fb_r) = feedback.read_feedback(mod_routing.feedback_amount, sample_rate);
    let mod_flt0 = apply_filter_modulation(&params.filters[0], 0, mod_offsets);
    let mod_flt1 = apply_filter_modulation(&params.filters[1], 1, mod_offsets);

    f1_l += fb_l;
    f1_r += fb_r;
    if mod_routing.filter_routing == FilterRouting::Parallel && mod_flt1.enabled {
        f2_l += fb_l;
        f2_r += fb_r;
    }

    let f1_has = f1_l.abs() + f1_r.abs() > 0.0;
    let f2_has = f2_l.abs() + f2_r.abs() > 0.0;
    if f1_has || !f2_has {
        (f1_l, f1_r) = run_inserts_at(
            InsertPosition::PreFilter,
            inserts,
            &mod_routing,
            params.engine_mode,
            sample_rate,
            f1_l,
            f1_r,
        );
    }
    if f2_has {
        (f2_l, f2_r) = run_inserts_at(
            InsertPosition::PreFilter,
            inserts_aux,
            &mod_routing,
            params.engine_mode,
            sample_rate,
            f2_l,
            f2_r,
        );
    }

    // Filter 1
    let (mut out1_l, mut out1_r) = filters[0].process_filter(
        f1_l,
        f1_r,
        &mod_flt0,
        effective_note,
        sample_rate,
        params.engine_mode,
    );

    (out1_l, out1_r) = run_inserts_at(
        InsertPosition::BetweenFilters,
        inserts,
        &mod_routing,
        params.engine_mode,
        sample_rate,
        out1_l,
        out1_r,
    );

    // Filter 2 + Serial / Parallel Bus Routing + Parallel FX Bus Assignment
    let mix = mod_routing.filter_mix.clamp(-1.0, 1.0);
    let gain_f1 = (1.0 - mix).clamp(0.0, 1.0);
    let gain_f2 = (1.0 + mix).clamp(0.0, 1.0);

    match mod_routing.filter_routing {
        FilterRouting::Serial => {
            let (mut post_l, mut post_r, out_dest) = if mod_flt1.enabled {
                let (out2_l, out2_r) = filters[1].process_filter(
                    out1_l + f2_l,
                    out1_r + f2_r,
                    &mod_flt1,
                    effective_note,
                    sample_rate,
                    params.engine_mode,
                );
                let mixed = if mix >= 0.0 {
                    (out2_l, out2_r)
                } else {
                    let w1 = -mix;
                    let w2 = 1.0 + mix;
                    (out1_l * w1 + out2_l * w2, out1_r * w1 + out2_r * w2)
                };
                (mixed.0, mixed.1, mod_routing.filter_out_bus[1])
            } else {
                (
                    out1_l * gain_f1 + f2_l * gain_f2,
                    out1_r * gain_f1 + f2_r * gain_f2,
                    mod_routing.filter_out_bus[0],
                )
            };

            (post_l, post_r) = run_inserts_at(
                InsertPosition::PostFilter,
                inserts,
                &mod_routing,
                params.engine_mode,
                sample_rate,
                post_l,
                post_r,
            );
            feedback.store_post_filter(post_l, post_r);
            route_filter_to_bus(&mut bus_out, out_dest, params.fx_bus_topology, post_l, post_r);
        }
        FilterRouting::Parallel => {
            let (out2_l, out2_r) = filters[1].process_filter(
                f2_l,
                f2_r,
                &mod_flt1,
                effective_note,
                sample_rate,
                params.engine_mode,
            );
            let b1_l = out1_l * gain_f1;
            let b1_r = out1_r * gain_f1;
            let b2_l = out2_l * gain_f2;
            let b2_r = out2_r * gain_f2;

            if mod_routing.filter_out_bus[0] == mod_routing.filter_out_bus[1] {
                let (mut post_l, mut post_r) = (b1_l + b2_l, b1_r + b2_r);
                (post_l, post_r) = run_inserts_at(
                    InsertPosition::PostFilter,
                    inserts,
                    &mod_routing,
                    params.engine_mode,
                    sample_rate,
                    post_l,
                    post_r,
                );
                feedback.store_post_filter(post_l, post_r);
                route_filter_to_bus(
                    &mut bus_out,
                    mod_routing.filter_out_bus[0],
                    params.fx_bus_topology,
                    post_l,
                    post_r,
                );
            } else {
                let (post1_l, post1_r) = run_inserts_at(
                    InsertPosition::PostFilter,
                    inserts,
                    &mod_routing,
                    params.engine_mode,
                    sample_rate,
                    b1_l,
                    b1_r,
                );
                let (post2_l, post2_r) = run_inserts_at(
                    InsertPosition::PostFilter,
                    inserts_aux,
                    &mod_routing,
                    params.engine_mode,
                    sample_rate,
                    b2_l,
                    b2_r,
                );
                feedback.store_post_filter(post1_l + post2_l, post1_r + post2_r);
                route_filter_to_bus(
                    &mut bus_out,
                    mod_routing.filter_out_bus[0],
                    params.fx_bus_topology,
                    post1_l,
                    post1_r,
                );
                route_filter_to_bus(
                    &mut bus_out,
                    mod_routing.filter_out_bus[1],
                    params.fx_bus_topology,
                    post2_l,
                    post2_r,
                );
            }
        }
    }

    bus_out
}

/// Pre-allocated polyphonic voice.
#[derive(Debug, Clone)]
pub struct Voice {
    pub active: bool,
    pub releasing: bool,
    pub note: u8,
    pub channel: u8,
    pub voice_id: Option<i32>,
    pub velocity: f32,
    pub age: u64,
    pub current_midi_note: f32,
    pub target_midi_note: f32,
    pub pitch_bend_norm: f32,
    pub pitch_bend_smoothed: f32,
    pub poly_tuning_semi: f32,
    pub poly_tuning_smoothed: f32,
    pub aftertouch: f32,
    pub aftertouch_smoothed: f32,
    pub slide: f32,
    pub slide_smoothed: f32,
    pub oscillators: [OscillatorState; 4],
    pub noise: NoiseState,
    pub filters: [FilterState; 2],
    pub inserts: [InsertShaperState; 2],
    pub inserts_aux: [InsertShaperState; 2],
    pub feedback: VoiceFeedbackState,
    pub amp_env: AmpAdsr,
    pub mod_envs: [AmpAdsr; 3],
    pub modseqs: [ModSeqState; 4],
    pub mod_offsets: [f32; NUM_MOD_DESTS],
    pub target_mod_offsets: [f32; NUM_MOD_DESTS],
    pub last_sources: [f32; NUM_ACTIVE_SOURCES],
    pub osc_out: [f32; 4],
    pub prev_osc_out: [f32; 4],
}

impl Default for Voice {
    fn default() -> Self {
        Self::new()
    }
}

impl Voice {
    pub fn new() -> Self {
        Self {
            active: false,
            releasing: false,
            note: 0,
            channel: 0,
            voice_id: None,
            velocity: 0.0,
            age: 0,
            current_midi_note: 60.0,
            target_midi_note: 60.0,
            pitch_bend_norm: 0.0,
            pitch_bend_smoothed: 0.0,
            poly_tuning_semi: 0.0,
            poly_tuning_smoothed: 0.0,
            aftertouch: 0.0,
            aftertouch_smoothed: 0.0,
            slide: 0.0,
            slide_smoothed: 0.0,
            oscillators: [
                OscillatorState::new(),
                OscillatorState::new(),
                OscillatorState::new(),
                OscillatorState::new(),
            ],
            noise: NoiseState::new(),
            filters: [FilterState::new(20_000.0), FilterState::new(2_000.0)],
            inserts: [InsertShaperState::new(), InsertShaperState::new()],
            inserts_aux: [InsertShaperState::new(), InsertShaperState::new()],
            feedback: VoiceFeedbackState::new(),
            amp_env: AmpAdsr::new(),
            mod_envs: [AmpAdsr::new(); 3],
            modseqs: [ModSeqState::new(); 4],
            mod_offsets: [0.0; NUM_MOD_DESTS],
            target_mod_offsets: [0.0; NUM_MOD_DESTS],
            last_sources: [0.0; NUM_ACTIVE_SOURCES],
            osc_out: [0.0; 4],
            prev_osc_out: [0.0; 4],
        }
    }

    pub fn reset(&mut self) {
        self.active = false;
        self.releasing = false;
        self.voice_id = None;
        self.velocity = 0.0;
        self.pitch_bend_norm = 0.0;
        self.pitch_bend_smoothed = 0.0;
        self.poly_tuning_semi = 0.0;
        self.poly_tuning_smoothed = 0.0;
        self.aftertouch = 0.0;
        self.aftertouch_smoothed = 0.0;
        self.slide = 0.0;
        self.slide_smoothed = 0.0;
        self.amp_env.reset();
        for env in &mut self.mod_envs {
            env.reset();
        }
        for ms in &mut self.modseqs {
            ms.reset();
        }
        self.mod_offsets = [0.0; NUM_MOD_DESTS];
        self.target_mod_offsets = [0.0; NUM_MOD_DESTS];
        self.last_sources = [0.0; NUM_ACTIVE_SOURCES];
        self.osc_out = [0.0; 4];
        self.prev_osc_out = [0.0; 4];
        self.noise.reset();
        self.inserts[0].reset();
        self.inserts[1].reset();
        self.inserts_aux[0].reset();
        self.inserts_aux[1].reset();
        self.feedback.reset();
        for f in &mut self.filters {
            f.reset();
        }
        for osc in &mut self.oscillators {
            osc.reset();
        }
    }

    #[inline]
    pub fn release_modulators(&mut self) {
        for env in &mut self.mod_envs {
            env.release();
        }
    }

    /// Evaluates modulation sources and updates destination offsets for this voice.
    #[inline]
    pub fn update_modulation(
        &mut self,
        dt_seconds: f32,
        params: &SynthParams,
        global_modseq_vals: &[f32; 4],
        mod_wheel: f32,
        global_aftertouch: f32,
        global_slide: f32,
        snap: bool,
    ) {
        if !self.active {
            return;
        }
        if snap && dt_seconds == 0.0 {
            self.pitch_bend_smoothed = self.pitch_bend_norm;
            self.poly_tuning_smoothed = self.poly_tuning_semi;
            self.aftertouch_smoothed = self.aftertouch;
            self.slide_smoothed = self.slide;
        }
        let mut sources = [0.0_f32; NUM_ACTIVE_SOURCES];
        sources[0] = self.amp_env.level();
        let sr_equiv = if dt_seconds > 1e-6 {
            1.0 / dt_seconds
        } else {
            48_000.0
        };
        for i in 0..3 {
            let env_p = &params.mod_envs[i];
            if dt_seconds > 0.0 {
                sources[1 + i] = self.mod_envs[i].process(
                    sr_equiv,
                    env_p.attack_s,
                    env_p.decay_s,
                    env_p.sustain,
                    env_p.release_s,
                );
            } else {
                sources[1 + i] = self.mod_envs[i].level();
            }
        }
        for i in 0..4 {
            let mut ms_p = params.modseqs[i];
            ms_p.rate_mod = self.mod_offsets[ModDestination::modseq_rate(i).index()];
            sources[4 + i] = if ms_p.retrig == ModSeqRetrig::PolyVoice {
                self.modseqs[i].advance_with_mseg(
                    dt_seconds,
                    params.tempo_bpm,
                    &ms_p,
                    &params.modseq_steps.seqs[i],
                    &params.modseq_steps.msegs[i],
                )
            } else {
                global_modseq_vals[i]
            };
        }
        sources[8] = self.velocity.clamp(0.0, 1.0);
        sources[9] = ((self.current_midi_note - 12.0) / 96.0).clamp(0.0, 1.0);
        sources[10] = mod_wheel.clamp(0.0, 1.0);
        sources[11] = global_aftertouch.max(self.aftertouch_smoothed).clamp(0.0, 1.0);
        for m in 0..8 {
            sources[12 + m] = params.macros[m].clamp(0.0, 1.0);
        }
        sources[20] = global_slide.max(self.slide_smoothed).clamp(0.0, 1.0);

        self.last_sources = sources;
        params
            .mod_matrix
            .evaluate_destinations(&self.last_sources, &mut self.target_mod_offsets);

        if snap {
            self.mod_offsets = self.target_mod_offsets;
        } else {
            for i in 0..NUM_MOD_DESTS {
                self.mod_offsets[i] += (self.target_mod_offsets[i] - self.mod_offsets[i]) * 0.25;
            }
        }
    }

    /// Processes 1 stereo sample for this voice.
    #[inline]
    pub fn process_sample(
        &mut self,
        params: &SynthParams,
        tables: [&Wavetable; 4],
        sample_rate: f32,
        pitch_bend_semitones: f32,
    ) -> VoiceBusOutput {
        if !self.active {
            return VoiceBusOutput::default();
        }

        // Portamento / Glide (with GlideMs modulation support)
        let eff_glide_ms =
            (params.glide_ms + self.mod_offsets[ModDestination::GlideMs.index()] * 500.0)
                .clamp(0.0, 500.0);
        if (self.current_midi_note - self.target_midi_note).abs() > 1e-4 {
            if eff_glide_ms <= 0.1 {
                self.current_midi_note = self.target_midi_note;
            } else {
                let glide_samples = (eff_glide_ms * 0.001 * sample_rate).max(1.0);
                let coeff = 1.0 - (-4.6 / glide_samples).exp();
                self.current_midi_note += (self.target_midi_note - self.current_midi_note) * coeff;
            }
        }

        let env_level = self.amp_env.process(
            sample_rate,
            params.amp_attack_s,
            params.amp_decay_s,
            params.amp_sustain,
            params.amp_release_s,
        );

        if !self.amp_env.is_active() {
            self.active = false;
            self.releasing = false;
            return VoiceBusOutput::default();
        }

        // 1.5 ms per-voice 1-pole smoothing for MPE pitch bend, tuning, pressure, and slide.
        let expr_coeff = 1.0 - (-1.0 / (0.0015 * sample_rate.max(1000.0))).exp();
        self.pitch_bend_smoothed += (self.pitch_bend_norm - self.pitch_bend_smoothed) * expr_coeff;
        self.poly_tuning_smoothed +=
            (self.poly_tuning_semi - self.poly_tuning_smoothed) * expr_coeff;
        self.aftertouch_smoothed += (self.aftertouch - self.aftertouch_smoothed) * expr_coeff;
        self.slide_smoothed += (self.slide - self.slide_smoothed) * expr_coeff;

        let mpe_pb_semi = if params.mpe_enabled && self.channel > 0 {
            self.pitch_bend_smoothed * params.mpe_pitch_bend_range
        } else {
            0.0
        };
        let effective_note =
            self.current_midi_note + pitch_bend_semitones + mpe_pb_semi + self.poly_tuning_smoothed;
        let out = process_voice_bus(
            &mut self.oscillators,
            &mut self.osc_out,
            &mut self.prev_osc_out,
            &mut self.noise,
            &mut self.filters,
            &mut self.inserts,
            &mut self.inserts_aux,
            &mut self.feedback,
            params,
            &self.mod_offsets,
            tables,
            effective_note,
            sample_rate,
        );

        out.scale(env_level)
    }
}

/// Pre-allocated 3 ms fast-fade "tail" voice for click-free voice stealing.
#[derive(Debug, Clone)]
pub struct TailVoice {
    pub active: bool,
    pub age: u64,
    pub current_midi_note: f32,
    pub amp_at_steal: f32,
    pub fade_remaining: u32,
    pub fade_total: u32,
    pub oscillators: [OscillatorState; 4],
    pub noise: NoiseState,
    pub filter_cores: [FilterSvfCore; 2],
    pub inserts: [InsertShaperState; 2],
    pub inserts_aux: [InsertShaperState; 2],
    pub feedback: VoiceFeedbackState,
    pub mod_offsets: [f32; NUM_MOD_DESTS],
    pub osc_out: [f32; 4],
    pub prev_osc_out: [f32; 4],
}

impl Default for TailVoice {
    fn default() -> Self {
        Self::new()
    }
}

impl TailVoice {
    pub fn new() -> Self {
        Self {
            active: false,
            age: 0,
            current_midi_note: 60.0,
            amp_at_steal: 0.0,
            fade_remaining: 0,
            fade_total: 1,
            oscillators: [
                OscillatorState::new(),
                OscillatorState::new(),
                OscillatorState::new(),
                OscillatorState::new(),
            ],
            noise: NoiseState::new(),
            filter_cores: [FilterSvfCore::new(20_000.0), FilterSvfCore::new(2_000.0)],
            inserts: [InsertShaperState::new(), InsertShaperState::new()],
            inserts_aux: [InsertShaperState::new(), InsertShaperState::new()],
            feedback: VoiceFeedbackState::new(),
            mod_offsets: [0.0; NUM_MOD_DESTS],
            osc_out: [0.0; 4],
            prev_osc_out: [0.0; 4],
        }
    }

    pub fn reset(&mut self) {
        self.active = false;
        self.fade_remaining = 0;
        self.amp_at_steal = 0.0;
        self.mod_offsets = [0.0; NUM_MOD_DESTS];
        self.osc_out = [0.0; 4];
        self.prev_osc_out = [0.0; 4];
        self.noise.reset();
        self.inserts[0].reset();
        self.inserts[1].reset();
        self.inserts_aux[0].reset();
        self.inserts_aux[1].reset();
        self.feedback.reset();
        for fc in &mut self.filter_cores {
            fc.reset_states();
        }
        for osc in &mut self.oscillators {
            osc.reset();
        }
    }

    pub fn steal_from(&mut self, voice: &Voice, fade_samples: u32, age: u64) {
        self.active = true;
        self.age = age;
        self.current_midi_note = voice.current_midi_note;
        self.amp_at_steal = voice.amp_env.level();
        self.fade_total = fade_samples.max(1);
        self.fade_remaining = self.fade_total;
        self.oscillators = voice.oscillators.clone();
        self.noise = voice.noise;
        self.filter_cores = [voice.filters[0].core, voice.filters[1].core];
        self.inserts = voice.inserts;
        self.inserts_aux = voice.inserts_aux;
        self.feedback = voice.feedback;
        self.mod_offsets = voice.mod_offsets;
        self.osc_out = voice.osc_out;
        self.prev_osc_out = voice.prev_osc_out;
    }

    #[inline]
    pub fn process_sample(
        &mut self,
        params: &SynthParams,
        tables: [&Wavetable; 4],
        sample_rate: f32,
        pitch_bend_semitones: f32,
    ) -> VoiceBusOutput {
        if !self.active || self.fade_remaining == 0 {
            self.active = false;
            return VoiceBusOutput::default();
        }

        let fade_gain = self.fade_remaining as f32 / self.fade_total as f32;
        self.fade_remaining -= 1;
        if self.fade_remaining == 0 {
            self.active = false;
        }

        let effective_note = self.current_midi_note + pitch_bend_semitones;
        let out = process_voice_bus(
            &mut self.oscillators,
            &mut self.osc_out,
            &mut self.prev_osc_out,
            &mut self.noise,
            &mut self.filter_cores,
            &mut self.inserts,
            &mut self.inserts_aux,
            &mut self.feedback,
            params,
            &self.mod_offsets,
            tables,
            effective_note,
            sample_rate,
        );

        let gain = self.amp_at_steal * fade_gain;
        out.scale(gain)
    }
}
