"""Investigation-only fixed chromatic readout; never imported by the game.

Two artificial channels partition the existing sensory population deterministically.
Readout neurons are non-input cells selected by existing synaptic connectivity.
No biological red/avoid or yellow/approach association is claimed.
"""
import json
from pathlib import Path
import numpy as np
from controller.brainworker.model import Brain

OUT=Path(__file__).resolve().parent
ROOT=OUT.parents[1]

class Candidate:
    def __init__(self, seed=7):
        self.b=Brain.load(ROOT/'data/cache/malecns-v1',seed=seed)
        channel=np.load(OUT/'connectivity_channels.npz')
        self.yellow_mask=channel['yellow_mask']
        wy,wr=channel['yellow_weight'],channel['red_weight']
        valid=np.ones(len(wy),bool);valid[self.b.inputs]=False
        self.y_ix=np.flatnonzero((wy>.05)&(wy/(wy+wr+1e-9)>.9)&valid)
        self.r_ix=np.flatnonzero((wr>.05)&(wr/(wy+wr+1e-9)>.9)&valid)
        self.initial=self.b.snapshot()
        # Neutral calibration is fixed and never updates from the active cue.
        neutral=np.full((96,128,3),160,np.uint8)
        baseline=[]
        for _ in range(10):
            self.encode_and_tick(neutral)
            baseline.append([self.b.activity[self.y_ix].mean(),self.b.activity[self.r_ix].mean()])
        self.baseline=np.mean(baseline[-5:],axis=0)
        self.reset(seed)

    def reset(self,seed=7,heading=1):
        self.b.restore(self.initial);self.b.rng=np.random.default_rng(seed)
        self.heading=heading;self.commit=0;self.armed=True;self.clear=0;self.search=0

    def encode_and_tick(self, frame):
        rgb=frame[self.b.samples[:,1],self.b.samples[:,0]].astype(np.float32)
        y=np.clip((np.minimum(rgb[:,0],rgb[:,1])-rgb[:,2]-24)/160,0,1)
        r=np.clip((rgb[:,0]-np.maximum(rgb[:,1],rgb[:,2])-48)/160,0,1)
        currents=np.where(self.yellow_mask,y,r).astype(np.float32)
        self.b.previous=currents.copy()
        for _ in range(5):self.b.tick(currents)

    def evidence(self):
        raw=np.array([self.b.activity[self.y_ix].mean(),self.b.activity[self.r_ix].mean()])
        # Expose raw excess rates for calibration before choosing a motor gain.
        return np.maximum(raw-self.baseline,0)

    def step(self,frame):
        self.encode_and_tick(frame)
        y,r=self.evidence()
        ys,rs=y/.025,r/.025
        threat=rs>.08 and rs>ys*1.25
        if rs<.03:
            self.clear+=1
            if self.clear>=3:self.armed=True
        else:self.clear=0
        if self.commit:self.commit-=1
        if threat and self.armed and self.commit==0:
            self.heading*=-1;self.commit=8;self.armed=False;self.search=0
        if self.commit:
            mode='escape';speed=.85
        elif threat and not self.armed:
            mode='hold';speed=0.0
        elif ys>.08 and ys>=rs:
            mode='approach';speed=.52+.48*ys/(ys+.25);self.search=0
        else:
            mode='search';speed=.52;self.search+=1
            if self.search>=35:
                self.heading*=-1;self.commit=4;self.search=0
        # Body acceleration already smooths movement; no second steer low-pass.
        return dict(steer=float(self.heading*speed),yellow=float(y),red=float(r),mode=mode,heading=self.heading)

def main():
    c=Candidate();print('relay populations',len(c.y_ix),len(c.r_ix),'baseline',c.baseline,flush=True)
    traces={}
    for name in ['wall_gray','wall_red','wall_yellow','floor_red','floor_yellow','full_red','full_yellow']:
        c.reset();frame=np.frombuffer((OUT/(name+'.rgb')).read_bytes(),np.uint8).reshape(96,128,3)
        traces[name]=[c.step(frame) for _ in range(12)]
        print(name,'first',traces[name][0],'last',traces[name][-1],flush=True)
    (OUT/'candidate_static.json').write_text(json.dumps(dict(populations=[len(c.y_ix),len(c.r_ix)],baseline=c.baseline.tolist(),cases=traces),indent=2))

if __name__=='__main__':main()
