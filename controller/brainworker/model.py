"""Declared leaky rate approximation; activity is not a biological spike recording."""

import base64
import copy
import hashlib
import json
from pathlib import Path

import numpy as np
from scipy import sparse

from .chromatic import READOUT_SCALE, SENSOR_PROFILE, derive_readout, encode
from .motor import MotorConfig, MotorState


class Brain:
    def __init__(
        self,
        w,
        inputs,
        samples,
        left,
        right,
        seed=7,
        profile=None,
        readout=None,
        baseline=None,
        motor_config=None,
        readout_scale=READOUT_SCALE,
    ):
        self.w = w.astype(np.float32)
        self.inputs = np.asarray(inputs, dtype=np.int32)
        self.samples = np.asarray(samples, dtype=np.int32)
        self.left = np.asarray(left, dtype=np.int32)
        self.right = np.asarray(right, dtype=np.int32)
        if not len(self.left) or not len(self.right) or not len(self.inputs):
            raise ValueError("Missing declared input/output population")
        readout = derive_readout(self.w, self.inputs) if readout is None else readout
        self.yellow_mask = np.asarray(readout["yellow_mask"])
        self.approach_indices = np.asarray(readout["approach_indices"])
        self.avoid_indices = np.asarray(readout["avoid_indices"])
        self.baseline = np.asarray(
            [0.0, 0.0] if baseline is None else baseline, np.float32
        )
        self.readout_scale = readout_scale
        if (
            self.yellow_mask.dtype != np.bool_
            or self.yellow_mask.shape != self.inputs.shape
            or not self.yellow_mask.any()
            or self.yellow_mask.all()
            or self.baseline.shape != (2,)
            or not np.isfinite(self.baseline).all()
            or (self.baseline < 0).any()
            or (self.baseline > 1).any()
            or not np.isfinite(readout_scale)
            or readout_scale <= 0
        ):
            raise ValueError("Invalid chromatic profile")
        for indices in (self.approach_indices, self.avoid_indices):
            if (
                indices.ndim != 1
                or not len(indices)
                or not np.issubdtype(indices.dtype, np.integer)
                or (indices < 0).any()
                or (indices >= w.shape[0]).any()
                or len(np.unique(indices)) != len(indices)
                or np.intersect1d(indices, self.inputs).size
            ):
                raise ValueError("Invalid postsynaptic readout population")
        if np.intersect1d(self.approach_indices, self.avoid_indices).size:
            raise ValueError("Chromatic readout populations overlap")
        self.activity = np.zeros(w.shape[0], np.float32)
        self.rng = np.random.default_rng(seed)
        self.ticks = 0
        self.connected = True
        self.noise = 0.0005
        self.recurrence = 0.9
        self.retention = 0.7
        self.motor_config = motor_config or MotorConfig()
        self.motor = MotorState()
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
        if manifest.get("schema") != 2 or manifest.get("sensor") != SENSOR_PROFILE:
            raise ValueError(
                "Controller profile requires ChromaticValenceV1; run .venv/bin/python -m controller.brainworker.prepare"
            )
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
            readout={
                key: a[key]
                for key in ("yellow_mask", "approach_indices", "avoid_indices")
            },
            baseline=manifest["readout"]["neutral_baseline"],
            motor_config=MotorConfig(**manifest["motor"]),
            readout_scale=manifest["readout"]["scale"],
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
        b.recurrence = cfg["recurrence_gain"]
        b.retention = cfg["retention"]
        a.close()
        return b

    @property
    def steer(self):
        return self.motor.steer

    def population_rates(self):
        return np.array(
            [
                self.activity[self.approach_indices].mean(),
                self.activity[self.avoid_indices].mean(),
            ],
            np.float32,
        )

    def evidence(self):
        return (
            np.maximum(self.population_rates() - self.baseline, 0) / self.readout_scale
        )

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
        currents = encode(
            frame[self.samples[:, 1], self.samples[:, 0]], self.yellow_mask
        )
        self.previous = currents.copy()
        for _ in range(5):
            self.tick(currents)
        approach, avoidance = self.evidence()
        steer = self.motor.step(float(approach), float(avoidance), self.motor_config)
        return {"steer": steer, "jump": False}

    def inspect(self):
        approach, avoidance = self.evidence()
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
            "heading": self.motor.heading,
            "search_age": self.motor.search_age,
            "approach": float(approach),
            "avoidance": float(avoidance),
            "motor_mode": self.motor.mode,
            "turn_decisions_remaining": self.motor.remaining,
            "model": "leaky rate; engineered chromatic valence",
        }

    def snapshot(self):
        return {
            "schema": 2,
            "activity": base64.b64encode(self.activity.tobytes()).decode(),
            "previous": base64.b64encode(self.previous.tobytes()).decode(),
            "rng": copy.deepcopy(self.rng.bit_generator.state),
            "ticks": self.ticks,
            "connected": self.connected,
            "motor": self.motor.snapshot(),
        }

    def restore(self, s):
        if (
            not isinstance(s, dict)
            or set(s)
            != {"schema", "activity", "previous", "rng", "ticks", "connected", "motor"}
            or s["schema"] != 2
        ):
            raise ValueError("Incompatible model checkpoint")
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
            or (a < 0).any()
            or (a > 1).any()
            or (p < 0).any()
            or (p > 1).any()
            or type(s["ticks"]) is not int
            or s["ticks"] < 0
            or type(s["connected"]) is not bool
        ):
            raise ValueError("Invalid model checkpoint")
        rng = np.random.default_rng()
        rng.bit_generator.state = s["rng"]
        motor = MotorState.restore(s["motor"], self.motor_config)
        self.activity = a
        self.previous = p
        self.rng = rng
        self.ticks = s["ticks"]
        self.connected = s["connected"]
        self.motor = motor
