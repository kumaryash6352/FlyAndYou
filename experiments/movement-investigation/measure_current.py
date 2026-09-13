"""Full-graph investigation. Fixed rendered observations, matched model/RNG starts."""
import hashlib
import json
import subprocess
import time
from pathlib import Path
import numpy as np
import pyarrow.feather as feather
from controller.brainworker.model import Brain

ROOT = Path(__file__).resolve().parents[2]
OUT = Path(__file__).resolve().parent

class World:
    def __init__(self):
        self.p = subprocess.Popen([str(OUT/'probe_world')], stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True)
    def call(self, **r):
        self.p.stdin.write(json.dumps(r)+'\n'); self.p.stdin.flush()
        return json.loads(self.p.stdout.readline())
    def close(self):
        self.p.stdin.close(); self.p.wait(); self.p.stdout.close()

def frames():
    w = World(); result = {}
    try:
        for label,color in [('gray',[160,160,160]),('red',[195,80,57]),('yellow',[232,186,60])]:
            w.call(kind='reset',x=340)
            w.call(kind='edit',tool='solid',points=[[380,216],[380,276]],radius=4)
            state = w.call(kind='edit',tool='ink',points=[[380,216],[380,276]],radius=12,color=color)
            result['wall_'+label] = np.array(state['rgb'],np.uint8).reshape(96,128,3)
        for label,color in [('red',[195,80,57]),('yellow',[232,186,60])]:
            w.call(kind='reset',x=320)
            state=w.call(kind='edit',tool='ink',points=[[336,280],[400,280]],radius=8,color=color)
            result['floor_'+label] = np.array(state['rgb'],np.uint8).reshape(96,128,3)
        result['full_red']=np.tile(np.array([195,80,57],np.uint8),(96,128,1))
        result['full_yellow']=np.tile(np.array([232,186,60],np.uint8),(96,128,1))
    finally: w.close()
    for name,a in result.items():
        (OUT/(name+'.rgb')).write_bytes(a.tobytes())
        (OUT/(name+'.ppm')).write_bytes(b'P6\n128 96\n255\n'+a.tobytes())
    return result

def main():
    cases=frames()
    b=Brain.load(ROOT/'data/cache/malecns-v1')
    base=b.snapshot()
    rows=feather.read_table(ROOT/'data/raw/annotations.feather',columns=['bodyId','status','superclass','type','somaSide','rootSide']).to_pylist()
    kept=sorted((r for r in rows if r['status']=='Traced' and r['superclass']),key=lambda r:r['bodyId'])
    groups={name:np.array([i for i,r in enumerate(kept) if r['type']==name],np.int32) for name in ['DNa02','DNa01','DNp09','MDN','PFL3']}
    logs={}; final={}; output_rates={}
    for name,frame in cases.items():
        b.restore(base); trace=[]; rates=[]
        start=time.perf_counter()
        for i in range(40):
            action=b.step_image(frame)
            vp=float((b.activity[b.left].mean()+b.activity[b.right].mean())*.5)
            trace.append(dict(decision=i+1, steer=action['steer'], input_mean=float(b.previous.mean()), vp_mean=vp, attraction=float(np.clip(b.gain*(vp-b.bias),0,1)), heading=b.heading, search_age=b.search_age, atlas_max=float(b.activity[b.anatomy_indices].max()), **{k:float(b.activity[ix].mean()) for k,ix in groups.items()}))
            rates.append(b.activity[np.r_[b.left,b.right]].copy())
        logs[name]=trace; final[name]=b.activity.copy(); output_rates[name]=np.stack(rates)
        print(name, json.dumps({**trace[-1], 'seconds':time.perf_counter()-start}),flush=True)
    np.savez_compressed(OUT/'current_final_activity.npz',**final)
    np.savez_compressed(OUT/'current_vp_activity.npz',**output_rates)
    report=dict(profile_sha256=hashlib.sha256((ROOT/'data/cache/malecns-v1/manifest.json').read_bytes()).hexdigest(),seed=7,neurons=b.w.shape[0],edges=b.w.nnz,cases=logs,limits='Static observations from real renderer plus synthetic maximum-drive controls. Same seed and initial state; no native usability conclusion.')
    (OUT/'current.json').write_text(json.dumps(report,indent=2))
    print('Saved',OUT/'current.json',flush=True)

if __name__=='__main__': main()
