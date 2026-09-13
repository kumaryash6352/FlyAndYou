"""Scripted player interventions, real world core and frozen neural controller.
The scripted player's coordinates stay in this harness; Brain receives RGB only.
"""

import hashlib
import json
import subprocess
import time
from pathlib import Path

import numpy as np

from controller.brainworker.model import Brain

ROOT = Path(__file__).resolve().parents[1]


def main():
    raise SystemExit(
        "Historical cutaway-view experiment; does not validate eye-level-v1. Use native play for the current build."
    )
    proc = subprocess.Popen(
        [str(ROOT / "target/debug/examples/headless")],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        text=True,
    )

    def command(**r):
        proc.stdin.write(json.dumps(r) + "\n")
        proc.stdin.flush()
        v = json.loads(proc.stdout.readline())
        if "error" in v:
            raise ValueError(v["error"])
        return v

    b = Brain.load(ROOT / "data/cache/malecns-v1")
    trials = []
    try:
        for seed in [701, 702, 703]:
            b.rng = np.random.default_rng(seed)
            b.activity.fill(0)
            b.previous.fill(0)
            b.steer = 0
            b.ticks = 0
            base = b.snapshot()
            for bridge, cue in [
                (False, False),
                (True, False),
                (False, True),
                (True, True),
            ]:
                w = command(kind="reset")
                b.restore(base)
                edits = []
                actions = []
                last = None
                start = time.perf_counter()
                if bridge:
                    w = command(
                        kind="edit",
                        tool="solid",
                        points=[[188, 282], [292, 282]],
                        radius=4,
                    )
                for block in range(450):
                    if cue and (last is None or w["x"] > last[0] - 40):
                        if last:
                            command(
                                kind="edit", tool="erase_ink", points=[last], radius=24
                            )
                        last = [min(w["x"] + 70, 526), min(w["y"] - 34, 220)]
                        w = command(kind="edit", tool="ink", points=[last], radius=24)
                        edits.append({"tick": w["tick"], "point": last})
                    frame = np.array(w["rgb"], np.uint8).reshape(96, 128, 3)
                    a = b.step_image(frame)
                    actions.append(a["steer"])
                    w = command(kind="step", steer=a["steer"])
                    if w["outcome"] != "Running":
                        break
                trial = {
                    "seed": seed,
                    "bridge": bridge,
                    "cue": cue,
                    "outcome": w["outcome"],
                    "x": w["x"],
                    "tick": w["tick"],
                    "wall_seconds": time.perf_counter() - start,
                    "edits": edits,
                    "actions": actions,
                }
                trials.append(trial)
                print(
                    {k: v for k, v in trial.items() if k not in ("actions", "edits")},
                    flush=True,
                )
        report = {
            "profile_sha256": hashlib.sha256(
                (ROOT / "data/cache/malecns-v1/manifest.json").read_bytes()
            ).hexdigest(),
            "trials": trials,
            "limits": "Three seeds with shared geometry and a scripted player moving a cue. This is not unfamiliar-user usability validation or independent biological evidence.",
        }
        (ROOT / "runs/closed_loop.json").write_text(json.dumps(report, indent=2))
    finally:
        proc.terminate()
        proc.wait()


if __name__ == "__main__":
    main()
