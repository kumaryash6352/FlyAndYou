"""Compare experimental recurrence outputs through the unchanged production motor."""
from pathlib import Path
import json, hashlib
import numpy as np
from controller.brainworker.model import Brain

out=Path('runs/candle-evaluation')
base=json.loads((out/'baseline.json').read_text())
b=Brain.load('data/cache/malecns-v1')
n=b.w.shape[0]
reference=np.fromfile(out/'reference.f32',dtype='<f4').reshape(-1,n)
def decode(states):
    b.restore(base['initial_snapshot'])
    actions=[]; modes=[]; evidence=[]; motor=[]
    for state in states:
        b.activity=state.copy()
        e=b.evidence(); evidence.append(e)
        actions.append(b.motor.step(*map(float,e),b.motor_config))
        modes.append(b.motor.mode)
        motor.append({k:v for k,v in b.motor.snapshot().items() if k!='steer'})
    return np.array(actions),modes,np.array(evidence),motor
ra,rm,re,rs=decode(reference)
assert np.array_equal(ra,base['actions'])
results={}
for mode in ('cpu1','cpu5','metal-serial','metal-simd'):
    p=out/f'{mode}-states.f32'
    states=np.fromfile(p,dtype='<f4').reshape(reference.shape)
    a,m,e,s=decode(states)
    result=dict(motor_modes_match=m==rm,motor_state_fields_match=s==rs,max_steer_difference=float(np.max(np.abs(a-ra))),max_evidence_difference=float(np.max(np.abs(e-re))),max_activity_difference=float(np.max(np.abs(states-reference))),states_sha256=hashlib.sha256(p.read_bytes()).hexdigest(),reference_modes=sorted(set(rm)))
    assert result['motor_modes_match'] and result['motor_state_fields_match']
    assert result['max_steer_difference']<1e-5 and result['max_activity_difference']<1e-5
    results[mode]=result
(out/'parity.json').write_text(json.dumps(results,indent=2))
print(json.dumps(results,indent=2))
