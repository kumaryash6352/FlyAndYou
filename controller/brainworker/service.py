"""One serialized model, exact image-only play messages, local transport."""

import argparse
import hashlib
import json
import re
import socket
import sys
import time
from pathlib import Path

import numpy as np

from reference.contracts import (
    IDENTITY_FIELDS,
    encode_packet,
    read_packet,
    validate_action,
    validate_step,
)

from .model import Brain

ROOT = Path(__file__).resolve().parents[2]


def fields(r, expected):
    if set(r) != set(expected):
        raise ValueError("Unexpected command fields")


def name_path(root, name):
    if not isinstance(name, str) or not re.fullmatch(r"[a-zA-Z0-9_-]{1,80}", name):
        raise ValueError("Invalid checkpoint name")
    return root / (name + ".brain.json")


class Session:
    def __init__(self, brain, profile_hash, root):
        self.brain = brain
        self.profile_hash = profile_hash
        self.root = Path(root)
        self.root.mkdir(parents=True, exist_ok=True)
        self.initial = brain.snapshot()
        self.epoch = None
        self.next_id = 0
        self.cached = None
        self.elapsed_ms = 0.0

    def handle(self, r):
        kind = r.get("kind")
        if kind == "step":
            raw = validate_step(r)
            if r["profile_sha256"] != self.profile_hash:
                raise ValueError("Wrong profile")
            if self.epoch is None:
                self.epoch = r["epoch"]
            if r["epoch"] != self.epoch:
                raise ValueError("Stale epoch")
            digest = hashlib.sha256(encode_packet(r)).hexdigest()
            if self.cached and r["step_id"] == self.cached[0]:
                if digest != self.cached[1]:
                    raise ValueError("Duplicate id with changed content")
                return self.cached[2]
            if (
                r["step_id"] != self.next_id
                or r["physics_tick"] != self.brain.ticks * 2
            ):
                raise ValueError("Unexpected decision boundary")
            start = time.perf_counter()
            action = self.brain.step_image(
                np.frombuffer(raw, np.uint8).reshape(96, 128, 3)
            )
            self.elapsed_ms = (time.perf_counter() - start) * 1000
            reply = {k: r[k] for k in IDENTITY_FIELDS}
            reply.update(kind="action", neural_steps_done=5, action=action)
            validate_action(reply, r)
            self.next_id += 1
            self.cached = (r["step_id"], digest, reply)
            return reply
        if kind == "restore":
            fields(r, ("kind", "epoch", "checkpoint"))
            if not isinstance(r["epoch"], str) or not re.fullmatch(
                "[a-f0-9]{32}", r["epoch"]
            ):
                raise ValueError("Invalid epoch")
            if r["checkpoint"] == "initial":
                state = self.initial
                next_id = 0
            else:
                data = json.loads(name_path(self.root, r["checkpoint"]).read_text())
                if data["profile_sha256"] != self.profile_hash:
                    raise ValueError("Incompatible checkpoint profile")
                state = data["state"]
                next_id = data["next_id"]
            self.brain.restore(state)
            self.next_id = next_id
            self.cached = None
            self.epoch = r["epoch"]
            return dict(
                kind="restored",
                epoch=self.epoch,
                profile_sha256=self.profile_hash,
                next_step_id=self.next_id,
                physics_tick=self.brain.ticks * 2,
                telemetry=self.brain.inspect(),
            )
        if r.get("epoch") != self.epoch:
            raise ValueError("Stale epoch")
        if kind == "inspect":
            fields(r, ("kind", "epoch", "step_id"))
            if r["step_id"] != self.next_id - 1:
                raise ValueError("Inspection boundary mismatch")
            return dict(
                kind="inspection",
                epoch=self.epoch,
                step_id=r["step_id"],
                elapsed_ms=self.elapsed_ms,
                telemetry=self.brain.inspect(),
            )
        if kind == "snapshot":
            fields(r, ("kind", "epoch", "name"))
            p = name_path(self.root, r["name"])
            data = dict(
                profile_sha256=self.profile_hash,
                next_id=self.next_id,
                state=self.brain.snapshot(),
            )
            blob = json.dumps(data, allow_nan=False).encode()
            tmp = p.with_suffix(".tmp")
            tmp.write_bytes(blob)
            tmp.replace(p)
            return dict(
                kind="snapshot",
                name=r["name"],
                sha256=hashlib.sha256(blob).hexdigest(),
                physics_tick=self.brain.ticks * 2,
            )
        if kind == "shutdown":
            fields(r, ("kind", "epoch"))
            return dict(kind="shutdown")
        raise ValueError("Unsupported command")


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--profile", default="data/cache/malecns-v1")
    parser.add_argument("--runs", default="runs/live")
    args = parser.parse_args()
    profile = (ROOT / args.profile).resolve()
    runs = (ROOT / args.runs).resolve()
    if not profile.is_relative_to(ROOT / "data/cache") or not runs.is_relative_to(
        ROOT / "runs"
    ):
        raise ValueError("Paths must stay in configured roots")
    with socket.socket() as server:
        server.bind(("127.0.0.1", 0))
        server.listen(1)
        server.settimeout(30)
        print(json.dumps({"port": server.getsockname()[1]}), flush=True)
        brain = Brain.load(profile)
        profile_hash = hashlib.sha256(
            (profile / "manifest.json").read_bytes()
        ).hexdigest()
        session = Session(brain, profile_hash, runs)
        with server.accept()[0] as conn:
            conn.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
            conn.sendall(
                encode_packet(
                    dict(
                        kind="loaded",
                        profile_sha256=profile_hash,
                        mode="MaleCNS fixed controller",
                        nodes=brain.w.shape[0],
                        edges=brain.w.nnz,
                        telemetry=brain.inspect(),
                    )
                )
            )
            stream = conn.makefile("rb")
            while True:
                try:
                    req = read_packet(stream)
                    reply = session.handle(req)
                    conn.sendall(encode_packet(reply))
                    if req["kind"] == "shutdown":
                        break
                except EOFError:
                    break
                except Exception as e:
                    print(str(e), file=sys.stderr, flush=True)
                    try:
                        conn.sendall(encode_packet(dict(kind="fault", error=str(e))))
                    except OSError:
                        pass
                    break


if __name__ == "__main__":
    main()
