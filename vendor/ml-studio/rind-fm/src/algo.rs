// SPDX-License-Identifier: GPL-3.0-or-later
//! The 32 DX7 algorithms as a picture: which operators are carriers, which modulate which, where the feedback runs.
//! The table is the engine's own (`cpp/fm_core.cc`): per operator (operator 6 first) the bus it reads, the bus it
//! writes, whether it adds to it, and the feedback flags.
use os_theme::design::lemon as brand;
use os_ui::kit::lemon::plug;
use os_ui::{Canvas, Color, Font, Rect};

const ALG: [[u8; 6]; 32] = [
    [0xc1, 0x11, 0x11, 0x14, 0x01, 0x14],
    [0x01, 0x11, 0x11, 0x14, 0xc1, 0x14],
    [0xc1, 0x11, 0x14, 0x01, 0x11, 0x14],
    [0x41, 0x11, 0x94, 0x01, 0x11, 0x14],
    [0xc1, 0x14, 0x01, 0x14, 0x01, 0x14],
    [0x41, 0x94, 0x01, 0x14, 0x01, 0x14],
    [0xc1, 0x11, 0x05, 0x14, 0x01, 0x14],
    [0x01, 0x11, 0xc5, 0x14, 0x01, 0x14],
    [0x01, 0x11, 0x05, 0x14, 0xc1, 0x14],
    [0x01, 0x05, 0x14, 0xc1, 0x11, 0x14],
    [0xc1, 0x05, 0x14, 0x01, 0x11, 0x14],
    [0x01, 0x05, 0x05, 0x14, 0xc1, 0x14],
    [0xc1, 0x05, 0x05, 0x14, 0x01, 0x14],
    [0xc1, 0x05, 0x11, 0x14, 0x01, 0x14],
    [0x01, 0x05, 0x11, 0x14, 0xc1, 0x14],
    [0xc1, 0x11, 0x02, 0x25, 0x05, 0x14],
    [0x01, 0x11, 0x02, 0x25, 0xc5, 0x14],
    [0x01, 0x11, 0x11, 0xc5, 0x05, 0x14],
    [0xc1, 0x14, 0x14, 0x01, 0x11, 0x14],
    [0x01, 0x05, 0x14, 0xc1, 0x14, 0x14],
    [0x01, 0x14, 0x14, 0xc1, 0x14, 0x14],
    [0xc1, 0x14, 0x14, 0x14, 0x01, 0x14],
    [0xc1, 0x14, 0x14, 0x01, 0x14, 0x04],
    [0xc1, 0x14, 0x14, 0x14, 0x04, 0x04],
    [0xc1, 0x14, 0x14, 0x04, 0x04, 0x04],
    [0xc1, 0x05, 0x14, 0x01, 0x14, 0x04],
    [0x01, 0x05, 0x14, 0xc1, 0x14, 0x04],
    [0x04, 0xc1, 0x11, 0x14, 0x01, 0x14],
    [0xc1, 0x14, 0x01, 0x14, 0x04, 0x04],
    [0x04, 0xc1, 0x11, 0x14, 0x04, 0x04],
    [0xc1, 0x14, 0x04, 0x04, 0x04, 0x04],
    [0xc4, 0x04, 0x04, 0x04, 0x04, 0x04],
];

/// An algorithm's operators and who modulates whom. Operators are numbered as a DX7 shows them (1..=6; the table's
/// first entry is operator 6).
pub struct Graph {
    /// Per operator 1..=6 (index op - 1): it sounds on its own.
    pub carrier: [bool; 6],
    /// Per operator: where its feedback is read (the operator it loops back into), if it writes feedback.
    pub feedback_from: [bool; 6],
    pub feedback_to: [bool; 6],
    /// (modulator, target), operator numbers.
    pub edges: Vec<(usize, usize)>,
}

pub fn graph(algorithm: usize) -> Graph {
    let alg = &ALG[algorithm.min(31)];
    let mut g = Graph { carrier: [false; 6], feedback_from: [false; 6], feedback_to: [false; 6], edges: Vec::new() };
    let mut bus: [Vec<usize>; 3] = [Vec::new(), Vec::new(), Vec::new()];
    for (i, &f) in alg.iter().enumerate() {
        let op = 6 - i;
        let (add, inb, outb) = (f & 4 != 0, usize::from((f >> 4) & 3), usize::from(f & 3));
        if inb != 0 {
            for &src in &bus[inb] {
                g.edges.push((src, op));
            }
        }
        if outb == 0 {
            g.carrier[op - 1] = true;
        } else if add {
            bus[outb].push(op);
        } else {
            bus[outb] = vec![op];
        }
        g.feedback_to[op - 1] = f & 0x40 != 0;
        g.feedback_from[op - 1] = f & 0x80 != 0;
    }
    g
}

/// Where each operator sits in a picture of `r`: layers from the carriers (bottom) up, a modulator over what it feeds.
fn layout(g: &Graph, r: Rect, box_w: f32, box_h: f32) -> [(f32, f32); 6] {
    let mut layer = [0usize; 6];
    // Targets always come later in the table (a lower operator number), so the lowest numbers settle first.
    for op in 1..=6 {
        for &(m, t) in &g.edges {
            if m == op {
                layer[op - 1] = layer[op - 1].max(layer[t - 1] + 1);
            }
        }
    }
    let top = *layer.iter().max().unwrap_or(&0);
    let y_of = |l: usize| {
        if top == 0 { r.cy() } else { r.bottom() - box_h / 2.0 - (r.h - box_h) * l as f32 / top as f32 }
    };
    let mut x = [0f32; 6];
    // The carriers across the width, in operator order; what feeds them stands over the mean of their targets.
    let carriers: Vec<usize> = (1..=6).filter(|&o| layer[o - 1] == 0).collect();
    for (j, &o) in carriers.iter().enumerate() {
        x[o - 1] = r.x + r.w * (j as f32 + 0.5) / carriers.len() as f32;
    }
    for l in 1..=top {
        let mut at: Vec<(f32, usize)> = Vec::new();
        for o in 1..=6 {
            if layer[o - 1] != l {
                continue;
            }
            let targets: Vec<f32> = g.edges.iter().filter(|e| e.0 == o).map(|e| x[e.1 - 1]).collect();
            let mean = if targets.is_empty() { r.cx() } else { targets.iter().sum::<f32>() / targets.len() as f32 };
            at.push((mean, o));
        }
        at.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let gap = box_w + 6.0;
        let mut last = f32::MIN;
        for (mean, o) in at {
            let px = mean.max(last + gap).max(r.x + box_w / 2.0);
            x[o - 1] = px.min(r.right() - box_w / 2.0);
            last = x[o - 1];
        }
    }
    std::array::from_fn(|i| (x[i], y_of(layer[i])))
}

/// Draws algorithm `algorithm` (0-based) in `r`.
pub fn draw(c: &mut Canvas, r: Rect, algorithm: usize, ops_on: [bool; 6]) {
    let g = graph(algorithm);
    let (bw, bh) = (30.0, 26.0);
    let pos = layout(&g, r, bw, bh);
    let line = Color::rgb(0x8a8a8a);
    for &(m, t) in &g.edges {
        let ((mx, my), (tx, ty)) = (pos[m - 1], pos[t - 1]);
        c.line(mx, my + bh / 2.0, tx, ty - bh / 2.0, 1.6, line);
    }
    // Feedback: a line from the operator that writes it round the right side back to the one that reads it.
    for from in (1..=6).filter(|&o| g.feedback_from[o - 1]) {
        for to in (1..=6).filter(|&o| g.feedback_to[o - 1]) {
            let ((fx, fy), (tx, ty)) = (pos[from - 1], pos[to - 1]);
            if from == to {
                let (x, y) = (fx + bw / 2.0, fy - bh / 2.0 + 3.0);
                c.polyline(&[(x, y + 6.0), (x + 9.0, y + 6.0), (x + 9.0, y - 7.0), (fx + 6.0, y - 7.0), (fx + 6.0, y)], 1.6, brand::MAIN);
            } else {
                let x = fx.max(tx) + bw / 2.0 + 9.0;
                c.polyline(&[(fx + bw / 2.0, fy), (x, fy), (x, ty), (tx + bw / 2.0, ty)], 1.6, brand::MAIN);
            }
        }
    }
    let font = Font::new(13).bold();
    for op in 1..=6 {
        let (x, y) = pos[op - 1];
        let b = Rect::new(x - bw / 2.0, y - bh / 2.0, bw, bh);
        let on = ops_on[op - 1];
        let (fill, ink) = match (g.carrier[op - 1], on) {
            (true, true) => (brand::MAIN, Color::rgb(0x1f1f1f)),
            (false, true) => (Color::rgb(0x5a5a5a), Color::rgb(0xf0f0f0)),
            (_, false) => (Color::rgb(0x2c2c2c), plug::OFF),
        };
        c.fill_rrect(b, 5.0, fill);
        c.text_center(font, b, ink, &op.to_string());
    }
}
