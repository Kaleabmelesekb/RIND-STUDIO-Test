# ML Phase

Stock time and polarity alignment effect using ML Studio's shared native editor. ConjureAlign's GPL-3.0-or-later correlation and fractional-delay DSP is adapted here; see UPSTREAM.md and LICENSE for provenance.

- Stereo or mono main input with matching reference sidechain; measurement uses the left channel and applies one correction to all main channels.
- Two-second capture, background correlation, confidence/rejection report, explicit Apply suggestion. Manual timing range ±50 ms in 0.001 ms steps, plus polarity reversal.
- Fixed latency: ceil(sample_rate × 0.05) + FIR_CENTER samples, reported to the host. Supported rates 8–192 kHz. This is a mixing tool, not a low-latency monitoring insert.
- Capture buffers are allocated outside processing. The audio callback uses atomic storage, fixed delay memory and no mutexes, allocation or analysis. Cancellation/reset invalidates worker generations; teardown joins outstanding workers.
- Smooth linear delay-tap crossfades avoid boosting correlated signals while changing offsets. Only timing/polarity parameters persist; temporary captures do not.
- Linked CLAP entry for stock availability, plus the normal standalone CLAP/VST3 bundling path. Stock DSP visibility applies. No donor analytics, updater or network dependencies.

The in-app User Guide contains routing and capture instructions. Focused checks: `cargo test -p ml-phase --lib`. Set `ML_PHASE_RENDER_DIR` to an output folder to render the actual native editor at 100%, 150% and 200% without opening a window. Host tests exercise latency, impulse processing and state restoration using ML Studio's CLAP host.
