"""Short full-graph check of the integrated controller and unchanged world core."""
import hashlib
import json
from pathlib import Path
import numpy as np
from controller.brainworker.model import Brain
from measure_current import World

out = Path(__file__).resolve().parent
b = Brain.load(out.parents[1] / "data/cache/malecns-v1")
assert b.ticks == 0
initial = b.snapshot()
w = World()
trials = []
try:
    for color_name, color in [("red", [195, 80, 57]), ("yellow", [232, 186, 60])]:
        for heading in [1, -1]:
            state = w.call(kind="reset", x=340, facing=heading, bridge=True)
            if color_name == "red":
                x = 340 + heading * 40
                w.call(kind="edit", tool="solid", points=[[x, 216], [x, 276]], radius=4)
                state = w.call(kind="edit", tool="ink", points=[[x, 216], [x, 276]], radius=12, color=color)
            else:
                state = w.call(kind="edit", tool="ink", points=[[340 + heading*16, 280], [340 + heading*64, 280]], radius=8, color=color)
            b.restore(initial)
            b.motor.heading = heading
            trace = []
            for _ in range(12):
                action = b.step_image(np.array(state["rgb"], np.uint8).reshape(96, 128, 3))
                state = w.call(kind="step", steer=action["steer"])
                trace.append(dict(x=state["x"], steer=action["steer"], mode=b.motor.mode))
            displacement = state["x"] - 340
            expected_sign = -heading if color_name == "red" else heading
            assert displacement * expected_sign > 100
            trial = dict(color=color_name, facing=heading, displacement=displacement, trace=trace)
            trials.append(trial)
            print(color_name, heading, round(displacement, 2), flush=True)
    disconnected = []
    for color in [[195, 80, 57], [232, 186, 60]]:
        b.restore(initial)
        b.connected = False
        frame = np.tile(np.array(color, np.uint8), (96, 128, 1))
        disconnected.append([b.step_image(frame) for _ in range(12)])
    assert disconnected[0] == disconnected[1]
    report = dict(profile=b.profile["sensor"], profile_sha256=hashlib.sha256((out.parents[1]/"data/cache/malecns-v1/manifest.json").read_bytes()).hexdigest(), trials=trials, disconnection_identical=True, startup_ticks=0)
    (out / "production-motion.json").write_text(json.dumps(report, indent=2))
finally:
    w.close()
