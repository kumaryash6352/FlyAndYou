# Chromatic movement implementation plan

Execute inline, as requested; this workspace has no Git repository.

**Goal:** Implement the approved red-repels/yellow-attracts neural controller and explain its decisions in the existing toolbox.

**Design:** [Approved investigation](/Users/tyush/Documents/Events/HackWesTX27/FlyAndYou/runs/movement-investigation/REPORT.md). Keep the sparse graph, eye-level-v2 RGB renderer, signed-steer action, five neural steps, ten physics ticks, and body constants. Use precomputed chromatic channel populations and a fixed neutral calibration; movement consumes postsynaptic activity only.

## 1. Neural readout and reproducible motor state

- [x] Add behavioral regressions to `controller/tests/test_brain.py`: identical R/B colors with different G must produce opposite actions; a red-triggered turn stays committed; restore during retreat reproduces subsequent actions, activity, and inspection. Run these against the old controller and observe failure.
- [x] Add `controller/brainworker/chromatic.py` for deterministic sensory partitioning, postsynaptic population selection, and RGB encoding. Add `motor.py` for profile parameters and approach/retreat/search/hold state. Keep search commitments distinct from red retreat commitments.
- [x] Integrate both into `model.py`; load and validate the new prepared profile. Store all motor fields in the brain checkpoint and validate the entire replacement before mutation. Preserve inspection purity and existing action/request identities.
- [x] Update `prepare.py` to persist channel masks/populations and a fixed neutral baseline. Rebuild locally from existing official data. Verify the old profile fails clearly and the new profile loads with zero model ticks.

## 2. Desktop feedback and current documentation

- [x] Extend `worker_client.rs` inspection telemetry and `display.rs` toolbox with the current movement mode and yellow/red neural evidence. Update palette hints and tutorial wording. Preserve real anatomical animation.
- [x] Update `README.md`, `README.org`, `config/desktop.toml`, and the opening refinement notes of the design reference. Update the local `AGENTS.md` briefing for the approved behavior; keep it ignored.

## 3. Verification and delivery

- [x] Run the focused controller regressions, normal verification script, and real full-graph socket/checkpoint test. The socket check must cover saving and restoring during red retreat.
- [x] Run a short production-controller check against the real world renderer/physics, including both travel directions and retinal disconnection.
- [x] Review the changes inline against the approved design. Build/package the native application and check drawing, red retreat, yellow approach, pause, and bookmark restoration. Do not interrupt an existing user play session.
- [x] Report the completed checks, app availability, and any remaining limitations. Do not claim broad usability from a short play check.


## Verification results

- `./scripts/verify.sh --full` passed: 24 Rust tests, 30 reference tests, 16 controller tests, and the opt-in full-graph socket test. The socket test now restores during retreat and compares all subsequent actions and inspection telemetry exactly.
- The focused Python suite passed again after import formatting; final `cargo fmt --all -- --check` passed. `./scripts/package-macos.sh` passed after the native font correction. Packaged and built binaries have matching SHA-256 `14cce8c25970961e73b2d0f067b8cf9c7301067e856b1e2c22c13b6956e6ae27`.
- Production controller with the real Rust renderer/physics: over 1.2 simulated seconds, a red wall caused -147.75 px displacement when initially facing right, and +147.75 px facing left. Yellow support caused +141.19 px and -141.06 px respectively. Retinal disconnection removed color-dependent actions. See `runs/movement-investigation/check_production.py` and `production-motion.json`.
- Native check in `runs/play-150e1c93c975`: built and painted a wall, observed physical retreat, reset and painted yellow support, observed approach, recolored red and observed retreat. The first red-wall actions were two search decisions followed by eight retreat decisions. F5/F9 restored position, image, neural evidence, and mode. The saved retreat checkpoint contains heading -1, mode retreat, remaining 3, steer -0.85, and neural tick 50. Pause held the state.
- Native font inspection found missing arrow glyphs; the final build uses plain Left/Right/Still text. The corrected build was reopened and left ready, paused at the initial world.
- Model weights and dataset provenance hashes stayed unchanged; only prepared channel mapping and controller profile changed. Older brain bookmarks are incompatible with this profile.

The app bundle was rebuilt. Launch Services startup stalled on macOS Documents-folder access (confirmed by the TCC log and blocked file-open samples). Directly launching the same packaged executable with `FLY_AND_YOU_ROOT="$PWD" ./FlyAndYou.app/Contents/MacOS/fly-and-you` loaded successfully and was used for the native checks. No privacy settings were changed. This launch-path limitation is separate from the verified controller behavior. Logs are retained under `runs/movement-investigation/`.

These are focused correctness and native interaction checks, not a broad usability study or biological validation.
