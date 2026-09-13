//! Desktop-only sonification. The model never waits for or receives audio state.
mod signals;
mod synth;

use crate::worker_client::Telemetry;
use rodio::{ChannelCount, DeviceSinkBuilder, MixerDeviceSink, SampleRate, Source};
use signals::Signals;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};
use synth::{Controls, Synth};
use wire_types::{NEURAL_STEPS, Reply};

const STALE_AFTER: Duration = Duration::from_secs(1);

#[derive(Clone, Copy, Default)]
struct Snapshot {
    controls: Controls,
    playing: bool,
    volume: f32,
    generation: u64,
    heartbeat: u64,
}

type Shared = Arc<Mutex<Snapshot>>;

pub(crate) struct Music {
    pub enabled: bool,
    pub volume: f32,
    signals: Signals,
    snapshot: Snapshot,
    shared: Shared,
    last_observation: Option<Instant>,
    output: Option<MixerDeviceSink>,
    output_failed: Arc<AtomicBool>,
    output_error: Option<String>,
}

impl Music {
    pub fn new() -> Self {
        let mut music = Self {
            enabled: true,
            volume: 0.35,
            signals: Signals::default(),
            snapshot: Snapshot::default(),
            shared: Arc::new(Mutex::new(Snapshot::default())),
            last_observation: None,
            output: None,
            output_failed: Arc::new(AtomicBool::new(false)),
            output_error: None,
        };
        music.open_output();
        music
    }

    pub fn open_output(&mut self) {
        self.output = None;
        self.output_failed.store(false, Ordering::Relaxed);
        let failed = Arc::clone(&self.output_failed);
        let result = DeviceSinkBuilder::from_default_device().and_then(|builder| {
            builder
                // Send the mono mix to both speakers, even if the device's
                // preferred format happens to expose a single channel.
                .with_channels(ChannelCount::new(2).unwrap())
                .with_buffer_size(rodio::cpal::BufferSize::Fixed(512))
                .with_error_callback(move |_| failed.store(true, Ordering::Relaxed))
                .open_sink_or_fallback()
        });
        match result {
            Ok(mut output) => {
                output.log_on_drop(false);
                let rate = output.config().sample_rate();
                output
                    .mixer()
                    .add(Decoder::new(Arc::clone(&self.shared), rate));
                self.output = Some(output);
                self.output_error = None;
            }
            Err(error) => self.output_error = Some(format!("Audio unavailable: {error}")),
        }
    }

    pub fn error(&self) -> Option<&str> {
        if self.output_failed.load(Ordering::Relaxed) {
            Some("Audio output stopped. Retry music to reconnect.")
        } else {
            self.output_error.as_deref()
        }
    }

    /// Call only after Coordinator::accept has validated the action's identity.
    pub fn accept(&mut self, reply: &Reply, telemetry: &Telemetry) {
        let expected = reply
            .step_id
            .checked_add(1)
            .and_then(|step| step.checked_mul(u64::from(NEURAL_STEPS)));
        let controls = expected.and_then(|tick| {
            self.signals.observe(
                telemetry.ticks,
                tick,
                telemetry.approach,
                telemetry.avoidance,
                &telemetry.anatomy_activity,
            )
        });
        if let Some(controls) = controls {
            self.snapshot.controls = controls;
            self.last_observation = Some(Instant::now());
        } else {
            self.last_observation = None;
        }
    }

    pub fn reset(&mut self) {
        self.signals = Signals::default();
        self.last_observation = None;
        self.snapshot.controls = Controls::default();
        self.snapshot.generation = self.snapshot.generation.wrapping_add(1);
        self.update(false);
    }

    pub fn update(&mut self, running: bool) {
        self.snapshot.playing = running
            && self.enabled
            && self.error().is_none()
            && self
                .last_observation
                .is_some_and(|time| time.elapsed() < STALE_AFTER);
        self.snapshot.volume = self.volume;
        self.snapshot.heartbeat = self.snapshot.heartbeat.wrapping_add(1);
        // Both sides use try_lock; neither simulation nor audio can wait here.
        // This is a latest-value mailbox, so missed copies cannot build a queue.
        if let Ok(mut shared) = self.shared.try_lock() {
            *shared = self.snapshot;
        }
    }
}

struct Decoder {
    shared: Shared,
    synth: Synth,
    sample_rate: SampleRate,
    snapshot: Snapshot,
    stale_frames: u32,
    poll_in: u8,
    right: Option<f32>,
}

impl Decoder {
    fn new(shared: Shared, sample_rate: SampleRate) -> Self {
        Self {
            shared,
            synth: Synth::new(sample_rate.get()),
            sample_rate,
            snapshot: Snapshot::default(),
            stale_frames: 0,
            poll_in: 0,
            right: None,
        }
    }
}

impl Iterator for Decoder {
    type Item = f32;
    fn next(&mut self) -> Option<f32> {
        if let Some(right) = self.right.take() {
            return Some(right);
        }
        if self.poll_in == 0 {
            // Copy once per short block. A busy/poisoned mailbox leaves the
            // previous snapshot in place; the independent watchdog still runs.
            if let Ok(shared) = self.shared.try_lock() {
                if shared.heartbeat != self.snapshot.heartbeat {
                    self.stale_frames = 0;
                }
                if shared.generation != self.snapshot.generation {
                    self.synth.reset();
                }
                self.snapshot = *shared;
            }
            self.synth.set_controls(self.snapshot.controls);
            self.synth.set_volume(self.snapshot.volume);
            self.synth
                .set_playing(self.snapshot.playing && self.stale_frames < self.sample_rate.get());
            self.poll_in = 128;
        }
        self.poll_in -= 1;
        self.stale_frames = self.stale_frames.saturating_add(1);
        let [left, right] = self.synth.next_frame();
        self.right = Some(right);
        Some(left)
    }
}

impl Source for Decoder {
    fn current_span_len(&self) -> Option<usize> {
        None
    }
    fn channels(&self) -> ChannelCount {
        ChannelCount::new(2).unwrap()
    }
    fn sample_rate(&self) -> SampleRate {
        self.sample_rate
    }
    fn total_duration(&self) -> Option<Duration> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn playing() -> Shared {
        Arc::new(Mutex::new(Snapshot {
            controls: Controls {
                approach: 0.8,
                avoidance: 0.3,
                activity: 0.5,
                tone: 2,
            },
            playing: true,
            volume: 0.7,
            heartbeat: 1,
            generation: 0,
        }))
    }

    #[test]
    fn mono_music_reaches_both_stereo_channels_identically() {
        for output_rate in [44_100, 48_000] {
            let shared = playing();
            let decoder = Decoder::new(shared, SampleRate::new(48_000).unwrap());
            let (mixer, mut output) = rodio::mixer::mixer(
                ChannelCount::new(2).unwrap(),
                SampleRate::new(output_rate).unwrap(),
            );
            mixer.add(decoder);
            let mut energy = 0.;
            for _ in 0..output_rate / 2 {
                let left = output.next().unwrap();
                let right = output.next().unwrap();
                assert_eq!(
                    left, right,
                    "Mono music must be centered after device conversion"
                );
                energy += left * left;
            }
            assert!(
                energy > 0.01,
                "Both channels must carry real music, not silence"
            );
        }
    }

    #[test]
    fn omitted_color_evidence_is_unavailable_instead_of_neutral() {
        for omitted in ["approach", "avoidance"] {
            let mut value = serde_json::json!({
                "ticks": 2, "approach": 0.5, "avoidance": 0.0,
                "anatomy_activity": vec![0.0002; signals::ANATOMY_COUNT],
            });
            value.as_object_mut().unwrap().remove(omitted);
            let telemetry: Telemetry = serde_json::from_value(value).unwrap();
            assert!(
                Signals::default()
                    .observe(
                        telemetry.ticks,
                        2,
                        telemetry.approach,
                        telemetry.avoidance,
                        &telemetry.anatomy_activity,
                    )
                    .is_none()
            );
        }
    }

    #[test]
    fn desktop_stall_releases_audio_even_if_mailbox_is_locked() {
        let shared = playing();
        let mut decoder = Decoder::new(Arc::clone(&shared), SampleRate::new(48_000).unwrap());
        let energy: f32 = decoder.by_ref().take(48_000).map(f32::abs).sum();
        assert!(
            energy > 1.,
            "A valid neural sample must produce audible output"
        );
        let _held = shared.lock().unwrap();
        let tail: Vec<f32> = decoder.by_ref().take(192_000).collect();
        assert!(tail.iter().all(|v| v.is_finite() && v.abs() <= 1.));
        assert!(tail[tail.len() - 48_000..].iter().all(|v| *v == 0.));
    }

    #[test]
    fn reset_generation_clears_a_previous_phrase_before_resume() {
        let shared = playing();
        let rate = SampleRate::new(48_000).unwrap();
        let mut decoder = Decoder::new(Arc::clone(&shared), rate);
        for _ in 0..32_000 {
            decoder.next();
        }
        {
            let mut state = shared.lock().unwrap();
            state.generation += 1;
            state.heartbeat += 1;
            state.playing = false;
        }
        let release: Vec<_> = decoder.by_ref().take(48_000).collect();
        assert!(release[24_000..].iter().all(|v| *v == 0.));
        {
            let mut state = shared.lock().unwrap();
            state.heartbeat += 1;
            state.playing = true;
        }
        assert!(decoder.take(48_000).map(f32::abs).sum::<f32>() > 1.);
    }
}
