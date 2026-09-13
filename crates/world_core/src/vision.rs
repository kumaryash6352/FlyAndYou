//! A deterministic eye-level perspective of the editable side-on world.
//!
//! XY collision cells are extruded across a 64-unit corridor. The canonical
//! actor-free composition textures solid surfaces and the flag model. Unpainted
//! surfaces are grayscale; only player-applied surface paint introduces color. No GPU readback or HUD enters
//! the controller image. The ray origin and heading come only from the body.
use crate::*;

const HALF_DEPTH: f64 = 32.;
const FAR: f64 = 768.;
const TAN_HALF_FOV: f64 = 1.4281480067421144; // 110 degree horizontal field of view

impl World {
    fn surface_texel(&self, rgb: &[u8], x: f64, y: f64) -> [u8; 3] {
        if !(0.0..WIDTH as f64).contains(&x) || !(0.0..HEIGHT as f64).contains(&y) {
            return [221; 3];
        }
        let x = x.floor() as usize;
        let y = y.floor() as usize;
        let pixel = y * WIDTH + x;
        let i = pixel * 3;
        let c = [rgb[i], rgb[i + 1], rgb[i + 2]];
        if self.paint[pixel * 4 + 3] == 255 && self.paintable(x, y) {
            return c;
        }
        let grey = ((c[0] as u16 * 77 + c[1] as u16 * 150 + c[2] as u16 * 29) / 256) as u8;
        [grey; 3]
    }
    pub fn observe(&self) -> Vec<u8> {
        self.observe_from(&self.compose())
    }

    pub fn observe_from(&self, rgb: &[u8]) -> Vec<u8> {
        assert_eq!(rgb.len(), WIDTH * HEIGHT * 3);
        let mut out = vec![0; 128 * 96 * 3];
        for py in 0..96 {
            for px in 0..128 {
                // Pinhole rays. Map Y points down; camera-right points into Z.
                let dz = self.facing as f64 * ((px as f64 + 0.5) / 64. - 1.) * TAN_HALF_FOV;
                let dy = ((py as f64 + 0.5) / 48. - 1.) * TAN_HALF_FOV * 0.75;
                let dx = self.facing as f64;
                let ray_len = (1. + dy * dy + dz * dz).sqrt();
                let wall_t = (HALF_DEPTH / dz.abs()).min(FAR);
                let mut t = wall_t;
                let mut shade = 0.88;
                let grain = ((self.actor.x + dx * t) as i32 / 8
                    + (self.actor.y + dy * t) as i32 / 8)
                    .rem_euclid(23)
                    == 0;
                let mut c = [if grain { 215 } else { 221 }; 3];
                let mut cx = (self.actor.x / 4.).floor() as i32;
                let mut cy = (self.actor.y / 4.).floor() as i32;
                let sx = self.facing as i32;
                let sy = if dy >= 0. { 1 } else { -1 };
                let mut tx = if sx > 0 {
                    ((cx + 1) as f64 * 4. - self.actor.x) / dx
                } else {
                    (cx as f64 * 4. - self.actor.x) / dx
                };
                let mut ty = if sy > 0 {
                    ((cy + 1) as f64 * 4. - self.actor.y) / dy
                } else {
                    (cy as f64 * 4. - self.actor.y) / dy
                };
                let step_y = 4. / dy.abs();
                // Exact grid-boundary traversal; a thin drawn ledge cannot be skipped.
                for _ in 0..400 {
                    let horizontal_face = ty < tx;
                    let hit_t = if horizontal_face {
                        let v = ty;
                        ty += step_y;
                        cy += sy;
                        v
                    } else {
                        let v = tx;
                        tx += 4.;
                        cx += sx;
                        v
                    };
                    if hit_t >= t {
                        break;
                    }
                    if self.ray_surface(cx, cy) {
                        t = hit_t;
                        let x = self.actor.x + dx * (t + 0.001);
                        let y = self.actor.y + dy * (t + 0.001);
                        c = if !(0..COLS as i32).contains(&cx) {
                            [95; 3]
                        } else {
                            self.surface_texel(rgb, x, y)
                        };
                        shade = if horizontal_face { 1.0 } else { 0.68 };
                        break;
                    }
                }
                if let Some((hit, x, y, light)) = self.flag_hit([dx, dy, dz], t) {
                    t = hit;
                    c = self.surface_texel(rgb, x as f64, y as f64);
                    shade = light;
                }
                let fog = (t * ray_len / FAR).clamp(0., 0.85);
                let sky = [221.; 3];
                for ch in 0..3 {
                    out[(py * 128 + px) * 3 + ch] =
                        (c[ch] as f64 * shade * (1. - fog) + sky[ch] * fog) as u8;
                }
            }
        }
        out
    }
}
