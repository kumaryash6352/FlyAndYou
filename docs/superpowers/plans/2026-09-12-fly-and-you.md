# Fly & You implementation plan

**Goal:** A native drawing platformer with a literal little fly, a large game pane, neural activity above exact vision in a narrow right column, and an inspectable image-only controller.

**Architecture:** Engine-independent Rust world, Bevy/bevy-egui desktop, separately owned Python sparse neural model over framed localhost TCP. Bevy 0.19.1 and bevy-egui 0.42.0 implement the user-requested renderer; the world, sensory, timing and evidence contracts remain the guide's.

**Spec:** `zero_player_platformer_implementation.org`, including the agreed 80/20 layout and subsequent rough, low-resolution fly art direction.

## Constraints

- 640 x 360 world, 4-pixel collision grid, 128 x 96 eye-level-v1 perspective observations.
- Exactly five 20 ms neural steps followed by ten 10 ms world steps; one request outstanding.
- Solid and ink tools and erasers, guarded atomic edits, undo/redo, full reset, pause at boundaries.
- No coordinates, goal labels or planning inside the play request. No silent controller substitution.
- Learning off during puzzles. Real telemetry in neural panel; sprite/HUD absent from observations.
- Inline execution, meaningful behavioral tests and native visual verification.

## Tasks

- [x] **World and reference contracts.** Extract the four supplied source blocks. Add tests in `crates/world_core/tests/world.rs` for bridge traversal versus ink, swept collision, support deletion, atomic guard, undo guard, sensor bytes and replay; implement `terrain.rs`, `body.rs`, `sensory.rs`, `lib.rs`. Run `cargo test -p world_core`.
- [x] **Transport and coordinator.** Add strict framed messages in `wire_types`; test malformed/duplicate/stale replies. Implement `worker_client.rs` and `coordinator.rs` with bounded background I/O, epoch resets, aligned pauses and joint snapshots. Run crate tests and actual worker round trips.
- [x] **Neural backend.** In `controller/brainworker/`, implement verified graph import, explicit retina, sparse recurrence, fixed downstream readout, inspect and full restoration. Tests use a tiny graph with known propagation and disconnected-input controls. Import official MaleCNS tables and record hashes, selections and timings. The historical cutaway assay is retained but retired after the camera change. Validate the current perspective controller in native play.
- [x] **Native game.** Implement `crates/desktop/src/main.rs` and `display.rs`: rough original raster art, tiny black fly, top drawing tools, footer controls, upper-right activity and lower-right actual transmitted vision. Verify drawing, erasure, keyboard controls, resize, pause/reset, fault handling and readability in the running native app.
- [x] **Delivery.** Add launch and verification scripts, README with current readiness and controls, dependency locks and source attribution. Run focused contract checks, format/lint/build and native interaction checks; record actual limits. Leave a runnable app and a clear recovery path.

## User revision: eye-level vision

The editor and physics remain in the 2D XY plane. `vision.rs` extrudes occupied cells across a 64-unit corridor and casts a 110-degree horizontal perspective at 128 x 96 from the actor, facing its direction of travel. Exact player pixels texture walls and blocks; geometry occludes ink. Heading is checkpointed. The fixed readout now uses bilateral downstream activity to approach visible stimuli and turn to search when salience fades. The old global image-left/right interpretation and cutaway assay no longer apply.
