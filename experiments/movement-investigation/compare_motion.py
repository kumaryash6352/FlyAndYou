"""Small held-out motion check using unchanged world_core physics and five neural ticks.
Scene coordinates remain in this harness. Controllers receive RGB only.
"""
import json
from pathlib import Path
import numpy as np
from candidate import Candidate, OUT
from measure_current import World

def setup(w,scene):
    heading=-1 if scene.endswith('_left') else 1
    state=w.call(kind='reset',x=340,facing=heading,bridge=True)
    if scene.startswith('red_wall'):
        wx=340+heading*40
        w.call(kind='edit',tool='solid',points=[[wx,216],[wx,276]],radius=4)
        state=w.call(kind='edit',tool='ink',points=[[wx,216],[wx,276]],radius=12,color=[195,80,57])
    elif scene.startswith('yellow_floor'):
        state=w.call(kind='edit',tool='ink',points=[[340+heading*16,280],[340+heading*64,280]],radius=8,color=[232,186,60])
    return heading,state

def main():
    c=Candidate();w=World();trials=[]
    try:
        for scene in ['red_wall_right','red_wall_left','yellow_floor_right','yellow_floor_left']:
            for method in ['current','candidate']:
                for seed in [101,202]:
                    heading,state=setup(w,scene);c.reset(seed,heading);c.b.heading=heading
                    log=[]
                    for i in range(12):
                        frame=np.array(state['rgb'],np.uint8).reshape(96,128,3)
                        a=c.b.step_image(frame) if method=='current' else c.step(frame)
                        state=w.call(kind='step',steer=a['steer'])
                        log.append({**a,**{k:state[k] for k in ['x','y','vx','facing','tick','outcome']}})
                    t=dict(scene=scene,method=method,seed=seed,displacement=state['x']-340,trace=log)
                    trials.append(t)
                    print(scene,method,seed,round(t['displacement'],2),'vx',round(state['vx'],2),'turns',sum(log[i]['facing']!=log[i-1]['facing'] for i in range(1,len(log))),flush=True)
        controls={}
        for mode in ['disconnected','zero_synapses']:
            traces=[]
            if mode=='zero_synapses':
                weights=c.b.w;c.b.w=c.b.w.copy();c.b.w.data.fill(0)
            for name in ['wall_red','wall_yellow']:
                c.reset(303);c.b.connected=mode!='disconnected'
                frame=np.frombuffer((OUT/(name+'.rgb')).read_bytes(),np.uint8).reshape(96,128,3)
                traces.append([c.step(frame) for _ in range(12)])
            controls[mode]=dict(actions_identical=traces[0]==traces[1],traces=traces)
            if mode=='zero_synapses':c.b.w=weights
            print(mode,controls[mode]['actions_identical'],flush=True)
        (OUT/'motion.json').write_text(json.dumps(dict(trials=trials,controls=controls,limits='16 short trials, two held-out RNG seeds, four controlled scene orientations. This is a headless check, not native feel or broad navigation validation.'),indent=2))
    finally:w.close()

if __name__=='__main__':main()
