// SPDX-License-Identifier: GPL-3.0-or-later
// Honeydew (vendored addition): ML Studio's `os_plugkit::visibility`, without the settings file. Honeydew has no
// "hide the stock DSP controls" setting, so the Advanced DSP controls of ML Studio's plugins are always shown.

/// ML Studio's settings key for the Advanced DSP controls (kept for the crates that name it).
pub const STOCK_DSP_CONTROLS: &str = "visible_stock-dsp-controls";

/// Whether the plugins show their Advanced DSP controls: always.
pub fn stock_dsp_controls() -> bool {
    true
}
