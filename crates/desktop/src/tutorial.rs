const DURATIONS: [f32; 16] = [
    // The nervous-system explanation spans beats 9–10: 12 s to read and
    // notice the new views, followed by a 3 s hold before offering help.
    0.85, 0.7, 2.4, 1., 0.7, 2.4, 1.1, 2.6, 2.5, 12., 3., 2.2, 2.6, 4.2, 4.2, 3.7,
];
pub const READY: usize = 16;
pub const DONE: usize = 17;

/// Presentation time never advances the world or the neural controller.
#[derive(Default)]
pub struct Tutorial {
    pub beat: usize,
    pub elapsed: f32,
    intro_started: bool,
    start_pending: bool,
}

impl Tutorial {
    pub fn for_level(level: usize) -> Self {
        if level == 0 {
            return Self::default();
        }
        Self {
            beat: DONE,
            elapsed: 0.,
            intro_started: true,
            start_pending: false,
        }
    }
    pub fn awaiting_start(&self) -> bool {
        !self.intro_started
    }
    pub fn start_intro(&mut self) -> bool {
        if self.intro_started {
            return false;
        }
        self.intro_started = true;
        true
    }
    pub fn advance(&mut self, seconds: f32) {
        if self.awaiting_start() || self.beat >= READY {
            return;
        }
        self.elapsed += seconds.max(0.);
        while self.beat < READY && self.elapsed >= DURATIONS[self.beat] {
            self.elapsed -= DURATIONS[self.beat];
            self.beat += 1;
        }
        if self.beat == READY {
            self.elapsed = 0.;
        }
    }
    pub fn progress(&self) -> f32 {
        DURATIONS
            .get(self.beat)
            .map_or(1., |d| (self.elapsed / d).clamp(0., 1.))
    }
    pub fn show_rail(&self) -> bool {
        self.beat >= 9
    }
    pub fn rail_fraction(&self) -> f32 {
        if self.beat == 9 {
            smooth(self.elapsed / 0.7)
        } else if self.show_rail() {
            1.
        } else {
            0.
        }
    }
    pub fn can_paint(&self) -> bool {
        self.beat >= READY
    }
    pub fn active(&self) -> bool {
        self.beat < DONE
    }
    pub fn show_tools(&self) -> bool {
        self.beat >= 12
    }
    pub fn demo(&self) -> Option<usize> {
        match self.beat {
            13 => Some(0),
            14 => Some(1),
            _ => None,
        }
    }
    pub fn begin_paint(&mut self) -> bool {
        if self.beat != READY {
            return false;
        }
        self.beat = DONE;
        self.start_pending = true;
        true
    }
    pub fn take_start(&mut self) -> bool {
        std::mem::take(&mut self.start_pending)
    }
    pub fn skip(&mut self) {
        if self.awaiting_start() {
            return;
        }
        self.beat = READY;
        self.elapsed = 0.;
        self.start_pending = false;
    }
    pub fn cancel_start(&mut self) {
        self.start_pending = false;
    }
    pub fn caption(&self) -> &'static str {
        match self.beat {
            2 => "This is Fly",
            5 => "That is the Goal",
            7 => "Fly needs to get to Goal",
            8 => "But Fly does not know that",
            9 | 10 => {
                "Because Fly is the simulated nervous system of a male fruit fly (drosophila melanogaster) receiving visual stimulation and desiring left/right movement"
            }
            11 => "You can help Fly!",
            12 => "Use Paint to guide Fly to Goal",
            13 => "Fly is scared of Red",
            14 => "Yellow looks like juicy fruit",
            15 => "Fly is a fruit fly, so the other colors make Fly do fruit fly things",
            READY => "Fly will begin moving when you start painting",
            _ => "",
        }
    }
}

pub fn smooth(t: f32) -> f32 {
    let t = t.clamp(0., 1.);
    t * t * (3. - 2. * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn later_levels_are_ready_without_replaying_or_autostarting_tutorial() {
        for level in 1..6 {
            let mut intro = Tutorial::for_level(level);
            assert!(!intro.awaiting_start());
            assert!(!intro.active());
            assert!(intro.can_paint());
            assert!(intro.show_rail());
            assert!(!intro.take_start());
        }
        assert!(Tutorial::for_level(0).awaiting_start());
    }

    #[test]
    fn start_screen_holds_the_opening_and_enter_starts_only_once() {
        let mut intro = Tutorial::default();
        intro.advance(120.);
        assert!(intro.awaiting_start());
        assert_eq!(intro.beat, 0);
        assert_eq!(intro.elapsed, 0.);
        assert!(!intro.begin_paint());
        assert!(!intro.take_start());
        assert!(intro.start_intro());
        intro.advance(1.);
        assert!(!intro.awaiting_start());
        assert_eq!(intro.beat, 1);
        let elapsed = intro.elapsed;
        assert!(!intro.start_intro());
        assert_eq!(intro.beat, 1);
        assert_eq!(intro.elapsed, elapsed);
        assert!(!intro.take_start());
    }

    #[test]
    fn narration_holds_until_paint_and_hands_off_only_once() {
        let mut intro = Tutorial::default();
        intro.start_intro();
        assert!(intro.active());
        assert!(!intro.begin_paint());
        assert!(!intro.take_start());
        intro.advance(120.);
        assert!(intro.active());
        assert!(intro.can_paint());
        assert!(!intro.take_start());
        assert!(intro.begin_paint());
        assert!(!intro.active());
        assert!(intro.take_start());
        assert!(!intro.take_start());
        assert!(!intro.begin_paint());
        assert!(!intro.take_start());
    }

    #[test]
    fn brain_and_camera_reveal_on_nervous_system_line() {
        let mut intro = Tutorial::default();
        intro.start_intro();
        intro.advance(14.2);
        assert!(!intro.show_rail());
        intro.advance(0.1);
        assert!(intro.show_rail());
    }
}
