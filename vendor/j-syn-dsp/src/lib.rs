//! `j-syn-dsp`: Zero-allocation 16-voice polyphonic wavetable synthesizer DSP engine.

pub mod adaa;
pub mod engine;
pub mod envelope;
pub mod filter;
pub mod fx;
pub mod insert;
pub mod mod_matrix;
pub mod modseq;
pub mod noise;
pub mod oscillator;
pub mod oversample;
pub mod serde_lenient;
pub mod voice;
pub mod wavetable;
pub mod waveshaper;

pub use waveshaper::DistortionMode;

pub use engine::{SynthEngine, MAX_TAIL_VOICES, MAX_VOICES};
pub use envelope::{evaluate_adsr_preview, AdsrParams, AdsrStage, AmpAdsr};
pub use filter::{
    evaluate_filter_response_db, fast_tanh, FilterMode, FilterParams, FilterSlope, FilterState,
    FilterSvfCore, StereoZdfSvf, COMB_BUF_LEN,
};
pub use fx::{
    diopser_freq_hz, diopser_stage_count, evaluate_diopser_ripple, evaluate_flanger_scope_response,
    evaluate_tube_res_scope_response, filter_response_db_from_slot, flanger_delay_ms,
    flanger_feedback, flanger_phase_deg, flanger_rate_hz, format_tube_res_tune, format_utility_pan,
    fx_high_cut_hz, fx_low_cut_hz, limiter_ceil_db, limiter_gain_db, limiter_lookahead_ms,
    limiter_release_ms, mb_band_level_db, mb_band_magnitude_at_hz, mb_ordered_xover_hz,
    mb_out_gain_db, mb_xover_high_hz, mb_xover_high_norm, mb_xover_low_hz, mb_xover_low_norm,
    sub_cutoff_hz_to_norm, sub_cutoff_norm_to_hz, tube_res_decay_ms, tube_res_flt_freq_hz,
    tube_res_flt_width_oct, tube_res_lfo_rate_hz, tube_res_tune_hz, tube_res_tune_midi,
    utility_gain_db_from_norm, utility_pan_bipolar, utility_width_from_norm, FxBusParams,
    FxBusTopology, FxFilterSlope, FxRackParams, FxRackState, FxSlotParams, FxType,
    MbDistBandParams, SubBusParams, DELAY_SYNC_LABELS, DIOPSER_STAGE_COUNTS, MAX_FX_STACK,
    MB_BAND_LEVEL_DEFAULT_NORM, MB_XOVER_HIGH_DEFAULT_NORM, MB_XOVER_LOW_DEFAULT_NORM,
    NUM_FX_BUSES, NUM_FX_SLOTS, UTILITY_WIDTH_DEFAULT_NORM,
};
pub use insert::{
    evaluate_insert_curve, DirectDest, FilterBusDest, FilterRouting, InsertMode, InsertParams,
    InsertPosition, InsertShaperState, InsertState, RoutingParams, VoiceFeedbackState,
};
pub use mod_matrix::{
    FxScopeRing, ModDestination, ModMatrixSource, ModMatrixState, ModSlot, ModUiTelemetry,
    FX_SCOPE_LEN, NUM_ACTIVE_SOURCES, NUM_MOD_DESTS, NUM_MOD_SLOTS,
};
pub use filter::{cutoff_hz_to_norm, cutoff_norm_to_hz};
pub use modseq::{
    evaluate_modseq_curve, evaluate_modseq_curve_with_swing, evaluate_mseg_curve,
    evaluate_step_shape, modseq_division_label, modseq_free_norm_to_hz, modseq_rate_hz,
    LfoPreset, ModSeqMode, ModSeqParams, ModSeqPlayMode, ModSeqRetrig, ModSeqShapePreset,
    ModSeqState, ModSeqStep, ModSeqStepBank, ModSeqStepShape, ModSeqSyncMode, MsegCurve,
    MsegNode, MAX_MODSEQ_STEPS, MAX_MSEG_NODES, MODSEQ_DOTTED_DIVS, MODSEQ_STRAIGHT_DIVS,
    MODSEQ_TRIPLET_DIVS,
};
pub use noise::{NoiseParams, NoiseState, NoiseType};
pub use oscillator::{
    osc_hz_offset_from_norm, osc_hz_offset_to_norm, sample_with_warp_and_pm, warp_phase_bend,
    EngineMode, ModOrder, ModSource, ModType, OscParams, OscPitchMode, OscillatorState, WarpMode,
    MAX_UNISON,
};
pub use voice::{PolyMode, SynthParams, TailVoice, Voice, VoiceBusOutput};
pub use wavetable::{
    FactoryWavetable, Wavetable, WavetableBank, FRAME_LEN, MAX_FRAMES, NUM_FACTORY_WAVETABLES,
    NUM_MIP_LEVELS,
};
