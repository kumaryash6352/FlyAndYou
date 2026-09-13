use crate::*;
fn pixel(rgb: &mut [u8], x: i32, y: i32, c: [u8; 3]) {
    if (0..WIDTH as i32).contains(&x) && (0..HEIGHT as i32).contains(&y) {
        let i = (y as usize * WIDTH + x as usize) * 3;
        rgb[i..i + 3].copy_from_slice(&c);
    }
}
impl World {
    /// Spectator-only opening frame; normal observations always include the Goal.
    pub fn compose_without_goal(&self) -> Vec<u8> {
        self.compose_scene(false)
    }
    pub fn compose(&self) -> Vec<u8> {
        self.compose_scene(true)
    }
    fn compose_scene(&self, show_goal: bool) -> Vec<u8> {
        let mut rgb = vec![0; WIDTH * HEIGHT * 3];
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let noise = ((x as u32 * 73 + y as u32 * 151 + (x * y) as u32 * 7) % 19 == 0) as u8;
                let mut c = [235 - noise * 3, 234 - noise * 3, 218 - noise * 3];
                if x % 24 == 0 && y % 24 == 0 {
                    c = [207, 211, 190];
                }
                let i = (y / 4) * COLS + x / 4;
                if self.base[i] != 0 || self.solid[i] != 0 {
                    let top = y / 4 > 0 && !self.occupied((x / 4) as i32, (y / 4) as i32 - 1);
                    c = if self.base[i] != 0 {
                        [88, 103, 75]
                    } else {
                        [120, 124, 94]
                    };
                    if top && y % 4 < 2 {
                        c = [155, 166, 114];
                    }
                    if (x + 2 * y) % 17 < 2 {
                        c = c.map(|v| v.saturating_sub(16));
                    }
                }
                rgb[(y * WIDTH + x) * 3..(y * WIDTH + x) * 3 + 3].copy_from_slice(&c);
            }
        }
        // Original pixel scenery is part of the canonical sensory composition.
        for &(cx, cy) in &[(82, 65), (328, 99), (496, 52)] {
            for yy in -2..=2i32 {
                for xx in -15..=15i32 {
                    if xx.abs() + yy.abs() * 4 < 18 {
                        pixel(&mut rgb, cx + xx, cy + yy, [215, 220, 201]);
                    }
                }
            }
        }
        for y in 242..280 {
            for x in 560..580 {
                if show_goal && let Some(c) = crate::flag::flag_pixel(x, y) {
                    pixel(&mut rgb, x as i32, y as i32, c);
                }
            }
        }
        for y in 352..360 {
            for x in 192..288 {
                pixel(
                    &mut rgb,
                    x,
                    y,
                    if (x + y) % 8 < 3 {
                        [154, 84, 55]
                    } else {
                        [192, 139, 98]
                    },
                );
            }
        }
        for (p, ink) in self.paint.chunks_exact(4).enumerate() {
            if ink[3] == 255
                && self.paintable(p % WIDTH, p / WIDTH)
                && (show_goal || crate::flag::flag_pixel(p % WIDTH, p / WIDTH).is_none())
            {
                rgb[p * 3..p * 3 + 3].copy_from_slice(&ink[..3]);
            }
        }
        rgb
    }
}
