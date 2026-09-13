# Neural music

The desktop renders a small original procedural score from accepted live neural
telemetry. Music starts with play; the opening tutorial's recorded brain clips do
not drive it. Use **Music** and **volume** in the floating toolbox. The default
volume is 35%. An unavailable output leaves the game playable and offers **Retry
music** with the device error in its tooltip.

The score has a fixed 90 BPM pulse and a five-note pitch collection. Approach
evidence brings forward warm plucks; avoidance adds harmonics and detuning to a
quiet tonal bed. Positive changes among the 336 sampled anatomical neurons shape
sparse accents and pitch selection. Small background changes are suppressed.
With stable neural evidence the score settles into a repeating pattern.

These are authored musical mappings of the model's nonnegative leaky rates.
They are not biological spikes, recorded brain sounds, emotions, or evidence that
the fly has learned music. Pitch groups are deliberately interleaved assignments,
not claims about anatomical function. The controller receives no audio state.

## Timing and lifecycle

- The existing 25 Hz Rust/Metal worker and two-neural/four-physics-step decision
  contract are unchanged. Music uses the telemetry already obtained with each
  accepted action; it requests no additional neural work.
- Musical time runs on the audio device's sample clock. Neural controls smooth
  between accepted snapshots, so ordinary worker waits do not interrupt rhythm.
- Explicit pause, loss of focus, faults, save/restore, and a level ending stop
  new notes and release sound over 400 ms. Muting also releases the score; volume
  changes ramp over 60 ms. The musical clock holds while paused.
- Reset, level selection, and bookmark restore clear pending voices and restart
  the phrase after a short 30 ms release. Bookmarks contain world and brain state;
  they do not capture the audio oscillator or phrase position.
- A one-second timeout on both accepted observations and desktop heartbeats
  releases sound if the worker or desktop stops updating.

## Implementation

`crates/desktop/src/music/signals.rs` validates the expected neural tick, monotonic
samples, finite nonnegative evidence, and the 336 rates in `[0,1]`. Approach and
avoidance use bounded `x / (x + 0.25)` compression. Per-neuron recent baselines
follow simulated neural time with a 1.2 s response. A fixed floor and relative
threshold suppress tiny fluctuations. The first accepted sample after a reset
establishes a baseline without a startup accent.

`music/mod.rs` owns the native output and a small latest-value mailbox. Both the
desktop publisher and audio consumer use `try_lock`; neither waits for the other.
There is no growing event queue. The source checks the mailbox every 128 stereo
frames. Stream failures are reported using an atomic flag.

`music/synth.rs` owns the three voices and musical clock. Sample rendering uses
fixed state, low sine partials, envelope ramps, and conservative gain. It does no
allocation, file access, neural work, or locking. The stereo mix is bounded to
`[-0.5, 0.5]` at maximum volume.

The output uses Rodio 0.22 with only its playback feature. Enabling the Bevy audio
feature failed dependency resolution because the configured registry lacked
`bevy_audio` 0.19.1; direct Rodio preserves the pinned Bevy 0.19.1 application.
No sample assets or external music service are required.

## Focused checks

```sh
cargo test -p fly-and-you --locked music::
cargo fmt --all -- --check
cargo test --workspace --locked
```

The music tests render the real synthesizer and cover finite output/headroom,
distinct controls, pause/mute/reset, invalid or repeated telemetry, and a stalled
publisher with a held mailbox lock. Native play remains necessary to judge feel,
device behavior, and interaction while the real neural worker runs.

## Verification on this machine

The workspace suite passed all 65 tests, including 11 music tests. Formatting,
whitespace, and the native workspace build passed. A worktree app check opened
audio output, displayed the controls, and exercised drawing, live play, and pause.
A continuous 2,065-action segment ran at 25.00 accepted decisions/s, with a median
worker latency of 8.95 ms. This is one local observation, not a performance guarantee.

An ignored `runs/neural-music-preview.wav` renders 17 seconds from existing
source-verified neural tutorial recordings through the production mapping and
synthesizer. This is an audition artifact; live music still uses accepted live
telemetry. Subjective listening quality and native volume/mute response have not
been verified by a listener. Automated rendered-sample checks cover those fades.

The nervous-system intro caption now remains for 15 seconds (12 seconds plus a
3-second hold), up from 10.4 seconds. Existing tutorial checks pass, and the
worktree app includes this adjustment; a separate timed native replay remains.
