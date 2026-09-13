# Rust / Candle brain feasibility — 13 September 2026

**Recommendation: proceed with a Rust worker using Candle 0.11.0 and a custom float32 CSR Metal kernel, then separately revise wall-clock scheduling.** The full-graph kernel is fast enough to make a 10 Hz controller plausible on this M3 Pro. A completed realtime game implementation has not been demonstrated.

The retained code is a **throwaway feasibility probe**, not a worker or production dependency. It implements recurrent compute only. The game, prepared profile, app bundle, scheduling, and running native process were left unchanged. The source checkpoint before evaluation is `06f126e`. [Machine-readable measurements and source hashes](summary.json) accompany the probe.

## Measurements on the full graph

Apple M3 Pro, 5 performance plus 6 efficiency cores, 18 GiB RAM, macOS 26.0.1. Python 3.12.11, NumPy 2.5.3, SciPy 1.18.1; optimized Rust build, Candle pinned to 0.11.0. Benchmarks ran sequentially; the existing game and its worker remained open. These short runs do not isolate system load, thermal effects, or competition with the game's GPU rendering.

Each compute sample performs **five sequential recurrent steps over all 164,606 neurons and 25,558,671 connections**. No pruning, half precision, topology changes, learned approximation, or reduced neural step count was used.

| Compute backend | Median per decision | p95 | Maximum |
| --- | ---: | ---: | ---: |
| SciPy, reference recurrence | 111.59 ms | 123.45 ms | 130.38 ms |
| Rust CPU, one thread | 138.77 ms | 181.78 ms | 235.86 ms |
| Rust CPU, five Rayon threads | 43.32 ms | 47.12 ms | 66.66 ms |
| Candle / Metal, one GPU thread per row | 24.48 ms | 26.53 ms | 29.98 ms |
| Candle / Metal, one SIMD group per row, first run | 11.13 ms | 20.76 ms | 22.83 ms |
| Same SIMD kernel, separate repeat process | 17.82 ms | 46.56 ms | 64.18 ms |

All rows use identical precomputed float32 noise and input currents. There are 60 measured decisions per row, following warmup. The 60-decision trajectory cycles ten neutral, ten yellow, and ten red full-image observations twice, beginning from the same model state after eight neutral decisions. Metal timings include per-decision host input/noise uploads, five ordered custom-op calls, allocation overhead, and a completed readback of every neuron's activity. They exclude fixture loading, random-number generation, RGB encoding, motor readout, and transport. CPU timings reuse their output buffers. This compares straightforward implementations, not an exhaustive tuning study.

The SIMD kernel is about **6.3–10.0 times faster** than the corresponding SciPy recurrence at the observed medians. The repeat's slower tail is retained rather than discarded. Neither this ratio nor the 64 ms observed maximum is a latency guarantee. The five-thread CPU implementation is a credible fallback to investigate, although it still competes for CPU resources with the game.

The unmodified production `Brain.step_image` measured **115.12 ms median / 128.68 ms p95** over 60 decisions. A separate real Python worker, using the existing framed TCP protocol and both `step` and `inspect`, measured **118.22 ms median / 123.37 ms p95** over 40 requests after five warmups. Within those requests, the paired non-compute overhead was **1.40 ms median**. Removing TCP is therefore a secondary optimization.

A separate 100-tick breakdown found:

| Operation per neural tick | Median |
| --- | ---: |
| SciPy CSR matrix-vector multiply | 21.872 ms |
| Scale recurrence and add sensory input | 0.049 ms |
| Generate and add NumPy noise | 0.774 ms |
| Leak, rectification and tanh | 0.230 ms |

Sparse multiplication accounts for approximately 95% of these component medians. Rust syntax alone does not remove that cost: SciPy already executes its sparse operation in compiled code. The actual gains come from parallel sparse execution and fused updates.

## Why a custom Candle operation

The verified CSR arrays occupy **205,127,796 bytes** (195.62 MiB). A dense float32 matrix would occupy **108,380,540,944 bytes** (100.94 GiB), beyond this machine's memory. Preserve `W[post, pre]`, float32 values, u32 column indices, and row pointers.

Candle 0.11.0 has no ready-made CSR/COO matrix-vector primitive in its inspected [backend interface](https://github.com/huggingface/candle/blob/0.11.0/candle-core/src/backend.rs). A tensor expression that gathers presynaptic activity, multiplies every edge, and reduces via `index_add` is a poor substitute here: the current Metal rank-one accumulation dispatch has only one useful worker, and that worker loops over the entire index array. This is a source finding, not a measured gather/scatter result. See the [dispatch](https://github.com/huggingface/candle/blob/0.11.0/candle-metal-kernels/src/kernels/indexing.rs#L167-L172) and [accumulation kernel](https://github.com/huggingface/candle/blob/0.11.0/candle-metal-kernels/src/metal_src/indexing.metal#L234-L241).

The probe successfully uses Candle's [user-defined `CustomOp1::metal_fwd`](https://github.com/huggingface/candle/blob/0.11.0/candle-core/src/custom_op.rs#L6-L33). CSR weights remain resident. One SIMD group of 32 GPU threads owns each postsynaptic row, reduces its incoming edges, and writes exactly one next-state value. Each dispatch fuses:

```text
sum = sum_j W[row, j] * activity[j]
drive = 0.9 * sum + sensory_current[row] + noise[row]
next[row] = 0.7 * activity[row] + 0.3 * tanh(max(drive, 0))
```

Five dispatches retain the dependency between neural ticks. The implementation registers input and output buffers through Candle's encoder API so recurrence observes the required Metal barriers. It uses no global floating-point atomics. Custom shader fast math is disabled. Production could reuse two activity buffers and read back only motor populations and telemetry, but those optimizations are unmeasured.

Candle supplies device, tensor, buffer, synchronization and custom-op integration; the specialized sparse kernel supplies the acceleration. Direct Metal could execute the same algorithm with fewer dependencies, but adds lower-level resource management. Candle is a reasonable choice if it is the preferred Rust compute layer. Its library alone is not an automatic sparse acceleration solution.

## What the equivalence checks establish

At the 60 recorded decision boundaries after 300 recurrent ticks:

- Maximum activity difference from the SciPy reference: **2.3842e-7** for Rust CPU, **1.7881e-7** for both Metal kernels.
- Maximum Metal normalized evidence difference: **4.7684e-7**; maximum steering difference: **1.4174e-8**.
- The unchanged Python motor decoder produced identical modes and all discrete/state fields other than the tiny steer difference. The sequence exercised **search, approach, retreat, and hold**.
- A second process running the SIMD kernel from the identical initial arrays and noise reproduced all recorded activity values **bit for bit**.

These are fixed-fixture arithmetic and motor checks. They do not prove arbitrary threshold equivalence, long-run numerical stability, cross-device identity, native gameplay feel, or Rust worker snapshot/restore. All compared outputs were finite and within [0, 1]. The probe uses a 1e-5 maximum absolute activity/steer guard, while preserving the measured errors in the result file.

RNG and checkpoints are the largest remaining correctness boundary. The current brain stores NumPy's complete PCG64 generator state, including the implementation's normal sampling behavior. Candle Metal uses a different random algorithm; its [random kernel](https://github.com/huggingface/candle/blob/0.11.0/candle-metal-kernels/src/metal_src/random.metal) and [seed accessors](https://github.com/huggingface/candle/blob/0.11.0/candle-core/src/metal_backend/mod.rs#L2351-L2365) cannot be treated as a compatible NumPy checkpoint. This probe deliberately supplies identical reference noise and does not implement a new RNG. A production port must either reproduce the NumPy stream or define a new versioned, fully checkpointed RNG/backend contract and reject incompatible old checkpoints. Approximate numerical agreement does not justify silently sharing checkpoint identities.

## Realtime also requires a scheduling decision

The controller produces five 20 ms neural updates per observation, i.e. **10 decisions per simulated second**. Realtime here means roughly one second of world simulation per wall-clock second, with responsive rendering. It does not mean a neural decision on every 60 Hz display frame or a hard realtime guarantee.

`crates/desktop/src/coordinator.rs` currently holds physics during `Waiting`, clears its accumulator when accepting an action, then spends another 100 ms applying ten 10 ms physics ticks. `app.rs` explicitly supplies zero elapsed time on the acceptance frame. Ignoring rendering and frame-boundary overhead:

```text
current wall time per 100 ms simulated block ≈ worker latency + 100 ms
simulation / wall speed ≈ 100 / (100 + worker latency in ms)
```

The measured 118 ms Python exchange therefore implies about **0.46× realtime** before other overhead. Even a hypothetical complete 15–25 ms replacement worker would imply only **0.80–0.87× realtime** under the unchanged coordinator. Those replacement-worker figures are illustrative estimates, not measured end-to-end Rust worker latency. Five ticks of the measured current NumPy RNG alone cost around 3.9 ms; kernel timings cannot be advertised as a complete worker.

Recommended scheduling direction for a later approved change: use a wall-clock deadline for each 100 ms decision block. Capture the observation at its existing boundary, retain one outstanding request, and wait for a validated action before advancing any physics. Once accepted, account for elapsed time within that block and catch up the bounded number of due 10 ms ticks, then finish at the original block deadline. Preserve all ten physics steps and all five neural steps. Pause, reset, fault, edit ordering, and stale-action rejection remain explicit boundaries. An overrun must slow/freeze according to a declared rule rather than skip model steps or reuse an old action silently.

This changes the current scheduling contract and needs discussion before implementation. No speculative action, hidden waypoint, concurrent brain request, or altered neural timestep is needed to investigate it. Native frame pacing, eye rendering, and editor responsiveness must be measured after integration because the GPU will also serve the game's rendering.

## Proposed production scope, for discussion

1. **Optional Rust worker first.** Add a `brain_core` crate for profile loading, RGB channel encoding, motor state, telemetry, explicit RNG and snapshots, and CSR CPU/Candle Metal implementations. Add a separate `brain_worker` executable using `wire_types` and the current `loaded`, `step`, `inspect`, `snapshot`, `restore`, and `shutdown` messages. Keep Python preparation and the existing worker available during comparison. Preserve epoch, step, tick, revision, image hash, profile validation, exactly-once actions, inspection purity and freeze-on-fault behavior. Verify metadata/digests and atlas body-ID mappings before use. Backend selection/fallback should be explicit, and initial integration should preserve the current coordinator.
2. **Port evidence before switching the app.** Replay actual saved eye frames from matching initial state; compare motor transitions and neural outputs, test disconnected-input controls, and exercise restore during retreat plus rejected stale/duplicate actions over a real Rust socket. Record backend/RNG identity in the profile/checkpoint. Keep the engineered color rules and fixed neutral calibration consistent. Update `worker_client.rs`, setup/run/packaging scripts, and tutorial demo provenance (`controller/brainworker/tutorial.py`, `desktop/src/tutorial_demo.rs`) when selecting the new runtime. Recalibration, if needed, must be measured and versioned rather than quietly changing behavior.
3. **Deadline scheduling and native check second.** Change `coordinator.rs` / `app.rs` only after agreeing on wait debt, overrun, pause and recovery behavior. Measure full observation-to-action latency, simulation/wall ratio and frame pacing in the native app while drawing and showing brain telemetry. A useful proposed target is sustained 0.95–1.05 simulation/wall ratio, p95 complete decisions below 50 ms, and no unhandled 100 ms deadline misses in the observed play check. This is an acceptance target, not a result already achieved.

Do not merge the probe as that implementation. It trusts exported fixture shapes, hardcodes approved dynamics for measurement, omits protocol/profile/checkpoint/RNG loading, and reads every neuron back for comparison. A production implementation needs those boundaries before it can replace Python safely.

## Reproduce the evaluation

Run from the project root with the existing prepared graph and `.venv`. Xcode's Metal tooling and the normal Rust toolchain are required. The standalone probe has its own workspace and lockfile; it does not add Candle to the game workspace. First compilation may download its locked Cargo dependencies. Generated fixtures use roughly hundreds of MB under ignored `runs/candle-evaluation`; build products stay ignored too.

```sh
PYTHONPATH=. .venv/bin/python experiments/candle-evaluation/baseline.py
cargo build --release --locked --manifest-path experiments/candle-evaluation/spike/Cargo.toml
experiments/candle-evaluation/spike/target/release/fly-brain-feasibility-spike runs/candle-evaluation cpu1
experiments/candle-evaluation/spike/target/release/fly-brain-feasibility-spike runs/candle-evaluation cpu5
experiments/candle-evaluation/spike/target/release/fly-brain-feasibility-spike runs/candle-evaluation metal-serial
experiments/candle-evaluation/spike/target/release/fly-brain-feasibility-spike runs/candle-evaluation metal-simd
PYTHONPATH=. .venv/bin/python experiments/candle-evaluation/verify_parity.py
PYTHONPATH=. .venv/bin/python experiments/candle-evaluation/socket_baseline.py
```

For the exact replay check, preserve the first `metal-simd-states.f32` SHA-256, repeat the same executable command in a new process, and require the new file's SHA-256 to match. Do not run the CPU/socket and GPU timing commands concurrently. The committed `summary.json` records both measured GPU timing runs and the successful equality check.

Completed validation: optimized standalone Candle build, full-graph recurrence comparison for all four Rust backends, production-motor parity, exact same-backend fixture replay, and the existing real Python worker's step/inspect/shutdown exchange. No new production controller was installed and no native Rust-backend play check was performed.
