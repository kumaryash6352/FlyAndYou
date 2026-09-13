//! Musical controls derived only from accepted modeled rates, never screen pixels.
use super::synth::Controls;

pub(super) const ANATOMY_COUNT: usize = 336;

pub(super) struct Signals {
    baseline: [f32; ANATOMY_COUNT],
    previous_tick: Option<u64>,
    tone: u8,
}

impl Default for Signals {
    fn default() -> Self {
        Self {
            baseline: [0.; ANATOMY_COUNT],
            previous_tick: None,
            tone: 0,
        }
    }
}

impl Signals {
    pub(super) fn observe(
        &mut self,
        tick: u64,
        expected_tick: u64,
        approach: Option<f64>,
        avoidance: Option<f64>,
        rates: &[f32],
    ) -> Option<Controls> {
        let (approach, avoidance) = (approach?, avoidance?);
        if tick != expected_tick
            || tick == 0
            || self.previous_tick.is_some_and(|previous| tick <= previous)
            || rates.len() != ANATOMY_COUNT
            || rates
                .iter()
                .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
            || [approach, avoidance]
                .iter()
                .any(|v| !v.is_finite() || *v < 0.)
        {
            return None;
        }
        let mut groups = [0f32; 5];
        if let Some(previous) = self.previous_tick {
            // Baseline follows simulated neural time, regardless of rendering rate.
            let elapsed = ((tick - previous) as f32 * 0.02).min(1.);
            let follow = 1. - (-elapsed / 1.2).exp();
            for (i, &rate) in rates.iter().enumerate() {
                let baseline = self.baseline[i];
                let rise = ((rate - baseline - 0.00003) / (baseline + 0.0005) - 0.1).clamp(0., 1.);
                // Interleaved pitch groups are a musical assignment, not anatomy.
                groups[i % 5] += rise;
                self.baseline[i] += follow * (rate - baseline);
            }
        } else {
            self.baseline.copy_from_slice(rates);
        }
        self.previous_tick = Some(tick);
        let activity = (groups.iter().sum::<f32>() / ANATOMY_COUNT as f32).sqrt();
        if activity > 0.05 {
            self.tone = groups
                .iter()
                .enumerate()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .map_or(0, |(i, _)| i as u8);
        }
        Some(Controls {
            approach: (approach / (approach + 0.25)) as f32,
            avoidance: (avoidance / (avoidance + 0.25)) as f32,
            activity,
            tone: self.tone,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_neural_evidence_is_bounded_without_losing_its_direction() {
        let mut signals = Signals::default();
        let rates = [0.0002; ANATOMY_COUNT];
        let yellow = signals.observe(2, 2, Some(0.5), Some(0.), &rates).unwrap();
        assert!(yellow.approach > 0.5 && yellow.approach < 1.);
        assert_eq!(yellow.avoidance, 0.);
        assert_eq!(yellow.activity, 0., "First sample establishes a baseline");
        let red = signals
            .observe(4, 4, Some(0.), Some(f64::MAX), &rates)
            .unwrap();
        assert_eq!(red.approach, 0.);
        assert!(red.avoidance.is_finite() && red.avoidance <= 1.);
        assert!(red.avoidance > 0.99);
    }

    #[test]
    fn duplicate_stale_and_invalid_samples_cannot_change_the_baseline() {
        let mut signals = Signals::default();
        let rates = [0.0002; ANATOMY_COUNT];
        signals.observe(4, 4, Some(0.), Some(0.), &rates).unwrap();
        assert!(
            signals
                .observe(4, 4, Some(1.), Some(0.), &[1.; ANATOMY_COUNT])
                .is_none()
        );
        assert!(signals.observe(2, 2, Some(0.), Some(0.), &rates).is_none());
        assert!(signals.observe(6, 8, Some(0.), Some(0.), &rates).is_none());
        assert!(
            signals
                .observe(6, 6, Some(f64::NAN), Some(0.), &rates)
                .is_none()
        );
        assert!(signals.observe(6, 6, Some(-1.), Some(0.), &rates).is_none());
        assert!(signals.observe(6, 6, Some(0.), Some(0.), &[]).is_none());
        assert!(
            signals
                .observe(6, 6, Some(0.), Some(0.), &[f32::INFINITY; ANATOMY_COUNT])
                .is_none()
        );
        assert!(
            signals
                .observe(6, 6, Some(0.), Some(0.), &[-0.1; ANATOMY_COUNT])
                .is_none()
        );
        assert_eq!(
            signals
                .observe(6, 6, Some(0.), Some(0.), &rates)
                .unwrap()
                .activity,
            0.
        );
    }

    #[test]
    fn neural_changes_add_accents_but_stable_background_does_not() {
        let mut signals = Signals::default();
        let mut rates = [0.0002; ANATOMY_COUNT];
        signals.observe(2, 2, Some(0.), Some(0.), &rates).unwrap();
        rates[17] += 0.000001;
        assert_eq!(
            signals
                .observe(4, 4, Some(0.), Some(0.), &rates)
                .unwrap()
                .activity,
            0.
        );
        rates[..32].fill(0.02);
        let changed = signals.observe(6, 6, Some(0.), Some(0.), &rates).unwrap();
        assert!(changed.activity > 0. && changed.activity <= 1.);
        assert!(changed.tone < 5);
        for tick in 4..400 {
            signals
                .observe(tick * 2, tick * 2, Some(0.), Some(0.), &rates)
                .unwrap();
        }
        assert_eq!(
            signals
                .observe(800, 800, Some(0.), Some(0.), &rates)
                .unwrap()
                .activity,
            0.
        );
        let mut restored = Signals::default();
        assert_eq!(
            restored
                .observe(2, 2, Some(0.), Some(0.), &rates)
                .unwrap()
                .activity,
            0.
        );
    }
}
