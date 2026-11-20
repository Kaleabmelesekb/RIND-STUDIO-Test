//! Vital's own interface full screen, from its rack card's full screen button: as large as the screen holds it, its
//! aspect kept and centred, over the dimmed screen, and driven as in its card ([`vital_card::show`]). The close X or
//! `close` (Escape, Back) closes it, once the interface's text dialog is gone. The card behind lets go of the interface
//! while this shows (the interface is one, at one size).

use eframe::egui::{self, Area, Id, LayerId, Order, Rect, Sense, Vec2, pos2, vec2};

use super::{AppCtx, Nav};
use crate::icons::{IconId, draw_icon};
use crate::ui::anim;
use crate::ui::buttons::filled_circle_button;
use crate::ui::rack::vital_card::{self, VitalCard};

/// The close X's disc (U), as the Lemon editor's.
const CLOSE: f32 = 1.5;
const CLOSE_AT: (f32, f32) = (2.1, 0.2);

/// The interface shown: its texture and finger.
#[derive(Default)]
pub struct State {
    card: VitalCard,
}

/// Shows the interface of module `module` on `channel` full screen. `Nav::Close` once it closes, or once the module
/// is gone.
pub fn show(
    state: &mut State,
    ui: &egui::Ui,
    app: &mut AppCtx,
    channel: usize,
    module: i32,
) -> Option<Nav> {
    let Some(channel_id) = app.song.channels.get(channel).map(|c| c.id) else {
        return Some(Nav::Close);
    };
    let ctx = ui.ctx().clone();
    if crate::ui::close_pressed(&ctx) && !vital_card::typing(&state.card) {
        return close(state);
    }
    let (m, c) = (app.cx.m, &app.cx.theme.colors);
    let (id, screen, bg) = (
        Id::new("vital full screen"),
        app.regions.screen,
        c.background.to_color32(),
    );
    crate::ui::backdrop(&ctx, id);
    let dim = ctx.layer_painter(LayerId::new(Order::Middle, id.with("backdrop")));
    dim.rect_filled(screen, 0.0, anim::dim(bg, anim::DIM_ALPHA));
    let disc = Rect::from_min_size(
        pos2(
            screen.right() - m.u(CLOSE_AT.0),
            screen.top() + m.u(CLOSE_AT.1),
        ),
        Vec2::splat(m.u(CLOSE)),
    );
    let area = screen.shrink2(vec2(m.u(CLOSE_AT.0), 0.0));
    // A portrait screen turns the interface a quarter, to use its length.
    state.card.rotate = area.height() > area.width();
    let rect = vital_card::fit(area, &state.card);
    let mut closed = false;
    let mut shown = None;
    Area::new(id)
        .order(Order::Foreground)
        .fixed_pos(screen.min)
        .constrain(false)
        .fade_in(false)
        .show(&ctx, |ui| {
            ui.allocate_exact_size(screen.size(), Sense::hover());
            if let Some(module) = app.song.channels[channel].rack.get_mut(module) {
                shown = Some(vital_card::show(
                    ui,
                    rect,
                    channel_id,
                    module,
                    &mut state.card,
                ));
            }
            let x = filled_circle_button(ui, id.with("close"), disc.center(), disc.width(), bg);
            draw_icon(
                ui.painter(),
                disc,
                IconId::Close,
                c.text.to_color32(),
                0.7,
                m,
            );
            closed = x.clicked();
        });
    let Some(shown) = shown else {
        return close(state);
    };
    if !shown.changed.is_empty() {
        app.channel_param_edited();
    }
    // The interface cannot start: the card's own panel is behind it.
    if closed || (!shown.drawn && !vital_card::drawn(Some(&state.card))) {
        return close(state);
    }
    None
}

/// Closes it, letting go of its texture.
fn close(state: &mut State) -> Option<Nav> {
    *state = State::default();
    Some(Nav::Close)
}
