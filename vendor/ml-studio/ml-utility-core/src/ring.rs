// SPDX-License-Identifier: GPL-3.0-or-later
// From ML Studio's `os_engine::effects::delay::Ring` (Honeydew keeps it here: Lemon Studio's own is private).

/// Flush to zero: the original runs with denormals off.
#[inline]
fn ftz(x: f32) -> f32 {
    if x.abs() < f32::MIN_POSITIVE { 0.0 } else { x }
}

/// Four-point Catmull-Rom interpolation between the two middle samples.
#[inline]
fn cubic(a: f32, b: f32, c: f32, d: f32, f: f32) -> f32 {
    b + 0.5 * f * ((c - a) + f * ((2.0 * a - 5.0 * b + 4.0 * c - d) + f * (3.0 * (b - c) + d - a)))
}

/// A power-of-two ring read at fractional delays.
#[derive(Clone)]
pub struct Ring {
    buf: Vec<f32>,
    pos: usize,
}

impl Ring {
    pub fn new(len: usize) -> Ring {
        Ring { buf: vec![0.0; len], pos: 0 }
    }
    #[inline]
    pub fn read(&self, d: f32) -> f32 {
        let m = self.buf.len() - 1;
        let j = d.trunc() as i64 as u32;
        let f = d - j as f32;
        let i = self.pos.wrapping_sub(j as usize);
        (1.0 - f) * self.buf[i & m] + f * self.buf[i.wrapping_sub(1) & m]
    }
    #[inline]
    pub fn read_clean(&self, delay: f32) -> f32 {
        if !(2.0..(self.buf.len() - 2) as f32).contains(&delay) {
            return self.read(delay);
        }
        let mask = self.buf.len() - 1;
        let whole = delay as usize;
        let index = self.pos.wrapping_sub(whole);
        let at = |offset: usize| self.buf[index.wrapping_sub(offset) & mask];
        cubic(at(usize::MAX), at(0), at(1), at(2), delay - whole as f32)
    }
    #[inline]
    pub fn write(&mut self, v: f32) {
        let m = self.buf.len() - 1;
        self.buf[self.pos] = ftz(v);
        self.pos = (self.pos + 1) & m;
    }
    pub fn clear(&mut self) {
        self.buf.fill(0.0);
    }
}
