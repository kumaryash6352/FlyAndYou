use crate::*;
impl World {
    fn collides(&self, x: f64, y: f64) -> bool {
        let x0 = ((x - 6.) / 4.).floor() as i32;
        let x1 = ((x + 6. - 1e-8) / 4.).floor() as i32;
        let y0 = ((y - 8.) / 4.).floor() as i32;
        let y1 = ((y + 8. - 1e-8) / 4.).floor() as i32;
        (y0..=y1).any(|cy| (x0..=x1).any(|cx| self.occupied(cx, cy)))
    }
    fn axis_move(&self, x: f64, y: f64, delta: f64, horizontal: bool) -> f64 {
        let mut lo = 0.;
        let mut hi = delta.abs();
        let sign = delta.signum();
        if !self.collides(
            x + if horizontal { delta } else { 0. },
            y + if horizontal { 0. } else { delta },
        ) {
            return delta;
        }
        for _ in 0..28 {
            let mid = (lo + hi) / 2.;
            if self.collides(
                x + if horizontal { sign * mid } else { 0. },
                y + if horizontal { 0. } else { sign * mid },
            ) {
                hi = mid
            } else {
                lo = mid
            }
        }
        sign * lo
    }
    pub fn step(&mut self, a: Action) -> bool {
        if self.outcome != Outcome::Running || !a.steer.is_finite() || a.steer.abs() > 1. {
            return false;
        }
        let target = a.steer * 176.;
        self.actor.vx += (target - self.actor.vx).clamp(-1056. * DT, 1056. * DT);
        self.actor.vy = (self.actor.vy + 1100. * DT).min(320.);
        let dx = self.actor.vx * DT;
        if self.actor.vx.abs() > 1.0 {
            self.facing = if self.actor.vx > 0.0 { 1 } else { -1 };
        }
        let dy = self.actor.vy * DT;
        let n = (dx.abs().max(dy.abs()) / 2.).ceil().max(1.) as usize;
        for _ in 0..n {
            let x = self.actor.x;
            let y = self.actor.y;
            let wanted = dx / n as f64;
            let moved = self.axis_move(x, y, wanted, true);
            if (moved - wanted).abs() > 1e-6 {
                let mut raised = false;
                if self.actor.grounded && self.step_up && wanted != 0. {
                    for lift in 1..=4 {
                        let lift = lift as f64;
                        if (self.axis_move(x, y, -lift, false) + lift).abs() < 1e-6
                            && !self.collides(x + wanted, y - lift)
                            && self.collides(x + wanted, y - lift + 0.02)
                        {
                            self.actor.y = y - lift;
                            self.actor.x = x + wanted;
                            raised = true;
                            break;
                        }
                    }
                }
                if !raised {
                    self.actor.x += moved;
                    self.actor.vx = 0.;
                }
            } else {
                self.actor.x += moved;
            }
            let wanted_y = dy / n as f64;
            let moved_y = self.axis_move(self.actor.x, self.actor.y, wanted_y, false);
            self.actor.y += moved_y;
            if (moved_y - wanted_y).abs() > 1e-6 {
                self.actor.vy = 0.;
                self.actor.y = (self.actor.y * 1e6).round() / 1e6;
            }
            self.actor.grounded = self.collides(self.actor.x, self.actor.y + 0.02);
            let bounds = [self.actor.x - 6., self.actor.y - 8., 12., 16.];
            if self.actor.y > HEIGHT as f64 + 16.
                || self.hazards.iter().any(|r| overlaps(*r, bounds))
            {
                self.outcome = Outcome::Failed;
                break;
            }
            if overlaps(bounds, self.goal) {
                self.outcome = Outcome::Won;
                break;
            }
        }
        self.tick += 1;
        true
    }
}
