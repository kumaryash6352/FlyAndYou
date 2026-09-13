# Rust brain at 25 Hz implementation plan

Execute directly in the existing workspace. User approved the Candle migration and realtime scheduling after reviewing the full-graph accuracy/performance evaluation, then specified 25 decisions per second. One bounded helper handles exact NumPy RNG compatibility; integration stays inline.

**Goal:** Ship the actual full-graph Rust/Candle Metal worker and 25 Hz native decision loop.
**Architecture:** Separate Rust worker, resident sparse float32 CSR Metal recurrence, identical PCG64/normal stream, existing framed image-only request identities. A 40 ms decision contains two 20 ms neural steps and four 10 ms physics steps. The coordinator accrues bounded wall time while waiting but moves only after accepting the matching action.
**Spec:** User approval and `experiments/candle-evaluation/README.md`, refined to 25 Hz. This document records the concrete implementation contract.

## Constraints

- Same graph, RGB encoding, population mappings, fixed neutral baseline, dynamics, physics, native renderer and six levels.
- New distinct prepared Rust profile and protocol version 2; leave legacy Python profile usable as a reference and preserve the running user's process.
- Exact NumPy PCG64 and float64 normal-to-float32 noise from exported initial state; full RNG and motor fields checkpointed. Reject incompatible backend/profile/checkpoint identities.
- Motor count durations: retreat 20 decisions (0.8 s), search 88 (3.52 s), turn 10 (0.4 s), rearm 8 (0.32 s). The two latter rounded durations follow the 40 ms decision grid.
- One request outstanding; no physics until validated action; exactly four physics ticks per action, no skipped neural ticks. Carry normal fractional frame overshoot; cap backlog at 80 ms and slow under sustained overruns. Explicit pause/fault/reset clears debt at an aligned boundary.
- Run meaningful numerical, protocol/replay, clock, and native checks; don't build a broad test matrix. Keep the runtime separate from the experimental spike.

## Work and validation

- [x] `wire_types`: define version 2 and 2/4 step constants; strict request validation and legacy-count rejection tests.
- [x] `controller/brainworker/export_rust.py`: export hashed CSR buffers, mapping and NumPy seed state into `data/cache/malecns-rust-v1`, carrying official source/profile identity; no graph re-preparation or recalibration.
- [x] `brain_core`: validated profile loader, exact RNG golden fixtures, motor state parity/restore checks, reference CPU and Candle Metal compute, RGB-only observations, pure inspection and transactional snapshots.
- [x] `brain_worker`: strict serialized session with duplicate identity cache, atomic checkpoints, profile/epoch/tick enforcement, fault/shutdown; compare full graph against Python with internally generated matching noise and verify replay during retreat.
- [x] `coordinator.rs` / `app.rs`: failing deterministic tests for 25 decisions in one simulated wall second, no motion during waits, bounded overruns, pause/reset/stale/duplicate action behavior; implement carried wait debt and wall-time journal fields.
- [x] `worker_client.rs`, tutorial preparation/cache validation, scripts and packaging: use optimized Rust worker; preserve real neural tutorial responses and the matching provenance; bundle the worker beside the desktop binary.
- [x] Build and run workspace checks, full Rust socket/parity check, and a separate native app. Measure actual 25 Hz pacing with live rendering, paint, pause, reset, bookmark replay and worker fault recovery. Rebuild the bundle without terminating the user's existing app.
- [x] Update README/config/local briefing and this verification record; commit the implemented result and report only observed performance.

## Verification result

Completed: `./scripts/verify.sh --full` (54 Rust, 30 extracted reference, 16 Python unit tests, and both full-graph checks), shell syntax, formatting and diff checks, optimized app packaging, and separate native play/recovery checks. The final packaged native run sustained 25.0008 decisions/s across 1,260 decisions. Full-graph comparison retained maximum sampled activity error 1.19e-7 and steering error 1.57e-8, with exact RNG and retreat checkpoint replay. See `docs/rust-brain-25hz.md` and its JSON evidence. The original running app was preserved; reopen the rebuilt bundle to use the Rust worker.
