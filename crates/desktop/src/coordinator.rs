use std::time::Instant;
use wire_types::{Reply, Step};
use world_core::{Action, Outcome, World};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Loading,
    Paused,
    Waiting,
    Applying,
    Restoring,
    Fault,
}
pub struct Coordinator {
    pub world: World,
    pub phase: Phase,
    pub epoch: String,
    pub profile: String,
    pub step_id: u64,
    pub pending: Option<Step>,
    pub want_pause: bool,
    pub action: Action,
    pub remaining: u8,
    pub accumulator: f64,
    pub since: Instant,
    pub error: String,
}
fn epoch() -> String {
    let mut bytes = [0; 16];
    getrandom::fill(&mut bytes).expect("OS randomness");
    bytes.iter().map(|v| format!("{v:02x}")).collect()
}
impl Coordinator {
    pub fn new() -> Self {
        Self {
            world: World::bridge(),
            phase: Phase::Loading,
            epoch: epoch(),
            profile: String::new(),
            step_id: 0,
            pending: None,
            want_pause: true,
            action: Action {
                steer: 0.,
                jump: false,
            },
            remaining: 0,
            accumulator: 0.,
            since: Instant::now(),
            error: String::new(),
        }
    }
    pub fn loaded(&mut self, profile: String) {
        self.profile = profile;
        self.phase = Phase::Paused;
    }
    pub fn pause(&mut self) {
        self.want_pause = true;
        if self.phase == Phase::Paused {
            self.accumulator = 0.;
        }
    }
    pub fn play(&mut self) {
        if self.phase == Phase::Paused && self.world.outcome == Outcome::Running {
            self.want_pause = false;
        }
    }
    pub fn request(&mut self) -> Option<(Step, Vec<u8>)> {
        if self.phase != Phase::Paused || self.want_pause || self.world.outcome != Outcome::Running
        {
            return None;
        }
        let rgb = self.world.observe();
        let r = Step::new(
            self.epoch.clone(),
            self.step_id,
            self.world.tick,
            self.world.revision,
            self.profile.clone(),
            &rgb,
        );
        self.pending = Some(r.clone());
        self.phase = Phase::Waiting;
        self.since = Instant::now();
        self.accumulator = 0.;
        Some((r, rgb))
    }
    pub fn accept(&mut self, r: &Reply) -> Result<bool, String> {
        if r.epoch != self.epoch {
            return Ok(false);
        }
        let Some(p) = self.pending.as_ref() else {
            return Ok(false);
        };
        if self.phase != Phase::Waiting {
            return Ok(false);
        }
        r.validate(p)?;
        self.action = Action {
            steer: r.action.steer,
            jump: r.action.jump,
        };
        self.pending = None;
        self.phase = Phase::Applying;
        self.remaining = 10;
        self.accumulator = 0.;
        Ok(true)
    }
    pub fn advance(&mut self, seconds: f64) {
        if self.phase != Phase::Applying {
            return;
        }
        self.accumulator = (self.accumulator + seconds.max(0.)).min(0.05);
        while self.accumulator >= 0.01 && self.remaining > 0 {
            self.world.step(self.action);
            self.remaining -= 1;
            self.accumulator -= 0.01;
            // Complete the aligned block even when the body reaches a terminal outcome.
            if self.world.outcome != Outcome::Running {
                self.world.tick += self.remaining as u64;
                self.remaining = 0;
                self.want_pause = true;
            }
        }
        if self.remaining == 0 {
            self.step_id += 1;
            self.phase = Phase::Paused;
            self.accumulator = 0.;
        }
    }
    pub fn reset(&mut self) {
        self.epoch = epoch();
        self.phase = Phase::Restoring;
        self.pending = None;
        self.accumulator = 0.;
        self.remaining = 0;
        self.want_pause = true;
        self.error.clear();
    }
    pub fn restored(&mut self, w: World, step_id: u64) {
        self.world = w;
        self.step_id = step_id;
        self.action = Action {
            steer: 0.,
            jump: false,
        };
        self.phase = Phase::Paused;
        self.since = Instant::now();
    }
    pub fn fault(&mut self, e: String) {
        self.phase = Phase::Fault;
        self.want_pause = true;
        self.pending = None;
        self.accumulator = 0.;
        self.error = e;
    }
    pub fn label(&self) -> &str {
        match self.phase {
            Phase::Loading => "Waking the fly",
            Phase::Restoring => "Starting over",
            Phase::Fault => "Controller stopped",
            Phase::Waiting if self.want_pause => "Pausing",
            Phase::Waiting if self.since.elapsed().as_millis() > 250 => "Waiting for the fly",
            Phase::Waiting => "Running",
            Phase::Applying if self.want_pause => "Pausing",
            Phase::Applying => "Running",
            Phase::Paused if self.world.outcome == Outcome::Won => "Made it!",
            Phase::Paused if self.world.outcome == Outcome::Failed => "Try another idea",
            Phase::Paused if !self.want_pause => "Running",
            Phase::Paused => "Paused",
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn ready() -> Coordinator {
        let mut c = Coordinator::new();
        c.loaded("a".repeat(64));
        c.play();
        c
    }
    #[test]
    fn waiting_does_not_advance_and_stale_reply_cannot_move_reset() {
        let mut c = ready();
        let (r, _) = c.request().unwrap();
        c.advance(50.);
        assert_eq!(c.world.tick, 0);
        c.reset();
        assert!(!c.accept(&Reply::for_request(&r, 1.)).unwrap());
        assert_eq!(c.world.tick, 0);
    }
    #[test]
    fn duplicate_action_is_consumed_once() {
        let mut c = ready();
        let (r, _) = c.request().unwrap();
        let a = Reply::for_request(&r, 1.);
        assert!(c.accept(&a).unwrap());
        assert!(!c.accept(&a).unwrap());
        for _ in 0..10 {
            c.advance(0.01);
        }
        assert_eq!(c.world.tick, 10);
        assert!(!c.accept(&a).unwrap());
    }
    #[test]
    fn pause_waits_for_aligned_boundary() {
        let mut c = ready();
        let (r, _) = c.request().unwrap();
        c.pause();
        c.accept(&Reply::for_request(&r, 1.)).unwrap();
        for _ in 0..10 {
            c.advance(0.01);
        }
        assert_eq!(c.phase, Phase::Paused);
        assert_eq!(c.world.tick, 10);
        assert!(c.request().is_none());
        let h = c.world.state_hash();
        c.advance(500.);
        assert_eq!(h, c.world.state_hash());
    }
    #[test]
    fn frame_stall_cannot_fast_forward_entire_block() {
        let mut c = ready();
        let (r, _) = c.request().unwrap();
        c.accept(&Reply::for_request(&r, 1.)).unwrap();
        c.advance(50.);
        assert!(c.world.tick <= 5);
    }
}
