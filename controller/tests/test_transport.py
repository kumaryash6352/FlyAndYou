"""A real subprocess/socket round trip; explicitly opt in to the full data test."""

import base64
import hashlib
import json
import os
import socket
import subprocess
import unittest
from pathlib import Path

from reference.contracts import encode_packet, read_packet

ROOT = Path(__file__).resolve().parents[2]


@unittest.skipUnless(
    os.environ.get("FLY_FULL_TEST") == "1",
    "set FLY_FULL_TEST=1 for full graph subprocess test",
)
class TransportTests(unittest.TestCase):
    def test_live_worker_step_snapshot_restore_and_shutdown(self):
        p = subprocess.Popen(
            [
                str(ROOT / ".venv/bin/python"),
                "-m",
                "controller.brainworker.service",
                "--runs",
                "runs/transport-test",
            ],
            cwd=ROOT,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        try:
            port = json.loads(p.stdout.readline())["port"]
            with socket.create_connection(("127.0.0.1", port), timeout=30) as sock:
                stream = sock.makefile("rb")
                loaded = read_packet(stream)
                profile = loaded["profile_sha256"]

                def call(r):
                    sock.sendall(encode_packet(r))
                    return read_packet(stream)

                call(dict(kind="restore", epoch="1" * 32, checkpoint="initial"))
                self.assertEqual(
                    call(dict(kind="snapshot", epoch="1" * 32, name="before-play"))[
                        "physics_tick"
                    ],
                    0,
                )
                rgb = bytes([232, 186, 60]) * (128 * 96)
                r = dict(
                    kind="step",
                    version=1,
                    epoch="1" * 32,
                    step_id=0,
                    physics_tick=0,
                    world_revision=1,
                    profile_sha256=profile,
                    rgb_sha256=hashlib.sha256(rgb).hexdigest(),
                    width=128,
                    height=96,
                    format="rgb8",
                    neural_steps=5,
                    frame_b64=base64.b64encode(rgb).decode(),
                )
                first = call(r)
                self.assertEqual(first["kind"], "action")
                self.assertGreater(first["action"]["steer"], 0)
                self.assertEqual(first, call(r))
                inspect = call(dict(kind="inspect", epoch="1" * 32, step_id=0))
                self.assertEqual(inspect["telemetry"]["ticks"], 5)
                self.assertEqual(
                    call(
                        dict(kind="restore", epoch="2" * 32, checkpoint="before-play")
                    )["physics_tick"],
                    0,
                )
                r["epoch"] = "2" * 32
                again = call(r)
                self.assertEqual(first["action"], again["action"])
                # A joint bookmark must retain a red-triggered commitment even
                # when its next observation is yellow and the epoch changes.
                call(dict(kind="restore", epoch="3" * 32, checkpoint="initial"))
                red = bytes([195, 80, 57]) * (128 * 96)
                r.update(
                    epoch="3" * 32,
                    rgb_sha256=hashlib.sha256(red).hexdigest(),
                    frame_b64=base64.b64encode(red).decode(),
                )
                self.assertLess(call(r)["action"]["steer"], 0)
                state = call(dict(kind="inspect", epoch="3" * 32, step_id=0))[
                    "telemetry"
                ]
                self.assertEqual(state["motor_mode"], "retreat")
                self.assertGreater(state["turn_decisions_remaining"], 0)
                call(dict(kind="snapshot", epoch="3" * 32, name="mid-retreat"))

                def continuation(epoch):
                    actions = []
                    for step in range(1, 11):
                        r.update(
                            epoch=epoch,
                            step_id=step,
                            physics_tick=step * 10,
                            rgb_sha256=hashlib.sha256(rgb).hexdigest(),
                            frame_b64=base64.b64encode(rgb).decode(),
                        )
                        actions.append(call(r)["action"])
                    return actions, call(dict(kind="inspect", epoch=epoch, step_id=10))[
                        "telemetry"
                    ]

                expected = continuation("3" * 32)
                restored = call(
                    dict(kind="restore", epoch="4" * 32, checkpoint="mid-retreat")
                )
                self.assertEqual(restored["physics_tick"], 10)
                self.assertEqual(restored["next_step_id"], 1)
                self.assertEqual(expected, continuation("4" * 32))
                self.assertEqual(
                    call(dict(kind="shutdown", epoch="4" * 32))["kind"], "shutdown"
                )
            self.assertEqual(p.wait(timeout=10), 0)
        finally:
            if p.poll() is None:
                p.kill()
                p.wait()
            p.stdout.close()
            p.stderr.close()
