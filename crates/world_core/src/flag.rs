use crate::*;

/// Shared side-view paint coordinates for the pole and cloth.
fn original_flag_pixel(x: usize, y: usize) -> Option<[u8; 3]> {
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
    pub(crate) fn flag_pixel(&self, x: usize, y: usize) -> Option<[u8; 3]> {
        let x = x as i32 - (self.goal[0] as i32 - 560);
        let y = y as i32 - (self.goal[1] as i32 - 240);
        if x < 0 || y < 0 {
            return None;
        }
        original_flag_pixel(x as usize, y as usize)
    }
    pub fn paintable(&self, x: usize, y: usize) -> bool {
        if x >= WIDTH || y >= HEIGHT {
            return false;
        }
        self.ray_surface((x / 4) as i32, (y / 4) as i32) || self.flag_pixel(x, y).is_some()
    }

    /// Ray intersection with a cylindrical pole and a billowing cloth surface.
    /// The cloth is across the corridor, facing an approaching fly, and samples
    /// the exact paint applied to its corresponding side-view flag pixels.
    pub(crate) fn flag_hit(&self, ray: [f64; 3], limit: f64) -> Option<(f64, usize, usize, f64)> {
        let [dx, dy, dz] = ray;
        let mut hit = None;
        let mut nearest = limit;
        let offset_x = self.goal[0] - 560.;
        let offset_y = self.goal[1] - 240.;
        let ox = self.actor.x - (560.5 + offset_x);
        let oz = 9.;
        let a = dx * dx + dz * dz;
        let b = 2. * (ox * dx + oz * dz);
        let c = ox * ox + oz * oz - 1.0;
        let disc = b * b - 4. * a * c;
        if disc >= 0. {
            let t = (-b - disc.sqrt()) / (2. * a);
            let y = self.actor.y + dy * t;
            if t > 0.01 && t < nearest && (242. + offset_y..280. + offset_y).contains(&y) {
                let nx = (self.actor.x + dx * t - (560.5 + offset_x)).abs();
                hit = Some((t, (561. + offset_x) as usize, y as usize, 0.62 + nx * 0.3));
                nearest = t;
            }
        }
        // Each narrow strip has its own depth: genuine foreshortening/parallax,
        // not a flag pasted onto the corridor backdrop.
        for u in 0..17 {
            let wave = (u as f64 * 0.38 + self.tick as f64 * 0.09).sin() * u as f64 / 17. * 1.4;
            let t = (562. + offset_x + wave - self.actor.x) / dx;
            if t <= 0.01 || t >= nearest {
                continue;
            }
            let z = dz * t;
            let y = self.actor.y + dy * t;
            if z >= -9. + u as f64
                && z < -8. + u as f64
                && (242. + offset_y..257. + offset_y).contains(&y)
                && self
                    .flag_pixel((563. + offset_x) as usize + u, y as usize)
                    .is_some()
            {
                hit = Some((
                    t,
                    (563. + offset_x) as usize + u,
                    y as usize,
                    0.84 + 0.10 * (u as f64 * 0.38 + self.tick as f64 * 0.09).cos(),
                ));
                nearest = t;
            }
        }
        hit
    }
}
