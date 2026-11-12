//! A device window: one rack card of the Device Rack's strip in a window of its own, centred over the dimmed screen as a
//! synth's editor is, from the card's pop-out button. The card is the rack's ([`rack::show_window`]): its controls,
//! menus and dialogs work as in the rack, and an edit is the rack's. The X, a tap outside it or the close key closes it
//! (the close key goes to a card's open menu first), and so does the card's channel or module going away.

use eframe::egui::{self, Id, Rect, Sense, Stroke, StrokeKind, pos2, vec2};

use super::{AppCtx, Nav, rack};
use crate::icons::{IconId, draw_icon};
use crate::song::ModuleInstance;
use crate::ui::anim;

/// The title bar's height and its close X's width, in U.
const TITLE_H: f32 = 1.5;
const CLOSE_W: f32 = 1.5;
/// The window's margin from the screen's edges, in U.
const MARGIN: f32 = 0.75;

/// Shows the window of module `module` on `channel`, `held` as for the rack. `Nav::Close` once it closes or what it
/// shows is gone.
pub fn show(
    state: &mut rack::State,
    ui: &mut egui::Ui,
    app: &mut AppCtx,
    (channel, module): (usize, i32),
    clipboard: &mut Vec<ModuleInstance>,
    held: Option<(usize, u32)>,
) -> Option<Nav> {
    let Some(size) = rack::window_size(state, app, channel, module) else {
        return Some(Nav::Close);
    };
    if crate::ui::close_pressed(ui.ctx()) && !rack::popup_open(state) {
        return Some(Nav::Close);
    }
    let m = app.cx.m;
    let c = &app.cx.theme.colors;
    let (bg, text, bar) = (
        c.background.to_color32(),
        c.text.to_color32(),
        c.control_fill.to_color32(),
    );
    let px = 1.0 / m.ppp();
    let screen = app.regions.screen;
    let title = (app.song.channels.get(channel))
        .and_then(|ch| {
            ch.rack
                .get(module)
                .map(|d| format!("{}  /  {}", ch.name, d.title()))
        })
        .unwrap_or_default();

    // The screen dims; a tap outside the window closes it, so the window itself takes its taps first.
    ui.painter()
        .rect_filled(screen, 0.0, anim::dim(bg, anim::DIM_ALPHA));
    let outside = ui.interact(screen, Id::new("device window backdrop"), Sense::click());
    let (title_h, room) = (m.u(TITLE_H), screen.shrink(m.u(MARGIN)));
    let window = Rect::from_center_size(
        screen.center(),
        vec2(
            size.x.min(room.width()),
            (size.y + title_h).min(room.height()),
        ),
    );
    ui.interact(window, Id::new("device window"), Sense::click_and_drag());
    let head = window.with_max_y(window.top() + title_h);
    let body = window.with_min_y(head.bottom());
    let painter = ui.painter().clone();
    painter.rect_filled(window, 0.0, bg);
    painter.rect_filled(head, 0.0, bar);
    painter.rect_stroke(
        window,
        0.0,
        Stroke::new(px, text.gamma_multiply(0.35)),
        StrokeKind::Outside,
    );
    painter.line_segment(
        [head.left_bottom(), head.right_bottom()],
        Stroke::new(px, text.gamma_multiply(0.35)),
    );
    let label = head
        .with_max_x(head.right() - m.u(CLOSE_W))
        .shrink2(vec2(m.u(0.5), 0.0));
    crate::ui::widgets::truncated(
        &painter,
        label,
        egui::Align2::LEFT_CENTER,
        &title,
        m.font_id(crate::metrics::FontSize::Pt12),
        text,
        5,
    );
    let x = Rect::from_min_size(
        pos2(head.right() - m.u(CLOSE_W), head.top()),
        vec2(m.u(CLOSE_W), head.height()),
    );
    let closing = ui.interact(x, Id::new("device window close"), Sense::click());
    draw_icon(&painter, x, IconId::Close, text, 0.7, m);

    let nav = rack::show_window(state, ui, app, body, channel, module, clipboard, held);
    if nav.is_some() {
        return nav;
    }
    (closing.clicked() || outside.clicked()).then_some(Nav::Close)
}
