"""Executable guide examples, not tests of an implemented game or fly model."""
import base64
import io
import json
import struct
import unittest
from copy import deepcopy
import contracts as c

class ShortReader(io.BytesIO):
    def read(self, n=-1):
        return super().read(min(n, 3) if n >= 0 else 3)

class ContractsTest(unittest.TestCase):
    def request(self):
        frame = bytes([19]) * (128 * 96 * 3)
        import hashlib
        return dict(kind='step', version=1, epoch='a'*32, step_id=0,
                    physics_tick=0, world_revision=0, profile_sha256='b'*64,
                    width=128, height=96, format='rgb8', neural_steps=5,
                    rgb_sha256=hashlib.sha256(frame).hexdigest(),
                    frame_b64=base64.b64encode(frame).decode('ascii'))

    def reply(self, req):
        keys = ('version', 'epoch', 'step_id', 'physics_tick', 'world_revision',
                'profile_sha256', 'rgb_sha256')
        return dict(kind='action', **{k:req[k] for k in keys},
                    neural_steps_done=5, action={'steer':0.25, 'jump':False})

    def test_reference_api_exists(self):
        names = ('capsule_cells', 'apply_delta', 'crop_rgb', 'encode_packet',
                 'read_packet', 'validate_step', 'validate_action', 'wilson')
        self.assertTrue(all(callable(getattr(c, n, None)) for n in names))

    def test_capsule_continuous_horizontal(self):
        result = c.capsule_cells([(2*256,6*256),(30*256,6*256)], 2*256, 8, 4, 4*256)
        self.assertEqual(result, {(x,1) for x in range(8)})

    def test_collinear_resampling_invariant(self):
        a = c.capsule_cells([(512,1536),(7680,1536)],512,8,4,1024)
        b = c.capsule_cells([(512,1536),(4096,1536),(7680,1536)],512,8,4,1024)
        self.assertEqual(a,b)

    def test_single_point_is_disc(self):
        self.assertEqual(c.capsule_cells([(512,512)],256,2,2,1024), {(0,0)})

    def test_radius_must_be_positive(self):
        with self.assertRaises(ValueError):
            c.capsule_cells([(0,0)],0,2,2,1024)

    def test_atomic_reject_does_not_mutate_input(self):
        paint = bytes(4)
        with self.assertRaises(ValueError):
            c.apply_delta(bytes(4),paint,{0:1,3:1},protected={3},actor_guard=set())
        self.assertEqual(paint,bytes(4))

    def test_immutable_terrain_is_not_erased(self):
        result = c.apply_delta(bytes([1,0]),bytes(2),{0:0,1:1},set(),set())
        self.assertEqual(result,bytes([0,1]))

    def test_cannot_materialize_terrain_in_actor(self):
        with self.assertRaises(ValueError):
            c.apply_delta(bytes(4),bytes(4),{2:1},set(),{2})

    def test_erasing_support_is_allowed(self):
        self.assertEqual(c.apply_delta(bytes(4),bytes([0,0,1,0]),{2:0},set(),{2}),bytes(4))

    def test_undo_is_subject_to_the_same_guard(self):
        with self.assertRaises(ValueError):
            c.apply_delta(bytes(4),bytes(4),{2:1},set(),{2})

    def test_delta_outside_grid_rejected(self):
        with self.assertRaises(ValueError):
            c.apply_delta(bytes(4),bytes(4),{4:1},set(),set())

    def test_sensory_area_average(self):
        world=bytes([0,0,0, 40,40,40, 80,80,80, 120,120,120])
        self.assertEqual(c.crop_rgb(world,2,2,(1,1),1,1,2,(0,0,0)),bytes([60]*3))

    def test_sensory_crop_padding(self):
        self.assertEqual(c.crop_rgb(bytes([255]*12),2,2,(-20,-20),2,2,1,(7,8,9)),bytes([7,8,9]*4))

    def test_screen_state_cannot_change_sensor_bytes(self):
        world=bytes([10,20,30]*16)
        a=c.crop_rgb(world,4,4,(2,2),2,2,2,(0,0,0))
        editor={'zoom':8,'cursor':(99,1),'selected_tool':'eraser'}
        editor.update(zoom=2,cursor=(2,9),selected_tool='solid')
        b=c.crop_rgb(world,4,4,(2,2),2,2,2,(0,0,0))
        self.assertEqual(a,b)  # Tests this pure boundary, not a real UI integration.

    def test_framing_handles_short_reads(self):
        req=self.request()
        self.assertEqual(c.read_packet(ShortReader(c.encode_packet(req))),req)

    def test_framing_rejects_truncation(self):
        with self.assertRaises(EOFError):
            c.read_packet(io.BytesIO(c.encode_packet({'kind':'test'})[:-1]))

    def test_framing_rejects_oversize_before_read(self):
        with self.assertRaises(ValueError):
            c.read_packet(io.BytesIO(struct.pack('!I',c.MAX_MESSAGE+1)))

    def test_nan_not_encodable(self):
        with self.assertRaises(ValueError):
            c.encode_packet({'steer':float('nan')})

    def test_json_duplicate_keys_rejected(self):
        body=b'{"kind":"step","kind":"action"}'
        with self.assertRaises(ValueError):
            c.read_packet(io.BytesIO(struct.pack('!I',len(body))+body))

    def test_nonstandard_json_constants_rejected(self):
        body=b'{"steer":NaN}'
        with self.assertRaises(ValueError):
            c.read_packet(io.BytesIO(struct.pack('!I',len(body))+body))

    def test_step_validates_frame_hash_and_shape(self):
        req=self.request()
        self.assertEqual(len(c.validate_step(req)),128*96*3)
        req['frame_b64']=base64.b64encode(b'wrong').decode()
        with self.assertRaises(ValueError): c.validate_step(req)

    def test_controller_request_rejects_goal_leak(self):
        req=self.request(); req['goal_x']=500
        with self.assertRaises(ValueError): c.validate_step(req)

    def test_controller_request_rejects_boolean_tick(self):
        req=self.request(); req['physics_tick']=False
        with self.assertRaises(ValueError): c.validate_step(req)

    def test_action_accepts_only_matching_request(self):
        req=self.request(); reply=self.reply(req)
        self.assertEqual(c.validate_action(reply,req),(0.25,False))

    def test_old_epoch_reply_is_rejected(self):
        req=self.request(); reply=self.reply(req); reply['epoch']='c'*32
        with self.assertRaises(ValueError): c.validate_action(reply,req)

    def test_wrong_profile_reply_is_rejected(self):
        req=self.request(); reply=self.reply(req); reply['profile_sha256']='c'*64
        with self.assertRaises(ValueError): c.validate_action(reply,req)

    def test_out_of_range_or_nonfinite_action_rejected(self):
        for bad in (1.1,float('nan'),float('inf'),True):
            req=self.request(); reply=self.reply(req); reply['action']['steer']=bad
            with self.assertRaises(ValueError): c.validate_action(reply,req)

    def test_action_id_is_not_boolean(self):
        req=self.request(); reply=self.reply(req); reply['step_id']=False
        with self.assertRaises(ValueError): c.validate_action(reply,req)

    def test_wrong_neural_step_count_rejected(self):
        req=self.request(); reply=self.reply(req); reply['neural_steps_done']=4
        with self.assertRaises(ValueError): c.validate_action(reply,req)

    def test_wilson_interval_is_not_a_learning_result(self):
        lo,hi=c.wilson(48,64)
        self.assertTrue(0.62 < lo < 0.64)
        self.assertTrue(0.83 < hi < 0.85)
        with self.assertRaises(ValueError): c.wilson(1,0)

if __name__=='__main__':
    unittest.main(verbosity=2)
