"""Opt-in production Rust/Metal socket, exact RNG, numerical and bookmark checks."""
import base64
import copy
import hashlib
import json
import os
import socket
import subprocess
import time
import unittest
from pathlib import Path

import numpy as np
from controller.brainworker.model import Brain
from controller.brainworker.chromatic import encode
from controller.brainworker.motor import MotorConfig
from reference.contracts import encode_packet, read_packet

ROOT = Path(__file__).resolve().parents[2]


@unittest.skipUnless(os.environ.get("FLY_RUST_TEST") == "1", "set FLY_RUST_TEST=1 for full Rust/Metal verification")
class RustWorkerTests(unittest.TestCase):
    def test_full_graph_accuracy_replay_and_transport(self):
        run = ROOT / "runs/rust-worker-test"
        run.mkdir(parents=True, exist_ok=True)
        profile = json.loads((ROOT / "data/cache/malecns-rust-v1/manifest.json").read_text())
        ref = Brain.load(ROOT / "data/cache/malecns-v1")
        ref.motor_config = MotorConfig(**profile["motor"])
        with (run / "worker.log").open("w") as err:
            p = subprocess.Popen([str(ROOT / "target/release/fly-brain-worker"), "--runs", "runs/rust-worker-test"], cwd=ROOT, stdout=subprocess.PIPE, stderr=err, text=True)
            try:
                port = json.loads(p.stdout.readline())["port"]
                with socket.create_connection(("127.0.0.1", port), timeout=30) as sock:
                    sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
                    stream = sock.makefile("rb")
                    loaded = read_packet(stream)
                    self.assertEqual(loaded["kind"], "loaded", loaded)
                    self.assertEqual(loaded["telemetry"]["backend"], "candle-metal-csr-f32-v1")
                    def call(r):
                        sock.sendall(encode_packet(r)); reply = read_packet(stream)
                        self.assertNotEqual(reply["kind"], "fault", reply)
                        return reply
                    epoch = "b" * 32
                    call(dict(kind="restore", epoch=epoch, checkpoint="initial"))
                    def request(i, frame):
                        rgb = frame.tobytes()
                        return dict(kind="step", version=2, epoch=epoch, step_id=i, physics_tick=i*4, world_revision=1, profile_sha256=loaded["profile_sha256"], rgb_sha256=hashlib.sha256(rgb).hexdigest(), width=128, height=96, format="rgb8", neural_steps=2, frame_b64=base64.b64encode(rgb).decode())
                    def inspect(i): return call(dict(kind="inspect", epoch=epoch, step_id=i))["telemetry"]
                    def state(name):
                        call(dict(kind="snapshot", epoch=epoch, name=name))
                        return json.loads((run / f"{name}.brain.json").read_text())["state"]
                    frames = [np.full((96,128,3), c, np.uint8) for c in [(100,100,100),(232,186,60),(195,80,57)]]
                    sequence = [frames[0]]*10 + [frames[1]]*20 + [frames[2]]*35 + [frames[0]]*30 + [frames[1]]*10
                    # Include real eye-level renderer output, not only flat RGB fields.
                    rendered = json.loads(subprocess.check_output(["cargo","run","--quiet","--locked","-p","world_core","--example","tutorial_frames"], cwd=ROOT))
                    sequence += [np.array(s["rgb"],np.uint8).reshape(96,128,3) for s in rendered["scenes"]]*5
                    max_activity = max_steer = 0.; modes=set(); latencies=[]
                    for i,frame in enumerate(sequence):
                        currents = encode(frame[ref.samples[:,1],ref.samples[:,0]],ref.yellow_mask)
                        ref.previous = currents.copy()
                        for _ in range(2): ref.tick(currents)
                        expected = ref.motor.step(*map(float,ref.evidence()),ref.motor_config)
                        r=request(i,frame); start=time.perf_counter(); reply=call(r); observed=inspect(i); latencies.append((time.perf_counter()-start)*1000)
                        for k in ("version","epoch","step_id","physics_tick","world_revision","profile_sha256","rgb_sha256"): self.assertEqual(reply[k],r[k])
                        self.assertEqual(reply["neural_steps_done"],2)
                        self.assertEqual(reply["action"]["jump"],False)
                        self.assertEqual(observed["ticks"],(i+1)*2)
                        self.assertEqual(observed["motor_mode"],ref.motor.mode)
                        modes.add(ref.motor.mode)
                        max_steer=max(max_steer,abs(reply["action"]["steer"]-expected))
                        if i in (12,40,80,len(sequence)-1):
                            saved=state("accuracy")
                            a=np.frombuffer(base64.b64decode(saved["activity"]),np.float32)
                            max_activity=max(max_activity,float(np.max(np.abs(a-ref.activity))))
                            rng=copy.deepcopy(saved["rng"]);rng["state"]={k:int(v) for k,v in rng["state"].items()}
                            self.assertEqual(rng,ref.rng.bit_generator.state)
                            self.assertEqual({k:v for k,v in saved["motor"].items() if k!="steer"},{k:v for k,v in ref.motor.snapshot().items() if k!="steer"})
                        if i==40:
                            self.assertEqual(call(r),reply) # duplicate never advances brain
                            self.assertEqual(inspect(i),observed) # inspection is pure
                    self.assertLess(max_activity,1e-5);self.assertLess(max_steer,1e-5)
                    # Save during retreat, then restore into a fresh epoch and replay.
                    epoch="c"*32;call(dict(kind="restore",epoch=epoch,checkpoint="initial"))
                    for i in range(4): call(request(i,frames[2]))
                    self.assertEqual(inspect(3)["motor_mode"],"retreat")
                    state("mid-retreat")
                    def continuation():
                        result=[]
                        for i in range(4,29): result.append((call(request(i,frames[1]))["action"],inspect(i)))
                        return result,state("after-replay")
                    expected,expected_state=continuation()
                    epoch="d"*32;restored=call(dict(kind="restore",epoch=epoch,checkpoint="mid-retreat"))
                    self.assertEqual(restored["physics_tick"],16)
                    self.assertEqual(restored["next_step_id"],4)
                    actual,actual_state=continuation();self.assertEqual(actual,expected);self.assertEqual(actual_state,expected_state)
                    # Retinal disconnection removes image-dependent behavior with identical RNG/state.
                    saved=json.loads((run/"mid-retreat.brain.json").read_text());saved["state"]["connected"]=False
                    (run/"disconnected.brain.json").write_text(json.dumps(saved))
                    traces=[]
                    for j,frame in enumerate((frames[1],frames[2])):
                        epoch=str(j+1)*32;call(dict(kind="restore",epoch=epoch,checkpoint="disconnected"))
                        traces.append([call(request(i,frame))["action"] for i in range(4,14)])
                    self.assertEqual(*traces)
                    # Version 1 requests are explicit faults; they cannot silently use 100 ms cadence.
                    bad=request(14,frames[0]);bad["version"]=1
                    sock.sendall(encode_packet(bad));fault=read_packet(stream);self.assertEqual(fault["kind"],"fault")
                self.assertNotEqual(p.wait(timeout=10),0)
                report=dict(decisions_compared=len(sequence),max_activity_error=max_activity,max_steer_error=max_steer,motor_modes=sorted(modes),rng_exact=True,retreat_checkpoint_replay_exact=True,disconnected_color_actions_equal=True,protocol_version_1_rejected=True,latency_ms=dict(median=float(np.median(latencies)),p95=float(np.percentile(latencies,95)),max=max(latencies)),profile_sha256=loaded["profile_sha256"])
                (run/"verification.json").write_text(json.dumps(report,indent=2));print(json.dumps(report,indent=2))
            finally:
                if p.poll() is None: p.kill();p.wait()
                p.stdout.close()
