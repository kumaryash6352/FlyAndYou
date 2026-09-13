# Neural music implementation plan

**Goal:** Add a restrained three-voice procedural score influenced by the fly's accepted live neural activity.

**Approved design:** The preceding evaluation and the user's “Implement in a worktree” instruction. Warm plucks follow approach evidence, a buzzing tonal layer follows avoidance evidence, and sparse accents follow changes in sampled anatomical activity. A fixed musical clock keeps worker timing out of the rhythm.

**Architecture:** Rust desktop code reduces existing telemetry to bounded musical controls. A persistent Rodio custom audio source renders a small synthesizer from a nonblocking snapshot handoff. World, worker, protocol, and 25 Hz decision scheduling remain unchanged.

**Tech stack:** Bevy 0.19.1, Rodio 0.22 playback, standard-library synchronization, existing egui toolbox. No external audio assets or neural compute dependencies.

## Boundaries

- Worktree: `.worktrees/neural-music`, branch `codex/neural-music`.
- Only accepted live action telemetry changes musical evidence; tutorial recordings do not.
- Pause, fault, end, reset, and restore stop new notes and release sound smoothly.
- Restore restarts the musical phrase; audio phase is not part of world/brain bookmarks.
- Keep controls compact, default volume modest, output finite and bounded, and missing telemetry unavailable.
- Reuse prepared graph data without changing it. Do not interrupt an existing game process.

## 1. Synthesizer

Files: `crates/desktop/src/music/synth.rs`.

- [x] Implement `Controls { approach, avoidance, activity, tone }` and `Synth::new(sample_rate)`, `set_controls`, `set_playing`, `set_volume`, `reset`, `next_frame`.
- [x] Use a 90 BPM pulse, pentatonic notes, three restrained voices, smoothed controls, deterministic variation, and allocation-free sample rendering.
- [x] Check finite/headroom behavior, audible response to different controls, pause/mute release, and deterministic reset with focused rendered-sample tests.

## 2. Live signal mapping and audio source

Files: `crates/desktop/src/music/mod.rs`, `signals.rs`, desktop `Cargo.toml`, `Cargo.lock`, `main.rs`, `app.rs`.

- [x] Test rejection of duplicate/older identities and invalid telemetry, bounded evidence compression, quiet stable rates, and activity changes.
- [x] Track per-neuron recent baselines only on accepted ticks. Compress nonnegative approach/avoidance evidence and noise-gate positive anatomical changes.
- [x] Open a native Rodio output and attach one custom source (the registry lacks bevy_audio 0.19.1). Its decoder uses `try_lock` to copy a small latest-state snapshot, never blocks or allocates during sample generation, and fades when updates stop.
- [x] Publish after successful action acceptance. Check expected neural ticks against accepted physics ticks. Reset mapping and source generation on restored epochs.
- [x] Gate playback from current simulation, tutorial, focus, save/restore, and user settings; continue normal worker waits smoothly.

## 3. Toolbox, documentation, and verification

Files: `crates/desktop/src/display.rs`, `README.md`, `config/desktop.toml`, `docs/neural-music.md`.

- [x] Add Music on/off and volume in one compact toolbox row with a short explanatory tooltip.
- [x] Document engineered mappings, pause/restore behavior, and audio defaults.
- [x] Run `cargo fmt --all -- --check`, focused music tests, `cargo test --workspace --locked`, and a native startup/play/pause/control check using the worktree build.
- [x] Review changes for lifecycle mistakes and audio callback work. Record completed checks and remaining listening/performance limits; leave the implementation in its worktree.

## Requested intro adjustment

Lengthen the nervous-system dialogue from 10.4 s to 15 s across beats 9–10 (12 s reading/reveal plus 3 s hold). Update `tutorial.rs` and the README; keep the existing rail reveal onset. Existing tutorial checks pass and the native app is rebuilt. The new 15 s hold has not had a separate timed native replay.

## Verification result

- `cargo fmt --all -- --check`, `git diff --check`, and `cargo build --workspace --locked` passed.
- `cargo test --workspace --locked`: 65 tests passed, including 11 music tests and the existing tutorial checks, after the intro timing change.
- Native worktree app opened its audio output and displayed Music/volume controls. Drew a bridge, ran the real Metal worker, and paused. A continuous 2,065-action block sustained 25.00 accepted decisions/s (median worker latency 8.95 ms).
- Rendered a 17 s WAV using the production synth/mapping and existing recorded neural color responses. Listening quality and native mute/volume response remain subjective checks; the rendered-sample tests cover release and volume behavior.
- Focused review caught absent color evidence becoming zero; optional fields and a regression test now preserve unavailable input.
- Updated only the worktree app bundle. The native QA instance was closed; the other running game was left alone.
