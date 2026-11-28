//! Renders a frame the app put out to a PNG, in software: the way to look at the real screen from a test (CI uploads
//! the images, see `.github/workflows/screenshots.yml`). It is not a check.

use std::collections::HashMap;

use eframe::egui::{self, Color32, Pos2, Rect, epaint::Primitive, epaint::TextureId, pos2, vec2};

/// The textures egui has sent so far (the font atlas grows by patches), to sample when painting.
#[derive(Default)]
pub struct Textures(HashMap<TextureId, ([usize; 2], Vec<Color32>)>);

impl Textures {
    /// Takes in what a frame sent.
    pub fn absorb(&mut self, out: &egui::FullOutput) {
        for (id, delta) in &out.textures_delta.set {
            let egui::ImageData::Color(image) = &delta.image;
            let [w, h] = image.size;
            match delta.pos {
                None => {
                    self.0.insert(*id, (image.size, image.pixels.clone()));
                }
                Some([x0, y0]) => {
                    let Some((size, pixels)) = self.0.get_mut(id) else {
                        continue;
                    };
                    for y in 0..h {
                        let row = (y0 + y) * size[0] + x0;
                        pixels[row..row + w].copy_from_slice(&image.pixels[y * w..(y + 1) * w]);
                    }
                }
            }
        }
    }

    /// `out`'s shapes as an RGB image of `w` x `h` pixels (ppp 1), premultiplied "over" blending of every triangle
    /// sampled at pixel centres, from a black ground.
    pub fn paint(
        &self,
        ctx: &egui::Context,
        out: &egui::FullOutput,
        w: usize,
        h: usize,
    ) -> Vec<u8> {
        let ppp = 1.0f32;
        let mut image = vec![[0.0f32; 3]; w * h];
        let rgba = |c: Color32| c.to_array().map(|v| f32::from(v) / 255.0);
        let cross = |a: Pos2, b: Pos2, c: Pos2| (b - a).x * (c - a).y - (b - a).y * (c - a).x;
        for prim in ctx.tessellate(out.shapes.clone(), ppp) {
            let Primitive::Mesh(mesh) = prim.primitive else {
                continue;
            };
            let clip = prim
                .clip_rect
                .intersect(Rect::from_min_size(Pos2::ZERO, vec2(w as f32, h as f32)));
            let Some((tex_size, tex)) = self.0.get(&mesh.texture_id) else {
                continue;
            };
            for tri in mesh.indices.chunks_exact(3) {
                let v = [0, 1, 2].map(|k| mesh.vertices[tri[k] as usize]);
                let p = v.map(|v| v.pos);
                let area2 = cross(p[0], p[1], p[2]);
                if area2 == 0.0 {
                    continue;
                }
                let bounds = Rect::from_points(&p).intersect(clip);
                if !bounds.is_positive() {
                    continue;
                }
                for y in bounds.min.y.floor() as usize..(bounds.max.y.ceil() as usize).min(h) {
                    for x in bounds.min.x.floor() as usize..(bounds.max.x.ceil() as usize).min(w) {
                        let c = pos2(x as f32 + 0.5, y as f32 + 0.5);
                        let b = [
                            cross(p[1], p[2], c),
                            cross(p[2], p[0], c),
                            cross(p[0], p[1], c),
                        ]
                        .map(|e| e / area2);
                        if b.iter().any(|&e| e < 0.0) {
                            continue;
                        }
                        let uv = v[0].uv.to_vec2() * b[0]
                            + v[1].uv.to_vec2() * b[1]
                            + v[2].uv.to_vec2() * b[2];
                        let texel = |i: usize, n: f32| {
                            ((n * tex_size[i] as f32) as usize).min(tex_size[i] - 1)
                        };
                        let t = rgba(tex[texel(1, uv.y) * tex_size[0] + texel(0, uv.x)]);
                        let vc = v.map(|v| rgba(v.color));
                        let src: [f32; 4] = std::array::from_fn(|i| {
                            (vc[0][i] * b[0] + vc[1][i] * b[1] + vc[2][i] * b[2]) * t[i]
                        });
                        let dst = &mut image[y * w + x];
                        for i in 0..3 {
                            dst[i] = src[i] + dst[i] * (1.0 - src[3]);
                        }
                    }
                }
            }
        }
        image
            .iter()
            .flatten()
            .map(|&v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
            .collect()
    }
}

/// Writes `rgb` (`w` x `h`) to `path` as a PNG.
pub fn write_png(path: &std::path::Path, rgb: &[u8], w: usize, h: usize) {
    let file = std::fs::File::create(path).expect("create the png");
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()
        .expect("png header")
        .write_image_data(rgb)
        .expect("png data");
}
