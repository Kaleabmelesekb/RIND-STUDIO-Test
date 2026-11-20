//! Vital's card: below the header, Vital's own interface, the original JUCE/OpenGL one (`vendor/vital-ui`), drawn
//! off-screen by the engine's process into a texture at the card's width. Android only ([`gui::AVAILABLE`]); where the
//! interface is not there, or cannot start, the caller shows the module's own panel.
//!
//! - One finger is the interface's mouse: a press, its moves and its release go in the interface's pixels; a finger that
//!   rests on a control for [`LONG_S`] without moving presses its right button (Vital's context menus and modulation
//!   removal). The scroll wheel (two fingers on a tablet, the wheel on a desktop) is the interface's wheel.
//! - A text field of the interface (a typed value, a preset name) opens Android's own text dialog; its text and Enter
//!   go to the field.
//! - The interface edits the engine's parameters itself. The card reads what it changed ([`gui::changes`]) and the
//!   modulation slots it set into the module's params, so that the song, undo, automation and a save hold them.
//! - Vital keeps one interface at a time: the module that opened last has it, another card shows a stand-in and takes
//!   it with a tap.

use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use eframe::egui::{
    self, Align2, Color32, ColorImage, FontId, Id, Pos2, Rect, Sense, TextureHandle,
    TextureOptions, Ui, pos2, vec2,
};
use honeydew_audio::vital::{self, gui};

use crate::android_text;
use crate::song::ModuleInstance;
use crate::ui::{Finger, fingers, focus_lost};

/// Vital's own window: 1400 x 820 (its aspect is fixed).
const NATIVE: (u32, u32) = (1400, 820);
/// The frames per second the interface draws while it shows, and while nothing of it does.
const FPS: u32 = 30;
const FPS_HIDDEN: u32 = 1;
/// A finger held this long (seconds) within [`LONG_PT`] of where it went down is a right press.
const LONG_S: f64 = 0.55;
const LONG_PT: f32 = 8.0;
/// Pixels a resize waits to move by, and how long it has to hold (seconds), before the interface is resized.
const RESIZE_PX: f32 = 8.0;
const RESIZE_S: f64 = 0.25;
/// JUCE's button flags.
const LEFT: i32 = 16;
const RIGHT: i32 = 32;
/// How often (seconds) the modulation slots are read while the card is idle, and a failed open is tried again.
const SLOTS_S: f64 = 0.5;
const RETRY_S: f64 = 1.0;
/// Opens that fail before the card shows the module's own panel instead (it keeps trying meanwhile).
const GIVE_UP: u32 = 8;

const PENDING: u8 = 0;
const OPENED: u8 = 1;
const FAILED: u8 = 2;

/// The interface's key (the engine registry's) that has the one interface: 0 for none.
static CURRENT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// A Vital card's own state: the interface's texture and the finger's.
#[derive(Default)]
pub(crate) struct VitalCard {
    key: u64,
    /// The background open in flight: the interface starts on its own thread (it makes a GL context).
    opening: Option<Arc<AtomicU8>>,
    tries: u32,
    retry_at: f64,
    texture: Option<TextureHandle>,
    /// The size of the frame shown, and the buffer frames arrive in.
    size: (u32, u32),
    buffer: Vec<u8>,
    /// The size the interface was asked for, and a different one wanted since when.
    asked: (u32, u32),
    wanted: Option<((u32, u32), f64)>,
    /// The finger that is the mouse: its id, where and when it went down, whether it moved off, whether it is now a right
    /// press.
    finger: Option<Finger0>,
    /// A text dialog was asked for the field that has the keyboard, and has not answered.
    typing: bool,
    slots_at: f64,
    /// The last frame's rect was visible.
    shown: bool,
    /// The pointer or a finger is on the interface this frame: the rack's wheel is the interface's.
    over: bool,
}

impl std::fmt::Debug for VitalCard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VitalCard")
            .field("key", &self.key)
            .field("size", &self.size)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug)]
struct Finger0 {
    id: u64,
    at: Pos2,
    since: f64,
    moved: bool,
    right: bool,
}

/// The card body's height for a frame of `size` at `width` (points), a native-aspect body before one has come.
pub(super) fn body_height(card: Option<&VitalCard>, width: f32) -> f32 {
    let (w, h) = card
        .map(|c| c.size)
        .filter(|&(w, h)| w > 0 && h > 0)
        .unwrap_or(NATIVE);
    width * h as f32 / w as f32
}

/// Whether this build can show Vital's interface at all.
pub(super) fn available() -> bool {
    gui::AVAILABLE
}

/// Whether the card body is Vital's interface (its aspect): not once the interface has failed to start so often that
/// the module's own panel stands in.
pub(super) fn drawn(card: Option<&VitalCard>) -> bool {
    gui::AVAILABLE && card.is_none_or(|c| c.tries < GIVE_UP || c.texture.is_some())
}

/// Whether the pointer or a finger is on the interface: the rack's wheel is then the interface's.
pub(super) fn over(card: &VitalCard) -> bool {
    card.over || card.finger.is_some()
}

/// What [`show`] did: whether the interface is the card's body, and the host params it changed.
pub(crate) struct Shown {
    /// `false`: the caller draws the module's own panel (the interface is not available, or cannot start).
    pub drawn: bool,
    pub changed: Vec<i32>,
}

/// Draws module `module`'s interface (on the channel with id `channel_id`) in the card body `content` (points).
pub(crate) fn show(
    ui: &mut Ui,
    content: Rect,
    channel_id: i32,
    module: &mut ModuleInstance,
    card: &mut VitalCard,
) -> Shown {
    let none = Shown {
        drawn: false,
        changed: Vec::new(),
    };
    if !gui::AVAILABLE {
        return none;
    }
    let ctx = ui.ctx().clone();
    let (now, ppp) = ctx.input(|i| (i.time, i.pixels_per_point));
    let key = vital::key(channel_id, module.instance_id);
    if card.key != key {
        *card = VitalCard {
            key,
            ..VitalCard::default()
        };
    }
    let visible = ui.is_rect_visible(content);

    // The size the interface is drawn at: the body's width in device pixels, the native aspect, within the frame buffer.
    let k = (content.width() * ppp / NATIVE.0 as f32).max(0.3);
    let k = k
        .min(2400.0 / NATIVE.0 as f32)
        .min(1500.0 / NATIVE.1 as f32);
    let want = (
        (NATIVE.0 as f32 * k).round() as u32,
        (NATIVE.1 as f32 * k).round() as u32,
    );

    // The open, on its own thread, and its answer.
    if let Some(state) = card.opening.clone() {
        match state.load(Ordering::Acquire) {
            OPENED => {
                card.opening = None;
                card.tries = 0;
            }
            FAILED => {
                card.opening = None;
                card.tries += 1;
                card.retry_at = now + RETRY_S;
            }
            _ => {}
        }
    }
    let open = gui::is_open(key);
    if open {
        CURRENT.store(key, Ordering::Relaxed);
    } else if visible && card.opening.is_none() && now >= card.retry_at {
        // Another module's interface is open: this card takes it only with a tap ([`stand_in`]).
        let other = CURRENT.load(Ordering::Relaxed);
        let free = other == 0 || other == key || !gui::is_open(other);
        if free {
            begin_open(card, key, want, now);
        }
    }

    let id = Id::new(("vital gui", key));
    if !open {
        // Not there (yet): the texture it had, if any, else a stand-in, else the module's panel after a few failures.
        let height = body_height(Some(card), content.width());
        let rect = Rect::from_min_size(content.min, vec2(content.width(), height));
        let response = ui.interact(rect, id, Sense::click());
        if card.texture.is_none() {
            if card.tries >= GIVE_UP {
                ctx.request_repaint_after(std::time::Duration::from_secs(1));
                return none;
            }
            let other = CURRENT.load(Ordering::Relaxed);
            let elsewhere = other != 0 && other != key && gui::is_open(other);
            stand_in(
                ui,
                rect,
                if elsewhere {
                    "Vital is open on another module — tap to open it here"
                } else {
                    "Starting Vital…"
                },
            );
            if elsewhere && response.clicked() {
                CURRENT.store(0, Ordering::Relaxed);
                gui::close(other);
                card.retry_at = 0.0;
            }
        } else {
            paint(ui, rect, card);
            if response.clicked() {
                CURRENT.store(0, Ordering::Relaxed);
                card.retry_at = 0.0;
            }
        }
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
        return Shown {
            drawn: true,
            changed: Vec::new(),
        };
    }

    // Take the newest frame.
    if visible {
        if !card.shown {
            gui::set_fps(FPS);
        }
        if let Some((w, h)) = gui::take_frame(key, &mut card.buffer) {
            let image = ColorImage::from_rgba_unmultiplied(
                [w as usize, h as usize],
                &card.buffer[..(w * h * 4) as usize],
            );
            match &mut card.texture {
                Some(t) => t.set(image, TextureOptions::LINEAR),
                None => {
                    card.texture = Some(ctx.load_texture(
                        format!("vital gui {key}"),
                        image,
                        TextureOptions::LINEAR,
                    ))
                }
            }
            card.size = (w, h);
        }
    } else if card.shown {
        gui::set_fps(FPS_HIDDEN);
    }
    card.shown = visible;

    // Its size follows the card's width, once the change has held a moment.
    resize(card, key, want, now);

    let height = body_height(Some(card), content.width());
    let rect = Rect::from_min_size(content.min, vec2(content.width(), height));
    let response = ui.interact(rect, id, Sense::click_and_drag());
    let px = card.size.0.max(1) as f32 / rect.width();
    forward(ui, card, key, &response, rect, px, now);
    wheel(ui, card, key, rect, px, now);
    text(&ctx, card, key, id);
    paint(ui, rect, card);

    // What the interface changed, into the module.
    let mut changed = Vec::new();
    for (vital_index, value) in gui::changes(key) {
        let Some(index) = vital::host_index(vital_index) else {
            continue;
        };
        let Some(entry) = vital::TABLE.get(index - 1) else {
            continue;
        };
        let norm = entry.norm(value);
        if let Some(p) = module.params.get_mut(index)
            && (*p - norm).abs() > 1e-6
        {
            *p = norm;
            changed.push(index as i32);
        }
    }
    if changed.is_empty() && now - card.slots_at < SLOTS_S && card.finger.is_none() {
        // Nothing moved: the slots are read only now and then.
    } else if let Some(slots) = gui::slots(key) {
        card.slots_at = now;
        for (slot, (source, destination)) in slots.iter().enumerate().take(vital::SLOTS) {
            let Some((si, di)) = vital::slot_indices(slot) else {
                continue;
            };
            for (index, menu) in [
                (si, vital::source_index(source)),
                (di, vital::destination_index(destination)),
            ] {
                let norm = vital::TABLE[index - 1].norm(menu as f32);
                if let Some(p) = module.params.get_mut(index)
                    && (*p - norm).abs() > 1e-6
                {
                    *p = norm;
                    changed.push(index as i32);
                }
            }
        }
    }
    // A frame comes every 1/FPS: keep drawing while it shows.
    ctx.request_repaint_after(std::time::Duration::from_millis(if visible {
        1000 / u64::from(FPS)
    } else {
        500
    }));
    Shown {
        drawn: true,
        changed,
    }
}

/// Starts the interface on its own thread.
fn begin_open(card: &mut VitalCard, key: u64, want: (u32, u32), now: f64) {
    #[cfg(target_os = "android")]
    {
        use std::sync::Once;
        static HOME: Once = Once::new();
        HOME.call_once(|| {
            if let Some(dir) = crate::android::data_dir() {
                gui::set_home(&dir.to_string_lossy());
            }
        });
    }
    let state = Arc::new(AtomicU8::new(PENDING));
    card.opening = Some(state.clone());
    card.asked = want;
    card.retry_at = now + RETRY_S;
    let spawned = std::thread::Builder::new()
        .name("vital-gui-open".into())
        .spawn(move || {
            let ok = gui::open(key, want.0, want.1);
            state.store(if ok { OPENED } else { FAILED }, Ordering::Release);
        });
    if spawned.is_err() {
        card.opening = None;
        card.tries += 1;
    }
}

/// Asks for a new size of the interface once `want` has held for [`RESIZE_S`].
fn resize(card: &mut VitalCard, key: u64, want: (u32, u32), now: f64) {
    let far = (want.0 as f32 - card.asked.0 as f32).abs() > RESIZE_PX;
    if !far {
        card.wanted = None;
        return;
    }
    match card.wanted {
        Some((size, since)) if size == want => {
            if now - since >= RESIZE_S {
                gui::resize(key, want.0, want.1);
                card.asked = want;
                card.wanted = None;
            }
        }
        _ => card.wanted = Some((want, now)),
    }
}

/// The finger that is the interface's mouse. `px`: interface pixels per point.
fn forward(
    ui: &Ui,
    card: &mut VitalCard,
    key: u64,
    response: &egui::Response,
    rect: Rect,
    px: f32,
    now: f64,
) {
    let ms = (now * 1000.0) as i64;
    let at = |p: Pos2| ((p.x - rect.left()) * px, (p.y - rect.top()) * px);
    card.over = response.hovered();
    if ui.input(focus_lost)
        && let Some(f) = card.finger.take()
    {
        let (x, y) = at(f.at);
        gui::pointer(key, 2, x, y, 0, ms);
    }
    for f in fingers(ui, response.id) {
        match f {
            Finger::Down(id, pos) if card.finger.is_none() => {
                let (x, y) = at(pos);
                // The move first, so that the control under the finger has the mouse before the press.
                gui::pointer(key, 0, x, y, 0, ms);
                gui::pointer(key, 1, x, y, LEFT, ms);
                card.finger = Some(Finger0 {
                    id,
                    at: pos,
                    since: now,
                    moved: false,
                    right: false,
                });
            }
            Finger::Move(id, pos) => {
                if let Some(held) = card.finger.as_mut().filter(|h| h.id == id) {
                    let (x, y) = at(pos);
                    if pos.distance(held.at) > LONG_PT {
                        held.moved = true;
                    }
                    if !held.right {
                        gui::pointer(key, 0, x, y, LEFT, ms);
                    }
                } else if card.finger.is_none() && id == crate::ui::MOUSE && rect.contains(pos) {
                    // A mouse's hover: the interface highlights what it is over.
                    let (x, y) = at(pos);
                    gui::pointer(key, 0, x, y, 0, ms);
                }
            }
            Finger::Up(id, pos) => {
                if card.finger.is_some_and(|h| h.id == id) {
                    card.finger = None;
                    let (x, y) = at(pos);
                    gui::pointer(key, 2, x, y, 0, ms);
                }
            }
            _ => {}
        }
    }
    // A finger that has rested long enough presses the right button where it went down.
    if let Some(held) = card.finger.as_mut()
        && !held.moved
        && !held.right
    {
        if now - held.since >= LONG_S {
            let (x, y) = at(held.at);
            gui::pointer(key, 2, x, y, 0, ms);
            gui::pointer(key, 1, x, y, RIGHT, ms);
            held.right = true;
        } else {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_millis(50));
        }
    }
}

/// The wheel over the interface, in its pixels.
fn wheel(ui: &Ui, card: &VitalCard, key: u64, rect: Rect, px: f32, now: f64) {
    if !(card.over && ui.rect_contains_pointer(rect)) {
        return;
    }
    let (delta, pos) = ui.input(|i| (i.smooth_scroll_delta, i.pointer.hover_pos()));
    if delta == egui::Vec2::ZERO {
        return;
    }
    let pos = pos.unwrap_or(rect.center());
    // JUCE's wheel counts in notches of about 0.1: a notch of 50 points here.
    gui::wheel(
        key,
        (pos.x - rect.left()) * px,
        (pos.y - rect.top()) * px,
        delta.x / 500.0,
        delta.y / 500.0,
        (now * 1000.0) as i64,
    );
}

/// The text field of the interface that has the keyboard: Android's dialog types into it.
fn text(ctx: &egui::Context, card: &mut VitalCard, key: u64, id: Id) {
    let wants = gui::wants_text(key);
    if !wants {
        card.typing = false;
        return;
    }
    if !card.typing && !android_text::is_open(id) {
        card.typing = true;
        android_text::ask(ctx, id, "");
    }
    android_text::keep(id);
    if let Some(answer) = android_text::answer(id) {
        match answer {
            Some(text) => {
                for c in text.chars().filter(|c| !c.is_control()) {
                    gui::text(key, c);
                }
                gui::key(key, gui::Key::Return);
            }
            None => gui::key(key, gui::Key::Escape),
        }
    }
}

/// The interface's last frame in `rect`.
fn paint(ui: &Ui, rect: Rect, card: &VitalCard) {
    if let Some(t) = &card.texture {
        let uv = Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0));
        ui.painter().image(t.id(), rect, uv, Color32::WHITE);
    }
}

/// A dark body with `message`, for while the interface is not there.
fn stand_in(ui: &Ui, rect: Rect, message: &str) {
    ui.painter()
        .rect_filled(rect, 4.0, Color32::from_rgb(0x1a, 0x1c, 0x22));
    ui.painter().text(
        rect.center(),
        Align2::CENTER_CENTER,
        message,
        FontId::proportional(14.0),
        Color32::from_gray(0xb0),
    );
}

/// Closes the interface of module `instance_id` on channel `channel_id` (its card is gone): its GL surface goes.
pub(crate) fn release(channel_id: i32, instance_id: i32) {
    let key = vital::key(channel_id, instance_id);
    if CURRENT
        .compare_exchange(key, 0, Ordering::Relaxed, Ordering::Relaxed)
        .is_ok()
    {
        gui::close(key);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_body_has_vitals_aspect_until_a_frame_comes() {
        let h = body_height(None, 1400.0);
        assert!((h - 820.0).abs() < 1e-3);
        let card = VitalCard {
            size: (700, 700),
            ..VitalCard::default()
        };
        assert!((body_height(Some(&card), 300.0) - 300.0).abs() < 1e-3);
    }

    #[test]
    fn a_desktop_build_shows_no_interface() {
        if !gui::AVAILABLE {
            assert!(!available());
        }
    }
}
