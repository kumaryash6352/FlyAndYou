use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MotorConfig {
    pub explore_speed: f64,
    pub retreat_speed: f64,
    pub retreat_decisions: u32,
    pub search_decisions: u32,
    pub search_commit_decisions: u32,
    pub enter_threshold: f64,
    pub release_threshold: f64,
    pub red_dominance: f64,
    pub clear_decisions: u32,
    pub approach_half_response: f64,
}
impl MotorConfig {
    pub fn validate(&self) -> Result<(), String> {
        if self.retreat_decisions != 20
            || self.search_decisions != 88
            || self.search_commit_decisions != 10
            || self.clear_decisions != 8
            || ![
                self.explore_speed,
                self.retreat_speed,
                self.enter_threshold,
                self.release_threshold,
                self.red_dominance,
                self.approach_half_response,
            ]
            .iter()
            .all(|x| x.is_finite())
            || !(0.0..=1.0).contains(&self.explore_speed)
            || self.explore_speed == 0.
            || !(0.0..=1.0).contains(&self.retreat_speed)
            || self.retreat_speed == 0.
            || self.release_threshold < 0.
            || self.release_threshold >= self.enter_threshold
            || self.red_dominance < 1.
            || self.approach_half_response <= 0.
        {
            return Err("Invalid 25 Hz motor profile".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MotorState {
    pub heading: i8,
    pub mode: String,
    pub remaining: u32,
    pub armed: bool,
    pub clear: u32,
    pub search_age: u32,
    pub steer: f64,
}
impl Default for MotorState {
    fn default() -> Self {
        Self {
            heading: 1,
            mode: "search".into(),
            remaining: 0,
            armed: true,
            clear: 0,
            search_age: 0,
            steer: 0.,
        }
    }
}
impl MotorState {
    pub fn validate(&self, c: &MotorConfig) -> Result<(), String> {
        if ![-1, 1].contains(&self.heading)
            || !["search", "approach", "retreat", "hold"].contains(&self.mode.as_str())
            || self.remaining >= c.retreat_decisions.max(c.search_commit_decisions)
            || (self.remaining > 0 && !["search", "retreat"].contains(&self.mode.as_str()))
            || self.clear > c.clear_decisions
            || self.search_age >= c.search_decisions
            || !self.steer.is_finite()
            || self.steer.abs() > 1.
        {
            return Err("Invalid motor checkpoint".into());
        }
        Ok(())
    }
    pub fn step(&mut self, approach: f64, avoidance: f64, c: &MotorConfig) -> f64 {
        let threat = avoidance > c.enter_threshold && avoidance > approach * c.red_dominance;
        self.clear = if avoidance < c.release_threshold {
            (self.clear + 1).min(c.clear_decisions)
        } else {
            0
        };
        if self.clear == c.clear_decisions {
            self.armed = true;
        }
        if self.remaining == 0 {
            if threat && self.armed {
                self.heading *= -1;
                self.mode = "retreat".into();
                self.remaining = c.retreat_decisions;
                self.armed = false;
                self.search_age = 0;
            } else if threat {
                self.mode = "hold".into();
            } else if approach > c.enter_threshold && approach >= avoidance {
                self.mode = "approach".into();
                self.search_age = 0;
            } else {
                self.mode = "search".into();
                self.search_age += 1;
                if self.search_age >= c.search_decisions {
                    self.heading *= -1;
                    self.remaining = c.search_commit_decisions;
                    self.search_age = 0;
                }
            }
        }
        let speed = match self.mode.as_str() {
            "retreat" => c.retreat_speed,
            "hold" => 0.,
            "approach" => {
                c.explore_speed
                    + (1. - c.explore_speed) * approach / (approach + c.approach_half_response)
            }
            _ => c.explore_speed,
        };
        self.remaining = self.remaining.saturating_sub(1);
        self.steer = self.heading as f64 * speed;
        self.steer
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn cfg() -> MotorConfig {
        MotorConfig {
            explore_speed: 0.52,
            retreat_speed: 0.85,
            retreat_decisions: 20,
            search_decisions: 88,
            search_commit_decisions: 10,
            enter_threshold: 0.08,
            release_threshold: 0.03,
            red_dominance: 1.25,
            clear_decisions: 8,
            approach_half_response: 0.25,
        }
    }
    #[test]
    fn retreat_duration_and_mid_turn_restore() {
        let c = cfg();
        let mut s = MotorState::default();
        assert_eq!(s.step(0., 1., &c), -0.85);
        let mut restored: MotorState =
            serde_json::from_str(&serde_json::to_string(&s).unwrap()).unwrap();
        restored.validate(&c).unwrap();
        for _ in 1..20 {
            assert_eq!(s.step(1., 0., &c), -0.85);
            restored.step(1., 0., &c);
            assert_eq!(s, restored);
        }
        s.step(1., 0., &c);
        assert_eq!(s.mode, "approach");
    }
}
