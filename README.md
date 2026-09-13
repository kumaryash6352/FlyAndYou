# fly & you

You make the world. The fly makes up its mind.

A native **Bevy 0.19.1 + bevy-egui 0.42.0** drawing platformer: a tiny black fly, rough pixel scenery, an edge-to-edge 2D game view, scan-like neural anatomy above its eye view, and a compact floating toolbox that also teaches the game.

## Play

Open **FlyAndYou.app**. The game executable embeds the prepared neural graph, calibration, anatomical atlas, tutorial recordings, and provenance. Its Rust/Candle Metal controller runs on a background thread inside the Bevy process, using bounded channels. The app can move away from this checkout; Python and external model files are needed only for preparation and reference checks.

From a terminal:

```sh
./scripts/run.sh
```

For the automatic, roughly one-minute trailer, run `./scripts/trailer.sh` and press **Enter** when your screen recording is ready. The separate `fly-trailer` binary draws the bridge and ink through the real brushes while the full neural controller moves Fly. **Space** pauses the take; **R** resets it. [Trailer recording and rehearsal](docs/trailer.md).

After loading, **Press Enter to Start** holds until you are ready. First Flight then opens with a handwritten animated introduction. Terrain stays visible from the opening close-up through the pan and zoom; Fly and the Goal appear in turn. The brain scan and **FlyCam™** appear on the “simulated nervous system” line, which stays on screen for 15 seconds to leave time to read and inspect both views. Fly waits through the narration, then begins moving when you first paint. **Skip intro** goes directly to that waiting beat; **Replay intro** under Controls resets the world and brain to show it again. The intro pauses when the window loses focus.

During the red and yellow lines, a temporary painted surface appears ahead of Fly. FlyCam shows that surface and the scan replays actual responses recorded from this model with a fixed observer. Each recording starts from the same initial state; neither demonstration changes the player's world or brain. Setup, launch, and packaging scripts refresh the recordings when their model or images change.

Scribble **Ground** across the gap before Fly falls. Click a color to select **Ink**: **yellow attracts; red repels**. Click that swatch again, or **×**, to clear the selection. **Ground** uses the selected color; without one it stays neutral. Ink colors terrain and objects, including the flag. Painting empty space colors both side walls of the 3D corridor and provides no support. The fly keeps moving while you draw. Your scribbles commit in small pieces at decision boundaries; their rough edges and grain are part of the actual world. The tutorial leaves the camera on the whole map; **Follow** in the toolbox follows the fly instead. **Space** pauses or resumes after the first painting gesture. There is no gameplay time limit; physical hazards still cause failure.

The fly sees ahead from eye level in a **3D perspective projection**. Its world starts entirely grayscale, even though the editor retains its rough art colors. Only your surface ink introduces color to its vision. The editable side-view map becomes a 64-unit-deep corridor: ground forms solid blocks, and the flag has a cylindrical pole and billowing cloth. Paint follows those surfaces; empty-space ink maps to both corridor side walls. Wall paint stays behind terrain, and erasing Ground reveals the wall underneath. Objects enlarge as the fly approaches, and terrain hides what is behind it. The camera turns with travel and retains its heading at rest. Physics and editing stay in the 2D plane.

Rough solid walls and a ceiling line the left, right, and top map edges. You can paint their inward faces and borders to give the fly visible cues. Like the original ground, their geometry stays fixed. The bottom gap remains open and hazardous.

Yellow visible marks encourage approach; red marks trigger a turn and a brief committed retreat. Separate trigger and rearm thresholds prevent repeated flips while the old neural response fades. If strong red persists after retreat, the fly holds. Without strong color evidence it explores, eventually turning to search. **Brain** reveals its current decision and yellow-pull/red-push signal strengths. These color associations are explicitly engineered game rules.

**Music** in the toolbox turns a quiet mono procedural score on or off; the adjacent slider controls volume. The same mix plays through both speakers. Live modeled activity shapes soft plucks, a buzzing tonal layer, and little rhythmic accents. Yellow-pull evidence warms the sound, red-push evidence adds tension, and changes in sampled neurons vary the phrase. Music fades when paused and restarts its phrase on reset or bookmark restore. These are authored musical mappings, not biological brain recordings. [Music behavior and implementation](docs/neural-music.md).

| Control | Action |
|---|---|
| 1–4 | Ground, ink, erase ground, erase ink |
| Space | Pause / play |
| Enter | Begin the opening tutorial when ready |
| R | Reset the current level and brain |
| N | Next level after a win |
| Cmd/Ctrl Z | Undo an edit |
| Shift Cmd/Ctrl Z | Redo |
| F5 / F9 | Save / restore world and brain together, within this session |
| H | Controls |
| C | Hold to inspect collision cells |
| Whole map / Follow | Switch spectator framing |
| Middle drag | Pan when zoomed |
| Esc | Cancel a stroke |

Drawing stays live. Completed fragments commit at decision boundaries, and Undo groups the fragments of one drag into one scribble. The brush skips protected cells and the space occupied by the fly instead of rejecting a whole hurried stroke. Erasers affect your own layers. Space is an explicit pause. A worker fault pauses the simulation; Reset restarts it.

## Set up a fresh checkout

On macOS with Metal (validated on Apple Silicon), install Rust/Cargo, Python 3.12 and `uv`, then:

```sh
./scripts/setup.sh
./scripts/run.sh
```

Setup downloads about 1.2 GB of official MaleCNS tables, builds the sparse graph, and compiles the game. The first Bevy build takes several minutes. Subsequent play is entirely local. On macOS, `./scripts/package-macos.sh` builds an optimized app bundle with one game executable. Prepared assets must exist when compiling; they are embedded at build time. Distribution signing and notarization remain separate steps.

## What drives it

`world_core` owns deterministic terrain, physics, rendering, and snapshots. `desktop` owns the Bevy app, bevy-egui drawing tools, and bounded background worker communication. `wire_types` enforces the image-only step protocol. `brain_core` runs the sparse graph through Candle/Metal. The game calls the shared `brain_worker` session directly on its background thread; the separate worker binary retains the localhost protocol for preparation and reference tests. `controller/brainworker` prepares official MaleCNS v1.0 data and retains the Python reference model.

The prepared graph contains **164,606 traced neurons and 25,558,671 connections**, with **4,107 selected sensory inputs**. The target is **25 decisions per wall-clock second**: two 20 ms neural steps and four 10 ms physics steps per decision. Physics waits for the matching action, while the coordinator counts that wait toward the 40 ms deadline. It carries ordinary frame overshoot and bounds backlog to 80 ms; overload slows simulation without skipping neural steps or applying stale actions. The UI stays responsive.

The model uses declared leaky rate dynamics, approximate transmitter signs, two engineered RGB chromatic channels, and a fixed approach/retreat/search/hold motor interface. The channels use interleaved sensory inputs; 2,183 approach and 2,139 avoidance relay neurons are selected by their actual synaptic connectivity. Their activity above a fixed neutral baseline supplies movement evidence. The current controller profile is `eye-level-v2 / ChromaticValenceV1`. Receptor coordinates were unavailable for the selected population; their image sampling uses an explicit deterministic stratified mapping. Learning is off. The upper-right display uses two projections of 336 actual MaleCNS neuron skeletons. Blue shows sampled structure; orange highlights the strongest local increases in corresponding modeled activity above its recent baseline. Display contrast emphasizes the upper tenth of these increases, suppressing a uniform orange wash as the graph wakes. It is a scan-like skeleton projection, not a CT volume or an animation driven by walking speed. The lower-right image is the exact 128 × 96 RGB frame sent to the worker; before the first step, or after a paused edit, it previews the next look. There are no goal coordinates, object labels, or paths in its play requests.

## Current state

[Standalone deployment and final polish verification](docs/final-polish.md).

Six levels are implemented in the approved order: First Flight, Borrowed Ground, Touch and Go, Pull the Rug, Swat Team, and The Long Way Home. Use the toolbox chooser to replay any level, or **Next level / N** after a win. Every fresh level and normal reset waits for the first painting gesture, which starts the fly automatically. Later strokes respect an explicit pause. Later levels convey their rules through labels and visible behavior; the final level combines familiar rules. [Campaign rules and verification](levels/README.md). Maximum horizontal speed is 176 world pixels/s, acceleration is 1,056, and gravity is 1,100. Exploration uses 52% motor output; retreat uses 85% for 20 decisions (0.8 simulated seconds). Search reverses after 88 decisions (3.52 s), commits for ten (0.4 s), and rearms red retreat after eight clear observations (0.32 s). The body acceleration cap smooths velocity without an additional motor low-pass. Neural and physics timesteps remain 20 ms and 10 ms; only the observation/decision cadence changes. The full graph stays float32 and sparse, with an exact NumPy-compatible PCG64 normal stream. [The original feasibility evaluation](experiments/candle-evaluation/README.md) is historical; the production worker now lives in the Rust workspace. The Python worker remains a slower legacy reference, not a silent fallback.

Older `SurfaceRetinaV1` profiles must be regenerated with `.venv/bin/python -m controller.brainworker.prepare`. The new profile rejects incompatible brain bookmarks. The Rust export is `data/cache/malecns-rust-v1` and uses protocol version 2. Runtime fingerprints include the backend and model implementation. Current bookmarks include retreat commitment, rearm state, search state, neural activity, and complete RNG state; legacy Python bookmarks are rejected.

The earlier packaged Rust/Metal build with a separate worker process sustained **25.0008 decisions/s** across 1,260 decisions. Median worker exchange was 9.2 ms, p95 13.0 ms, with a 71.9 ms maximum in that run. A 115-decision Python comparison measured maximum checked activity error of 1.19×10⁻⁷, steering error of 1.57×10⁻⁸, matching motor modes and exact RNG/checkpoint replay. These are local measurements, not a guarantee for every input or machine. [Production verification and limits](docs/rust-brain-25hz.md).

The previous slower build completed a native bridge-and-ink playthrough in 10.5 simulated seconds. That result is historical after the pace and editing changes. Current verification emphasizes native drawing, danger, and visual response. Broad usability, biological fidelity, and learning remain unestablished.

The earlier cutaway-view experiments are historical and deliberately refuse to run against the new perspective controller. Their results do not validate this version. `config/desktop.toml` documents defaults, `levels/01_bridge.json` describes the tutorial geometry, and `crates/world_core/src/levels.rs` defines the campaign. These are baked levels; the runtime does not load arbitrary level files or the TOML file.

For short verification, run `./scripts/verify.sh`. Add `--full` for real Rust/Metal accuracy, RNG, socket/checkpoint/replay and legacy Python transport checks. Native play is the primary check for feel. Normal app launches keep journals, recent input frames, and bookmarks in `~/Library/Application Support/FlyAndYou/runs/play-*`. `FLY_AND_YOU_ROOT` can explicitly choose a different output root for development. In-process faults freeze play; Reset recovers ordinary worker failures. A hung Metal driver cannot be forcibly terminated independently of the app.

## Data and credits

Connectivity and annotations come from the [MaleCNS v1.0 dataset](https://male-cns.janelia.org/download/), a collaboration of FlyEM / HHMI Janelia, the University of Cambridge, MRC Laboratory of Molecular Biology, and Google Research. The dataset is licensed under [CC BY 4.0](https://creativecommons.org/licenses/by/4.0/). Source URLs, SHA-256 hashes, selection rules, and transformations are recorded in `data/source.lock.json`, the cache manifest, and `assets/brain_atlas.json`. The display atlas samples up to 600 branch segments per selected neuron and clips morphology to the brain; regenerate it with `python -m controller.brainworker.anatomy` after setup. The game filters, normalizes, assigns approximate signs, and adds its own sensory and motor mappings; those modifications are not claims made by the dataset authors.

Game artwork is original, generated directly by the renderer. The supplied Org guide and extracted reference examples remain in the repository; the implementation notes at the beginning of the guide describe the subsequent user-directed changes.

The Rust random sampler includes adapted NumPy/Julia/PCG code; retain [third-party notices](THIRD_PARTY_NOTICES.md) when distributing the application.
