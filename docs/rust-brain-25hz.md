# Rust brain at 25 decisions per second

The production controller now runs the full MaleCNS graph in a separate optimized Rust worker using Candle 0.11.0 and a custom Metal CSR recurrence. Native play reached approximately 25 decisions per second on this M3 Pro. The Python model remains available for preparation and reference comparisons.

## Runtime contract

Each image-only decision advances two 20 ms neural ticks and four 10 ms physics ticks. The desktop counts time spent waiting toward the 40 ms decision deadline, applies physics only after validating the matching action, and carries ordinary fractional frame overshoot into the next decision. Backlog is bounded at 80 ms; sustained overload slows simulated time. Pause, reset and faults clear timing debt. One request may be outstanding.

Graph size, orientation `W[post, pre]`, float32 weights/activity, chromatic encoding, sampled receptor mapping, readout populations, fixed neutral calibration, and 20 ms neural dynamics are unchanged. The Metal kernel assigns one SIMD group to each postsynaptic row, uses no floating-point atomics, and disables fast math. The exact NumPy PCG64 stream and float64 normal sampling, followed by float32 conversion, preserve the reference noise sequence. No learning, graph pruning or reduced precision is introduced.

The observation cadence changes from 100 ms to 40 ms. Motor durations are represented on that new decision grid: retreat 20 decisions (0.8 s), search 88 (3.52 s), search turn 10 (0.4 s), and rearm 8 (0.32 s). This deliberately increases opportunities to observe and react. Numerical parity is checked against Python at the same new cadence; it does not assert identical trajectories to the former 100 ms controller.

The exported profile lives separately in `data/cache/malecns-rust-v1`. Its file hashes, source provenance and compiled runtime fingerprint are validated. Protocol version 2 requires the 2/4 contract and matching epoch, step, tick, revision, profile and RGB hashes. Exact duplicate requests return their cached reply without advancing the model. Inspection is pure. Checkpoints retain neural activity, prior input, connectivity state, complete RNG and motor state; incompatible Python/backend/profile checkpoints are rejected.

## Observed accuracy and performance

Measurements below were collected on 2026-09-13. The compact [machine-readable evidence](verification/rust-brain-25hz.json) records the runtime profile and local journal location.

| Check | Observed result |
|---|---|
| Python/Metal comparison | 115 decisions, including grayscale, yellow, red and actual eye-level rendered images |
| Maximum full activity error at four checked boundaries | 1.1920929×10⁻⁷ |
| Maximum steering error across 115 decisions | 1.5673352×10⁻⁸ |
| Motor mode agreement | Exact: approach, retreat, hold and search all exercised |
| RNG state agreement | Exact at every checked snapshot; separate golden tests cover 100,000 normal outputs and rejection/tail paths |
| Mid-retreat bookmark continuation | 25 replayed decisions matched exactly, including full final brain state |
| Disconnected retina | Identical yellow/red action traces from identical saved state |
| Graceful shutdown | Initial restore, shutdown acknowledgement and zero exit status verified in a separate production worker |
| Native accepted-action cadence | 25.0757/s across 499 actions; 19.8599 s between first and last acceptance |
| Native observation cadence | 24.9487/s across the same interval of decision IDs |
| Native step + inspection exchange | 9.32 ms median, 13.02 ms p95 |
| Native startup / subsequent maximum | 108.10 ms first decision; 20.56 ms maximum thereafter |

Native measurement used a separately identified app with rendering active and a drawn obstacle keeping the fly safely in First Flight. Every submitted observation had `physics_tick == 4 * step_id`. The original user's app remained untouched. Short live checks exercised solid and red-ink edits, explicit pause, F5 save, F9 world/brain restore to tick 1996, and Reset. Terminating only the verification worker deliberately produced “failed to fill whole buffer”; the world froze, and Reset relaunched the worker and successfully produced 47 further actions before the unbridged level ended.

These observations support realtime operation on this machine and numerical equivalence within the measured tolerance. They are not a universal error bound or a hard realtime guarantee. Near a decision threshold, small floating-point differences can change a later branch; longer runs and other GPUs remain unmeasured. The model itself is still an engineered leaky-rate approximation with artificial color mappings, not validated biological navigation.

The final packaged executables were checked again after the build-script changes. Opening the verification copy from Finder passed Enter, the recorded color intro, live solid drawing and pause. Across **1,260 decisions**, the final run measured **25.0008 accepted actions/s** (24.9694 observations/s), **9.19 ms median / 13.00 ms p95** worker exchange, and a 71.87 ms maximum. Its runtime profile matched the numerical test and earlier native run exactly. The package manifest in the JSON record includes both executable hashes.

## Reproduce and maintain

```sh
./scripts/setup.sh
./scripts/verify.sh --full
./scripts/package-macos.sh
```

`./scripts/verify.sh --full` passed after the clean rebuild: 54 Rust tests, 30 extracted reference tests, 16 Python unit tests, and both opt-in full-graph Rust/Metal and legacy Python transport checks. The final Rust comparison reproduced the same activity/steering errors and exact RNG/replay results. The full-graph Rust check writes `runs/rust-worker-test/verification.json`; native event journals contain monotonic `wall_ms` values for independent pacing calculations. Generated graph data, raw journals and app bundles remain ignored.

Packaging puts the optimized worker beside the desktop executable. Normal source launches explicitly select `target/release/fly-brain-worker`, so an incidental debug worker cannot silently become the play backend. Setup, launch and packaging build the workspace together to reuse the same dependency features as the workspace tests. Development builds omit debug symbols and incremental artifacts to keep the Bevy/Candle workspace within local disk capacity. The first full test build exhausted disk space; generated development artifacts were cleared before rebuilding with that smaller profile.

Joint bookmark restoration validates the replacement state before mutation. Disk publication of the world/brain pair retains an inherited limitation: the two files are written separately, so a failure between writes can invalidate the previous pair. This work does not claim crash-atomic joint persistence.
