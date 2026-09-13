use crate::*;

/// Shared side-view paint coordinates for the pole and cloth.
pub(crate) fn flag_pixel(x: usize, y: usize) -> Option<[u8; 3]> {
    if (560..563).contains(&x) && (242..280).contains(&y) {
        return Some([54, 63, 48]);
    }
    if (242..257).contains(&y)
        && (563..580 - (y as i32 - 249).unsigned_abs() as usize / 2).contains(&x)
    {
        return Some([189, 83, 55]);
    }
    None
}

impl World {
    pub fn paintable(&self, x: usize, y: usize) -> bool {
        if x >= WIDTH || y >= HEIGHT {
            return false;
        }
        let i = (y / 4) * COLS + x / 4;
        self.base[i] != 0
            || self.solid[i] != 0
            || flag_pixel(x, y).is_some()
            || self
                .hazards
                .iter()
                .any(|r| overlaps(*r, [x as f64, y as f64, 1., 1.]))
    }

    /// Ray intersection with a cylindrical pole and a billowing cloth surface.
    /// The cloth is across the corridor, facing an approaching fly, and samples
    /// the exact paint applied to its corresponding side-view flag pixels.
    pub(crate) fn flag_hit(&self, ray: [f64; 3], limit: f64) -> Option<(f64, usize, usize, f64)> {
        let [dx, dy, dz] = ray;
        let mut hit = None;
        let mut nearest = limit;
        let ox = self.actor.x - 560.5;
        let oz = 9.;
        let a = dx * dx + dz * dz;
        let b = 2. * (ox * dx + oz * dz);
        let c = ox * ox + oz * oz - 1.0;
        let disc = b * b - 4. * a * c;
        if disc >= 0. {
            let t = (-b - disc.sqrt()) / (2. * a);
            let y = self.actor.y + dy * t;
            if t > 0.01 && t < nearest && (242.0..280.0).contains(&y) {
                let nx = (self.actor.x + dx * t - 560.5).abs();
                hit = Some((t, 561, y as usize, 0.62 + nx * 0.3));
                nearest = t;
            }
        }
        // Each narrow strip has its own depth: genuine foreshortening/parallax,
        // not a flag pasted onto the corridor backdrop.
        for u in 0..17 {
            let wave = (u as f64 * 0.38 + self.tick as f64 * 0.09).sin() * u as f64 / 17. * 1.4;
            let t = (562. + wave - self.actor.x) / dx;
            if t <= 0.01 || t >= nearest {
                continue;
            }
            let z = dz * t;
            let y = self.actor.y + dy * t;
            if z >= -9. + u as f64
                && z < -8. + u as f64
                && (242.0..257.0).contains(&y)
                && flag_pixel(563 + u, y as usize).is_some()
            {
                hit = Some((
                    t,
                    563 + u,
                    y as usize,
                    0.84 + 0.10 * (u as f64 * 0.38 + self.tick as f64 * 0.09).cos(),
                ));
                nearest = t;
            }
        }
        hit
    }
}
