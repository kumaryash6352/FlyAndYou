"""Held-out image assay using RGB frames produced by world_core's real compositor."""

import hashlib
import json
import time
from pathlib import Path

import numpy as np

from controller.brainworker.model import Brain

ROOT = Path(__file__).resolve().parents[1]


def main():
    raise SystemExit(
        "Historical cutaway-view experiment; does not validate eye-level-v1. Use native play for the current build."
    )
    b = Brain.load(ROOT / "data/cache/malecns-v1")
    trials = []
    latencies = []
    paths = sorted((ROOT / "runs/assay-inputs").glob("cue-*.rgb"))
    for seed in [101, 202, 303, 404]:
        b.rng = np.random.default_rng(seed)
        b.activity.fill(0)
        b.previous.fill(0)
        b.steer = 0
        b.ticks = 0
        base = b.snapshot()
        for p in paths:
            b.restore(base)
            frame = np.frombuffer(p.read_bytes(), np.uint8).reshape(96, 128, 3)
            actions = []
            for _ in range(12):
                t = time.perf_counter()
                actions.append(b.step_image(frame)["steer"])
                latencies.append((time.perf_counter() - t) * 1000)
            side = -1 if int(p.stem.split("-")[1]) < 3 else 1
            trials.append(
                {
                    "seed": seed,
                    "frame": p.name,
                    "frame_sha256": hashlib.sha256(p.read_bytes()).hexdigest(),
                    "side": side,
                    "actions": actions,
                    "signed_steer": float(np.mean(actions[-5:]) * side),
                }
            )
            if len(trials) % 18 == 0:
                print("Trials", len(trials), flush=True)
    # Matched state, same background noise; only input path is disconnected.
    control = []
    for p in (paths[0], paths[-1]):
        b.restore(base)
        b.connected = False
        frame = np.frombuffer(p.read_bytes(), np.uint8).reshape(96, 128, 3)
        control.append([b.step_image(frame)["steer"] for _ in range(12)])
    report = {
        "profile_sha256": hashlib.sha256(
            (ROOT / "data/cache/malecns-v1/manifest.json").read_bytes()
        ).hexdigest(),
        "model": "MaleCNS full traced graph; leaky rate approximation; fixed readout",
        "trial_count": len(trials),
        "directional_successes": sum(t["signed_steer"] > 0.1 for t in trials),
        "minimum_signed_steer": min(t["signed_steer"] for t in trials),
        "disconnected_actions_identical": control[0] == control[1],
        "step_latency_ms": {
            k: float(np.percentile(latencies, q))
            for k, q in [("p50", 50), ("p95", 95), ("p99", 99)]
        },
        "limits": "Image sequence steering assay, not closed-loop locomotion, shape recognition, learning, or independent biological validation. Repeated seeds share geometry.",
        "trials": trials,
        "disconnected": control,
    }
    (ROOT / "runs/cue_assay.json").write_text(json.dumps(report, indent=2))
    print(
        json.dumps(
            {k: v for k, v in report.items() if k not in ("trials", "disconnected")},
            indent=2,
        ),
        flush=True,
    )


if __name__ == "__main__":
    main()
