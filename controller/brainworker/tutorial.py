"""Prepare reproducible color demonstrations, entirely outside the play session.

The observer is fixed while each image drives the real model. These are recorded
stimulus responses, not navigation trials or biological emotion measurements.
"""

import hashlib
import json
import subprocess
from pathlib import Path

import numpy as np

from .model import Brain

ROOT = Path(__file__).resolve().parents[2]
SOURCES = [
    "controller/brainworker/model.py",
    "controller/brainworker/chromatic.py",
    "controller/brainworker/motor.py",
    "controller/brainworker/tutorial.py",
    "assets/brain_atlas.json",
]


def main():
    source_hashes = {
        name: hashlib.sha256((ROOT / name).read_bytes()).hexdigest()
        for name in SOURCES
    }
    profile = ROOT / "data/cache/malecns-v1"
    profile_hash = hashlib.sha256((profile / "manifest.json").read_bytes()).hexdigest()
    rendered = json.loads(subprocess.check_output(
        ["cargo", "run", "--locked", "--quiet", "-p", "world_core", "--example", "tutorial_frames"],
        cwd=ROOT,
    ))
    identity = dict(schema=1, profile_sha256=profile_hash, sources=source_hashes,
                    sensor=rendered["sensor"], seed=7, seconds_per_sample=0.2)
    output = ROOT / "data/cache/tutorial.json"
    if output.exists():
        cached = json.loads(output.read_text())
        if all(cached.get(k) == v for k, v in identity.items()) and len(cached.get("scenes", [])) == 2 and all(
            old.get("world_sha256") == new["world_sha256"]
            and old.get("rgb_sha256") == new["rgb_sha256"]
            for old, new in zip(cached.get("scenes", []), rendered["scenes"], strict=True)
        ):
            print("Tutorial color responses are current.")
            return
    brain = Brain.load(profile, seed=7)
    initial = brain.snapshot()
    scenes = []
    for scene, mode in zip(rendered["scenes"], ("retreat", "approach"), strict=True):
        brain.restore(initial)
        image = np.array(scene.pop("rgb"), dtype=np.uint8).reshape(96, 128, 3)
        samples = [brain.inspect()]
        for _ in range(20):
            brain.step_image(image)
            samples.append(brain.inspect())
        if not any(s["motor_mode"] == mode for s in samples):
            raise RuntimeError(f"Tutorial cue did not evoke {mode}; inspect the actual image and profile")
        scene["samples"] = samples
        scenes.append(scene)
        print(f"Recorded {mode}: {len(samples)} samples, {scene['rgb_sha256'][:12]}")
    blob = json.dumps(dict(**identity, scenes=scenes), allow_nan=False, separators=(",", ":"))
    temporary = output.with_suffix(".tmp")
    temporary.write_text(blob)
    temporary.replace(output)


if __name__ == "__main__":
    main()
