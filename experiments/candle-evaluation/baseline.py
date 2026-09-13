"""Throwaway full-graph timing/parity fixture. Does not modify the game/profile."""
from pathlib import Path
import json, time, hashlib, platform, sys
import numpy as np
import scipy
from controller.brainworker.model import Brain
from controller.brainworker.chromatic import encode

OUT = Path('runs/candle-evaluation')
OUT.mkdir(exist_ok=True)
t0 = time.perf_counter()
b = Brain.load('data/cache/malecns-v1')
load_ms = (time.perf_counter() - t0) * 1000
frames = [np.full((96,128,3), c, np.uint8) for c in ((100,100,100),(232,186,60),(195,80,57))]
for _ in range(8): b.step_image(frames[0])
initial = b.snapshot()
def stats(x):
    return dict(n=len(x), median_ms=float(np.median(x)), p95_ms=float(np.percentile(x,95)), min_ms=float(np.min(x)), max_ms=float(np.max(x)))
latencies=[]
for i in range(60):
    t=time.perf_counter(); b.step_image(frames[(i//10)%3]); latencies.append((time.perf_counter()-t)*1000)
# Per-operation profile of the actual arithmetic, separate from end-to-end timing.
b.restore(initial)
timings={k:[] for k in ('spmv','scale_input','rng','pointwise')}
current=encode(frames[1][b.samples[:,1], b.samples[:,0]], b.yellow_mask)
for _ in range(100):
    t=time.perf_counter(); drive=b.w.dot(b.activity); timings['spmv'].append((time.perf_counter()-t)*1000)
    t=time.perf_counter(); drive=b.recurrence*drive; drive[b.inputs]+=current; timings['scale_input'].append((time.perf_counter()-t)*1000)
    t=time.perf_counter(); drive+=b.rng.normal(0.,b.noise,len(drive)).astype(np.float32); timings['rng'].append((time.perf_counter()-t)*1000)
    t=time.perf_counter(); b.activity*=b.retention; b.activity+=(1.-b.retention)*np.tanh(np.maximum(drive,0)); timings['pointwise'].append((time.perf_counter()-t)*1000)
# Shared exact noise and encoded input lets the sparse backends be compared without RNG algorithm differences.
b.restore(initial)
b.w.data.astype('<f4').tofile(OUT/'values.f32')
b.w.indices.astype('<u4').tofile(OUT/'columns.u32')
b.w.indptr.astype('<u4').tofile(OUT/'rows.u32')
b.activity.astype('<f4').tofile(OUT/'initial.f32')
all_inputs=[]; all_noise=[]; all_states=[]; actions=[]
fixture_times=[]
for i in range(60):
    frame=frames[(i//10)%3]
    currents=encode(frame[b.samples[:,1],b.samples[:,0]], b.yellow_mask)
    injected=np.zeros(b.w.shape[0],np.float32); injected[b.inputs]=currents
    noises=np.stack([b.rng.normal(0.,b.noise,b.w.shape[0]).astype(np.float32) for _ in range(5)])
    t=time.perf_counter()
    for noise in noises:
        drive=b.recurrence*b.w.dot(b.activity)
        drive[b.inputs]+=currents
        drive+=noise
        b.activity*=b.retention
        b.activity+=(1.-b.retention)*np.tanh(np.maximum(drive,0))
        b.ticks+=1
    fixture_times.append((time.perf_counter()-t)*1000)
    actions.append(b.motor.step(*map(float,b.evidence()), b.motor_config))
    all_inputs.append(injected); all_noise.append(noises); all_states.append(b.activity.copy())
np.array(all_inputs,dtype='<f4').tofile(OUT/'inputs.f32')
np.array(all_noise,dtype='<f4').tofile(OUT/'noise.f32')
np.array(all_states,dtype='<f4').tofile(OUT/'reference.f32')
result=dict(kind='isolated feasibility measurement', python=sys.version, numpy=np.__version__, scipy=scipy.__version__, platform=platform.platform(), nodes=b.w.shape[0],edges=b.w.nnz,csr_bytes=b.w.data.nbytes+b.w.indices.nbytes+b.w.indptr.nbytes,dense_f32_bytes=b.w.shape[0]**2*4,load_ms=load_ms,brain_step_image=stats(latencies),tick_components={k:stats(v) for k,v in timings.items()},five_ticks_precomputed_noise=stats(fixture_times),row_degree_percentiles=np.percentile(np.diff(b.w.indptr),[0,50,90,95,99,100]).tolist(),initial_snapshot=initial,actions=actions,profile_sha256=hashlib.sha256(Path('data/cache/malecns-v1/manifest.json').read_bytes()).hexdigest())
# Keep large snapshot out of summary stdout; used to compare motor traces.
(OUT/'baseline.json').write_text(json.dumps(result,indent=2))
print(json.dumps({k:v for k,v in result.items() if k not in ('initial_snapshot','actions')},indent=2),flush=True)
