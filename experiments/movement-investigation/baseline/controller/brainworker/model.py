"""Declared leaky rate approximation; activity is not a biological spike recording."""

import base64
import copy
import hashlib
import json
from pathlib import Path

import numpy as np
from scipy import sparse


class Brain:
    def __init__(self, w, inputs, samples, left, right, seed=7, profile=None):
        self.w = w.astype(np.float32)
        self.inputs = np.asarray(inputs, dtype=np.int32)
        self.samples = np.asarray(samples, dtype=np.int32)
        self.left = np.asarray(left, dtype=np.int32)
        self.right = np.asarray(right, dtype=np.int32)
        if not len(self.left) or not len(self.right) or not len(self.inputs):
            raise ValueError("Missing declared input/output population")
        self.activity = np.zeros(w.shape[0], np.float32)
        self.rng = np.random.default_rng(seed)
        self.ticks = 0
        self.steer = 0.0
        self.connected = True
        self.noise = 0.0005
        self.recurrence = 0.9
        self.retention = 0.7
        self.gain = 3000.0
        self.bias = 0.00023
        self.motor_retention = 0.65
        self.explore_speed = 0.52
        self.search_steps = 35
        self.heading = 1
        self.search_age = 0
        self.profile = profile or {}
        self.anatomy_indices = np.array([], dtype=np.int32)
        self.previous = np.zeros(len(inputs), np.float32)
        self.telemetry_indices = np.linspace(
            0, w.shape[0] - 1, min(160, w.shape[0]), dtype=int
        )

    @classmethod
    def load(cls, path, seed=7):
        path = Path(path)
        manifest = json.loads((path / "manifest.json").read_text())
        for name, digest in manifest["files"].items():
            if hashlib.sha256((path / name).read_bytes()).hexdigest() != digest:
                raise ValueError("Profile digest mismatch: " + name)
        a = np.load(path / "mapping.npz", allow_pickle=False)
        b = cls(
            sparse.load_npz(path / "weights.npz"),
            a["inputs"],
            a["samples"],
            a["left"],
            a["right"],
            seed,
            manifest,
        )
        b.telemetry_indices = a["telemetry"]
        atlas = json.loads(
            (path.resolve().parents[2] / "assets/brain_atlas.json").read_text()
        )
        for neuron in atlas["neurons"]:
            if int(a["ids"][neuron["index"]]) != neuron["body_id"]:
                raise ValueError("Anatomical atlas does not match graph ordering")
        b.anatomy_indices = np.array(
            [n["index"] for n in atlas["neurons"]], dtype=np.int32
        )
        cfg = manifest["dynamics"]
        b.noise = cfg["noise"]
        b.gain = cfg["readout_gain"]
        b.bias = cfg["readout_bias"]
        b.recurrence = cfg["recurrence_gain"]
        b.retention = cfg["retention"]
        b.motor_retention = cfg["readout_retention"]
        b.explore_speed = cfg["explore_speed"]
        b.search_steps = cfg["search_steps"]
        if manifest["sensor"] != "eye-level-v2 / SurfaceRetinaV1":
            raise ValueError(
                "Controller profile requires eye-level-v2 vision; run setup"
            )
        return b

    def tick(self, currents):
        drive = self.recurrence * self.w.dot(self.activity)
        if self.connected:
            drive[self.inputs] += currents
        drive += self.rng.normal(0.0, self.noise, len(drive)).astype(np.float32)
        self.activity *= self.retention
        self.activity += (1.0 - self.retention) * np.tanh(np.maximum(drive, 0))
        self.ticks += 1

    def step_image(self, frame):
        if (
            not isinstance(frame, np.ndarray)
            or frame.shape != (96, 128, 3)
            or frame.dtype != np.uint8
        ):
            raise ValueError("Expected 128 x 96 RGB8")
        rgb = frame[self.samples[:, 1], self.samples[:, 0]].astype(np.float32)
        # Explicit opponent-color receptor current, not a semantic object detector.
        currents = np.maximum((rgb[:, 0] - rgb[:, 2] - 48.0) / 200.0, 0.0).astype(
            np.float32
        )
        self.previous = currents.copy()
        for _ in range(5):
            self.tick(currents)
        left = float(self.activity[self.left].mean())
        right = float(self.activity[self.right].mean())
        # Eye-level images have corridor-left/right, not world-left/right. A
        # declared artificial motor interface approaches a salient view, then
        # searches by turning around after 3.5 simulated seconds without one.
        # Only downstream neural rates enter this readout; it has no map or RGB.
        attraction = float(
            np.clip(self.gain * ((left + right) * 0.5 - self.bias), 0.0, 1.0)
        )
        self.search_age = 0 if attraction > 0.08 else self.search_age + 1
        if self.search_age >= self.search_steps:
            self.heading *= -1
            self.search_age = 0
        speed = self.explore_speed + (1.0 - self.explore_speed) * attraction
        raw = self.heading * speed
        self.steer = (
            self.motor_retention * self.steer + (1.0 - self.motor_retention) * raw
        )
        return {"steer": float(self.steer), "jump": False}

    def inspect(self):
        return {
            "ticks": self.ticks,
            "left": float(self.activity[self.left].mean()),
            "right": float(self.activity[self.right].mean()),
            "input_mean": float(self.previous.mean()),
            "activity": self.activity[self.telemetry_indices].tolist(),
            "anatomy_activity": self.activity[self.anatomy_indices].tolist(),
            "steer": self.steer,
            "nodes": self.w.shape[0],
            "edges": self.w.nnz,
            "connected": self.connected,
            "heading": self.heading,
            "search_age": self.search_age,
            "model": "leaky rate; schematic layout",
        }

    def snapshot(self):
        return {
            "activity": base64.b64encode(self.activity.tobytes()).decode(),
            "previous": base64.b64encode(self.previous.tobytes()).decode(),
            "rng": copy.deepcopy(self.rng.bit_generator.state),
            "ticks": self.ticks,
            "steer": self.steer,
            "connected": self.connected,
            "heading": self.heading,
            "search_age": self.search_age,
        }

    def restore(self, s):
        a = np.frombuffer(
            base64.b64decode(s["activity"], validate=True), dtype=np.float32
        ).copy()
        p = np.frombuffer(
            base64.b64decode(s["previous"], validate=True), dtype=np.float32
        ).copy()
        if (
            a.shape != self.activity.shape
            or p.shape != self.previous.shape
            or not np.isfinite(a).all()
            or not np.isfinite(p).all()
            or not np.isfinite(s["steer"])
            or s["heading"] not in (-1, 1)
            or not isinstance(s["search_age"], int)
            or not 0 <= s["search_age"] < self.search_steps
        ):
            raise ValueError("Invalid model checkpoint")
        rng = np.random.default_rng()
        rng.bit_generator.state = s["rng"]
        self.activity = a
        self.previous = p
        self.rng = rng
        self.ticks = int(s["ticks"])
        self.steer = float(s["steer"])
        self.connected = bool(s["connected"])
        self.heading = int(s["heading"])
        self.search_age = s["search_age"]
