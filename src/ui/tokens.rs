//! Rind's design tokens: the corner radii and insets the chrome is drawn with, in `U`, so a surface asks for a size
//! ("small", "medium", "large") and not for a number of its own.
//!
//! The unit tests draw with square corners and flat tiles, as the drawings they pin were made: [`styled`] is false
//! there, and every helper below gives the flat answer then.

use eframe::egui::{Rect, vec2};

use crate::metrics::Metrics;

/// Chips and hover bands.
pub const RADIUS_S: f32 = 0.14;
/// Buttons and tiles.
pub const RADIUS_M: f32 = 0.24;
/// Popups, cards and sheets.
pub const RADIUS_L: f32 = 0.36;

/// Whether the chrome wears Rind's rounded look (the running app) or the square one the tests pin.
pub const fn styled() -> bool {
    !cfg!(test)
}

/// A corner radius of `token` `U`, in points; none in the unit tests.
pub fn radius(m: &Metrics, token: f32) -> f32 {
    match styled() {
        true => m.u(token),
        false => 0.0,
    }
}

/// `cell` pulled in a hair on every side, so rounded bands side by side do not touch; unchanged in the tests.
pub fn inset(cell: Rect, m: &Metrics) -> Rect {
    match styled() {
        true => cell.shrink2(vec2(m.u(0.08), m.u(0.05))),
        false => cell,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::offscreen::metrics;

    #[test]
    fn the_tests_draw_flat_and_the_radii_grow_with_their_size() {
        let m = metrics();
        assert!(!styled());
        assert_eq!(radius(&m, RADIUS_L), 0.0);
        let cell = Rect::from_min_size(Default::default(), vec2(40.0, 20.0));
        assert_eq!(inset(cell, &m), cell);
        assert!(RADIUS_S < RADIUS_M && RADIUS_M < RADIUS_L);
    }
}
