import copy
import json
import tempfile
import unittest
from pathlib import Path

import numpy as np
from scipy import sparse

from controller.brainworker.model import Brain


class ModelTests(unittest.TestCase):
    def brain(self):
        # Presynaptic input 0 -> output 2, input 1 -> output 3.
        w = sparse.csr_matrix(
            ([1.0, 1.0], ([2, 3], [0, 1])), shape=(4, 4), dtype=np.float32
        )
        return Brain(
            w,
            np.array([0, 1]),
            np.array([[32, 48], [96, 48]]),
            np.array([2]),
            np.array([3]),
            seed=7,
        )

    def frame(self, side):
        a = np.full((96, 128, 3), 230, dtype=np.uint8)
        a[36:61, side - 12 : side + 13] = [230, 175, 40]
        return a

    def test_red_repels_and_yellow_attracts_with_identical_red_blue(self):
        # Changing green is the only visual difference; the old retina loses it.
        red, yellow = self.brain(), self.brain()
        r = np.tile(np.array([195, 80, 57], np.uint8), (96, 128, 1))
        y = np.tile(np.array([195, 195, 57], np.uint8), (96, 128, 1))
        self.assertLess(red.step_image(r)["steer"], 0)
        self.assertGreater(yellow.step_image(y)["steer"], 0)

    def test_red_turn_stays_committed_then_holds_without_oscillation(self):
        b = self.brain()
        red = np.tile(np.array([195, 80, 57], np.uint8), (96, 128, 1))
        actions = [b.step_image(red)["steer"] for _ in range(20)]
        self.assertTrue(all(a < 0 for a in actions[:8]))
        self.assertTrue(all(a == 0 for a in actions[8:]))

    def test_synapses_are_required_for_color_dependent_movement(self):
        a, b = self.brain(), self.brain()
        a.w.data.fill(0)
        b.w.data.fill(0)
        red = np.tile(np.array([195, 80, 57], np.uint8), (96, 128, 1))
        yellow = np.tile(np.array([232, 186, 60], np.uint8), (96, 128, 1))
        self.assertEqual(
            [a.step_image(red) for _ in range(12)],
            [b.step_image(yellow) for _ in range(12)],
        )

    def test_disconnect_removes_image_dependence(self):
        a, b = self.brain(), self.brain()
        a.connected = b.connected = False
        for _ in range(20):
            a.step_image(self.frame(32))
            b.step_image(self.frame(96))
        np.testing.assert_array_equal(a.activity, b.activity)
        self.assertEqual(a.steer, b.steer)

    def test_full_snapshot_replays_activity_and_action(self):
        b = self.brain()
        red = np.tile(np.array([195, 80, 57], np.uint8), (96, 128, 1))
        for _ in range(3):
            b.step_image(red)
        state = b.snapshot()
        frames = [self.frame(32)] * 12 + [red] * 12
        actions = [b.step_image(frame) for frame in frames]
        v = b.activity.copy()
        inspection = b.inspect()
        b.restore(state)
        self.assertEqual(actions, [b.step_image(frame) for frame in frames])
        np.testing.assert_array_equal(v, b.activity)
        self.assertEqual(inspection, b.inspect())

    def test_inspect_does_not_advance_state(self):
        b = self.brain()
        s = b.snapshot()
        b.inspect()
        self.assertEqual(s, b.snapshot())

    def test_invalid_frame_does_not_advance_model(self):
        b = self.brain()
        s = b.snapshot()
        with self.assertRaises(ValueError):
            b.step_image(np.zeros((4, 4, 3), dtype=np.uint8))
        self.assertEqual(s, b.snapshot())

    def test_invalid_motor_checkpoint_does_not_partly_restore_brain(self):
        b = self.brain()
        state = b.snapshot()
        b.step_image(self.frame(32))
        before = b.snapshot()
        bad = copy.deepcopy(state)
        bad["motor"]["heading"] = 0
        with self.assertRaises(ValueError):
            b.restore(bad)
        self.assertEqual(before, b.snapshot())

    def test_old_sensory_profile_requires_repreparation(self):
        with tempfile.TemporaryDirectory() as directory:
            Path(directory, "manifest.json").write_text(
                json.dumps({"schema": 1, "sensor": "eye-level-v2 / SurfaceRetinaV1"})
            )
            with self.assertRaisesRegex(ValueError, "ChromaticValenceV1"):
                Brain.load(directory)

    def test_sparse_matches_independent_dense_chain(self):
        b = self.brain()
        b.noise = 0.0
        b.activity[:] = [1.0, 0.0, 0.0, 0.0]
        b.tick(np.zeros(2, dtype=np.float32))
        expected = 0.3 * np.tanh(0.9)
        self.assertAlmostEqual(float(b.activity[2]), float(expected), places=6)

    def test_five_steps_per_observation(self):
        b = self.brain()
        b.step_image(self.frame(96))
        self.assertEqual(b.ticks, 5)


if __name__ == "__main__":
    unittest.main()
