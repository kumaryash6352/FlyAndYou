import base64
import hashlib
import tempfile
import unittest
from pathlib import Path

from scipy import sparse

from controller.brainworker.model import Brain
from controller.brainworker.service import Session


class SessionTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        w = sparse.csr_matrix(([1.0, 1.0], ([2, 3], [0, 1])), shape=(4, 4))
        b = Brain(w, [0, 1], [[32, 48], [96, 48]], [2], [3])
        self.s = Session(b, "a" * 64, Path(self.tmp.name))

    def tearDown(self):
        self.tmp.cleanup()

    def req(self):
        rgb = bytes(128 * 96 * 3)
        return dict(
            kind="step",
            version=1,
            epoch="0" * 32,
            step_id=0,
            physics_tick=0,
            world_revision=0,
            profile_sha256="a" * 64,
            rgb_sha256=hashlib.sha256(rgb).hexdigest(),
            width=128,
            height=96,
            format="rgb8",
            neural_steps=5,
            frame_b64=base64.b64encode(rgb).decode(),
        )

    def test_duplicate_returns_cached_reply_without_advancing(self):
        r = self.req()
        a = self.s.handle(r)
        s = self.s.brain.snapshot()
        self.assertEqual(a, self.s.handle(r))
        self.assertEqual(s, self.s.brain.snapshot())

    def test_same_id_different_content_is_fault(self):
        r = self.req()
        self.s.handle(r)
        r["world_revision"] = 1
        with self.assertRaises(ValueError):
            self.s.handle(r)

    def test_reset_rejects_old_epoch(self):
        r = self.req()
        self.s.handle(r)
        self.s.handle(dict(kind="restore", epoch="1" * 32, checkpoint="initial"))
        with self.assertRaises(ValueError):
            self.s.handle(r)

    def test_snapshot_restore_preserves_model_and_sequence(self):
        r = self.req()
        self.s.handle(r)
        state = self.s.brain.snapshot()
        self.s.handle(dict(kind="snapshot", epoch="0" * 32, name="test"))
        r["step_id"] = 1
        r["physics_tick"] = 10
        self.s.handle(r)
        self.s.handle(dict(kind="restore", epoch="2" * 32, checkpoint="test"))
        self.assertEqual(state, self.s.brain.snapshot())
        self.assertEqual(self.s.next_id, 1)

    def test_snapshot_path_cannot_escape_runs(self):
        self.s.handle(self.req())
        with self.assertRaises(ValueError):
            self.s.handle(dict(kind="snapshot", epoch="0" * 32, name="../bad"))


if __name__ == "__main__":
    unittest.main()
