//! A small authored score whose expression follows neural telemetry.
//!
//! The audio thread owns all state. Rendering and control changes neither allocate
//! nor lock. The three voices use a few low sine partials, with no sample assets,
//! random generator, or dependence on the simulation's decision cadence.

use std::f64::consts::TAU;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Controls {
    pub approach: f32,
    pub avoidance: f32,
    pub activity: f32,
    pub tone: u8,
}

const PENTATONIC: [f64; 5] = [293.6648, 329.6276, 369.9944, 440.0, 493.8833];
const MOTIF: [usize; 8] = [0, 2, 4, 2, 1, 3, 2, 1];

pub(crate) struct Synth {
    sample_rate: u32,
    target: Controls,
    smoothed: Controls,
    smoothing: f32,
    playing: bool,
    playing_gain: Ramp,
    volume: Ramp,
    reset_remaining: u32,
    // Three eighth notes per second is exactly 90 quarter-note beats/minute.
    // Integer phase accumulation avoids accumulating sample rounding error.
    clock: u64,
    step: usize,
    tick_pending: bool,
    pluck: Transient,
    accent: Transient,
    bed_phase: [f64; 2],
    bed_frequency: f64,
}

impl Synth {
    pub(crate) fn new(sample_rate: u32) -> Self {
        // All partials stay below Nyquist at 8 kHz and above. Audio devices use
        // higher rates; this also gives nonsensical caller input a safe fallback.
        let sample_rate = sample_rate.max(8_000);
        Self {
            sample_rate,
            target: Controls::default(),
            smoothed: Controls::default(),
            smoothing: (1.0 - (-1.0 / (sample_rate as f64 * 0.08)).exp()) as f32,
            playing: false,
            playing_gain: Ramp::new(0.0),
            volume: Ramp::new(1.0),
            reset_remaining: 0,
            clock: 0,
            step: 0,
            tick_pending: true,
            pluck: Transient::default(),
            accent: Transient::default(),
            bed_phase: [0.0; 2],
            bed_frequency: PENTATONIC[0] * 0.5,
        }
    }

    pub(crate) fn set_controls(&mut self, controls: Controls) {
        self.target = Controls {
            approach: unit(controls.approach),
            avoidance: unit(controls.avoidance),
            activity: unit(controls.activity),
            tone: controls.tone.min(4),
        };
    }

    pub(crate) fn set_playing(&mut self, playing: bool) {
        self.playing = playing;
    }

    pub(crate) fn set_volume(&mut self, volume: f32) {
        self.volume.set(unit(volume), self.frames(0.06));
    }

    /// Release stale sound for 30 ms, then clear the phrase and all voice state.
    /// Playing and volume remain caller-controlled. Controls supplied during the
    /// release become the new phrase's target, starting from a neutral baseline.
    pub(crate) fn reset(&mut self) {
        self.reset_remaining = self.frames(0.03);
        self.playing_gain.begin(0.0, self.reset_remaining);
    }

    pub(crate) fn next_frame(&mut self) -> [f32; 2] {
        let resetting = self.reset_remaining > 0;
        if !resetting {
            self.playing_gain.set(
                if self.playing { 1.0 } else { 0.0 },
                self.frames(if self.playing { 0.08 } else { 0.4 }),
            );
        }
        let gain = self.playing_gain.next();
        let volume = self.volume.next();
        if resetting {
            self.reset_remaining -= 1;
            if self.reset_remaining == 0 {
                self.clear_phrase();
                return [0.0; 2];
            }
        }
        if gain == 0.0 && !self.playing && !resetting {
            return [0.0; 2];
        }

        self.smoothed.approach += (self.target.approach - self.smoothed.approach) * self.smoothing;
        self.smoothed.avoidance +=
            (self.target.avoidance - self.smoothed.avoidance) * self.smoothing;
        self.smoothed.activity += (self.target.activity - self.smoothed.activity) * self.smoothing;

        if self.playing && !resetting && self.tick_pending {
            self.trigger_tick();
            self.tick_pending = false;
        }

        let pluck = self.pluck.next(self.sample_rate, false);
        let accent = self.accent.next(self.sample_rate, true);
        let bed = self.render_bed();
        if self.playing && !resetting {
            self.clock += 3;
            if self.clock >= self.sample_rate as u64 {
                self.clock -= self.sample_rate as u64;
                self.step = (self.step + 1) % MOTIF.len();
                self.tick_pending = true;
            }
        }

        if gain == 0.0 || volume == 0.0 {
            return [0.0; 2];
        }
        // One centered mono mix, duplicated for stereo device playback.
        // Average the previous voice gains to retain volume and headroom.
        let level = gain * volume;
        let mono =
            ((pluck * 0.75 + (bed[0] + bed[1]) * 0.5 + accent * 0.675) * level).clamp(-0.5, 0.5);
        [mono; 2]
    }

    fn frames(&self, seconds: f64) -> u32 {
        (self.sample_rate as f64 * seconds).round().max(1.0) as u32
    }

    fn trigger_tick(&mut self) {
        let approach = self.smoothed.approach;
        let activity = self.smoothed.activity;
        let note = (MOTIF[self.step] + self.target.tone as usize) % PENTATONIC.len();
        if self.step % 2 == 0 || (approach > 0.65 && matches!(self.step, 3 | 7)) {
            self.pluck.trigger(
                PENTATONIC[note],
                0.045 + 0.09 * approach,
                self.frames(0.3),
                self.frames(0.006),
            );
        }
        if (self.step == 2 && activity > 0.2) || (self.step == 7 && activity > 0.75) {
            self.accent.trigger(
                PENTATONIC[(note + 2) % PENTATONIC.len()] * 2.0,
                0.008 + 0.035 * activity,
                self.frames(0.075),
                self.frames(0.002),
            );
        }
    }

    fn render_bed(&mut self) -> [f32; 2] {
        let avoidance = self.smoothed.avoidance as f64;
        let target_frequency = PENTATONIC[self.target.tone as usize] * 0.5;
        self.bed_frequency += (target_frequency - self.bed_frequency) * self.smoothing as f64;
        let phase = self.bed_phase[0] * TAU;
        let detuned = self.bed_phase[1] * TAU;
        let main = phase.sin()
            + (0.08 + 0.3 * avoidance) * (phase * 3.0).sin()
            + (0.025 + 0.13 * avoidance) * (phase * 5.0).sin();
        let other = detuned.sin();
        advance_phase(
            &mut self.bed_phase[0],
            self.bed_frequency / self.sample_rate as f64,
        );
        advance_phase(
            &mut self.bed_phase[1],
            self.bed_frequency * (1.003 + 0.004 * avoidance) / self.sample_rate as f64,
        );
        let level = 0.013 + 0.023 * avoidance;
        [
            ((main * 0.7 + other * 0.3) * level) as f32,
            ((main * 0.55 + other * 0.45) * level) as f32,
        ]
    }

    fn clear_phrase(&mut self) {
        self.smoothed = Controls::default();
        self.playing_gain = Ramp::new(0.0);
        self.clock = 0;
        self.step = 0;
        self.tick_pending = true;
        self.pluck = Transient::default();
        self.accent = Transient::default();
        self.bed_phase = [0.0; 2];
        self.bed_frequency = PENTATONIC[0] * 0.5;
    }
}

fn unit(value: f32) -> f32 {
    if value.is_finite() {
        value.clamp(0.0, 1.0)
    } else {
        0.0
    }
}

fn advance_phase(phase: &mut f64, increment: f64) {
    *phase += increment;
    if *phase >= 1.0 {
        *phase -= 1.0;
    }
}

struct Ramp {
    value: f32,
    target: f32,
    increment: f32,
    remaining: u32,
}

impl Ramp {
    fn new(value: f32) -> Self {
        Self {
            value,
            target: value,
            increment: 0.0,
            remaining: 0,
        }
    }

    fn set(&mut self, target: f32, frames: u32) {
        if target != self.target {
            self.begin(target, frames);
        }
    }

    fn begin(&mut self, target: f32, frames: u32) {
        self.target = target;
        self.remaining = frames;
        self.increment = (target - self.value) / frames as f32;
    }

    fn next(&mut self) -> f32 {
        if self.remaining > 0 {
            self.remaining -= 1;
            self.value = if self.remaining == 0 {
                self.target
            } else {
                self.value + self.increment
            };
        }
        self.value
    }
}

#[derive(Default)]
struct Transient {
    phase: f64,
    frequency: f64,
    amplitude: f32,
    age: u32,
    duration: u32,
    attack: u32,
}

impl Transient {
    fn trigger(&mut self, frequency: f64, amplitude: f32, duration: u32, attack: u32) {
        *self = Self {
            phase: 0.0,
            frequency,
            amplitude,
            age: 0,
            duration,
            attack,
        };
    }

    fn next(&mut self, sample_rate: u32, accent: bool) -> f32 {
        if self.age >= self.duration {
            return 0.0;
        }
        let attack = (self.age as f32 / self.attack as f32).min(1.0);
        let onset = attack * attack * (3.0 - 2.0 * attack);
        let decay = 1.0 - self.age as f32 / self.duration as f32;
        let envelope = onset * decay * decay * decay;
        let phase = self.phase * TAU;
        let wave = if accent {
            // A brief, dry, slightly inharmonic tick, without broadband noise.
            phase.sin() * 0.8 + (phase * 1.49).sin() * 0.2
        } else {
            phase.sin() * 0.8
                + (phase * 2.0).sin() * 0.16 * decay as f64
                + (phase * 3.0).sin() * 0.04 * (decay * decay) as f64
        };
        self.age += 1;
        // The lifetime is bounded, so this can remain unwrapped. Wrapping at
        // the fundamental period would discontinuously restart the 1.49 partial.
        self.phase += self.frequency / sample_rate as f64;
        wave as f32 * envelope * self.amplitude
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RATE: u32 = 48_000;
    const ACTIVE: Controls = Controls {
        approach: 0.8,
        avoidance: 0.6,
        activity: 0.9,
        tone: 2,
    };

    fn start(synth: &mut Synth, controls: Controls) {
        synth.set_controls(controls);
        synth.set_volume(1.0);
        synth.set_playing(true);
    }

    fn render(synth: &mut Synth, frames: u32) -> Vec<[f32; 2]> {
        (0..frames).map(|_| synth.next_frame()).collect()
    }

    fn energy(frames: &[[f32; 2]]) -> f32 {
        frames
            .iter()
            .map(|frame| frame[0] * frame[0] + frame[1] * frame[1])
            .sum::<f32>()
            / frames.len() as f32
    }

    #[test]
    fn sound_requires_playing() {
        let mut synth = Synth::new(RATE);
        synth.set_controls(ACTIVE);
        assert!(
            render(&mut synth, RATE)
                .iter()
                .all(|frame| *frame == [0.0; 2])
        );
        start(&mut synth, ACTIVE);
        assert!(energy(&render(&mut synth, RATE)) > 0.00001);
    }

    #[test]
    fn extreme_and_malformed_controls_produce_finite_bounded_audio() {
        let mut synth = Synth::new(RATE);
        for controls in [
            Controls {
                approach: 1.0,
                avoidance: 1.0,
                activity: 1.0,
                tone: 4,
            },
            Controls {
                approach: f32::NAN,
                avoidance: f32::INFINITY,
                activity: -5.0,
                tone: u8::MAX,
            },
        ] {
            start(&mut synth, controls);
            for frame in render(&mut synth, RATE * 3) {
                assert!(
                    frame
                        .into_iter()
                        .all(|sample| sample.is_finite() && sample.abs() <= 0.5)
                );
            }
        }
        synth.set_volume(f32::NAN);
        let silence = render(&mut synth, RATE);
        assert!(
            silence[(RATE / 2) as usize..]
                .iter()
                .all(|frame| *frame == [0.0; 2])
        );
    }

    #[test]
    fn pause_and_mute_release_to_exact_silence() {
        let mut synth = Synth::new(RATE);
        start(&mut synth, ACTIVE);
        assert!(energy(&render(&mut synth, RATE)) > 0.00001);
        synth.set_playing(false);
        let release = render(&mut synth, RATE);
        assert!(
            energy(&release[..960]) > 0.0,
            "pause should release rather than cut sound"
        );
        assert!(release[24_000..].iter().all(|frame| *frame == [0.0; 2]));
        synth.set_playing(true);
        assert!(energy(&render(&mut synth, RATE)) > 0.00001);
        synth.set_volume(0.0);
        let release = render(&mut synth, RATE);
        assert!(
            energy(&release[..480]) > 0.0,
            "mute should ramp rather than cut sound"
        );
        assert!(release[4_800..].iter().all(|frame| *frame == [0.0; 2]));
    }

    #[test]
    fn reset_releases_old_sound_then_replays_deterministically() {
        let mut used = Synth::new(RATE);
        let mut fresh = Synth::new(RATE);
        start(&mut used, ACTIVE);
        assert!(energy(&render(&mut used, RATE)) > 0.00001);
        used.set_playing(false);
        render(&mut used, 480);
        used.reset();
        fresh.reset();
        let tail = render(&mut used, 1_440);
        assert!(
            energy(&tail) > 0.0,
            "reset should release the existing voices"
        );
        assert!(
            tail[1_400..]
                .iter()
                .flatten()
                .all(|sample| sample.abs() < 0.002),
            "reset during an existing pause release must fade before clearing voices"
        );
        render(&mut fresh, 1_440);
        start(&mut used, ACTIVE);
        start(&mut fresh, ACTIVE);
        assert_eq!(render(&mut used, RATE), render(&mut fresh, RATE));
    }

    #[test]
    fn approach_and_avoidance_change_the_rendered_music() {
        let mut yellow = Synth::new(RATE);
        let mut red = Synth::new(RATE);
        start(
            &mut yellow,
            Controls {
                approach: 1.0,
                activity: 0.4,
                ..Controls::default()
            },
        );
        start(
            &mut red,
            Controls {
                avoidance: 1.0,
                activity: 0.4,
                ..Controls::default()
            },
        );
        let yellow_frames = render(&mut yellow, RATE * 2);
        let red_frames = render(&mut red, RATE * 2);
        let difference = yellow_frames
            .iter()
            .zip(&red_frames)
            .map(|(yellow, red)| (yellow[0] - red[0]).powi(2) + (yellow[1] - red[1]).powi(2))
            .sum::<f32>()
            / yellow_frames.len() as f32;
        assert!(difference > 0.00001);
    }
}
