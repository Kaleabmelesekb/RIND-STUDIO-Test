// Copyright (C) 2026 The Honeydew contributors
// SPDX-License-Identifier: GPL-3.0-or-later

//! Fingers, one by one. The host window sends an editor one pointer ([`os_plugkit::gui::Input`]); a synth wants several
//! at once (a chord on the keyboard while a wheel is held), so the host hands Rind's editor every finger through this
//! queue and the editor drains it in its own thread. Positions are the editor's device px.

use std::collections::{HashMap, VecDeque};
use std::sync::{Mutex, OnceLock};

/// One step of a finger.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Touch {
    /// `dbl`: the second tap of a double tap.
    Down {
        id: u64,
        x: f32,
        y: f32,
        dbl: bool,
    },
    Move {
        id: u64,
        x: f32,
        y: f32,
    },
    Up {
        id: u64,
    },
}

type Queues = Mutex<HashMap<u64, VecDeque<Touch>>>;

fn queues() -> &'static Queues {
    static Q: OnceLock<Queues> = OnceLock::new();
    Q.get_or_init(Default::default)
}

/// Queues `t` for editor `editor` (its host handle).
pub fn send(editor: u64, t: Touch) {
    if let Ok(mut q) = queues().lock() {
        q.entry(editor).or_default().push_back(t);
    }
}

/// What editor `editor` has not taken yet, oldest first.
pub fn take(editor: u64) -> Vec<Touch> {
    queues()
        .lock()
        .ok()
        .and_then(|mut q| q.get_mut(&editor).map(|d| d.drain(..).collect()))
        .unwrap_or_default()
}

/// Editor `editor` is gone.
pub fn forget(editor: u64) {
    if let Ok(mut q) = queues().lock() {
        q.remove(&editor);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingers_queue_per_editor_in_order() {
        let (a, b) = (9_001, 9_002);
        send(
            a,
            Touch::Down {
                id: 1,
                x: 1.0,
                y: 2.0,
                dbl: false,
            },
        );
        send(b, Touch::Up { id: 7 });
        send(
            a,
            Touch::Move {
                id: 1,
                x: 3.0,
                y: 4.0,
            },
        );
        assert_eq!(
            take(a),
            [
                Touch::Down {
                    id: 1,
                    x: 1.0,
                    y: 2.0,
                    dbl: false
                },
                Touch::Move {
                    id: 1,
                    x: 3.0,
                    y: 4.0
                }
            ]
        );
        assert!(take(a).is_empty());
        assert_eq!(take(b), [Touch::Up { id: 7 }]);
        forget(b);
        assert!(take(b).is_empty());
    }
}
