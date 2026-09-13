"""Check replacing an attractive cue with a repulsive wall while already moving."""
import json
import numpy as np
from candidate import Candidate,OUT
from measure_current import World

c=Candidate();w=World();trials=[]
try:
    for method in ['current','candidate']:
        c.reset(303)
        w.call(kind='reset',x=340,bridge=True)
        w.call(kind='edit',tool='solid',points=[[440,216],[440,276]],radius=4)
        state=w.call(kind='edit',tool='ink',points=[[340,280],[436,280]],radius=8,color=[232,186,60])
        trace=[]
        for i in range(18):
            if i==5:
                w.call(kind='edit',tool='erase_ink',points=[[340,280],[436,280]],radius=24)
                state=w.call(kind='edit',tool='ink',points=[[440,216],[440,276]],radius=12,color=[195,80,57])
            frame=np.array(state['rgb'],np.uint8).reshape(96,128,3)
            a=c.b.step_image(frame) if method=='current' else c.step(frame)
            before={k:state[k] for k in ['x','vx','facing']}
            state=w.call(kind='step',steer=a['steer'])
            trace.append(dict(decision=i+1,**a,before=before,after={k:state[k] for k in ['x','vx','facing']}))
        trials.append(dict(method=method,trace=trace))
        print(method,'at recolor',trace[5]['before'],'last',trace[-1]['after'],'first negative command',next((t['decision'] for t in trace if t['steer']<0),None),flush=True)
    (OUT/'transition.json').write_text(json.dumps(trials,indent=2))
finally:w.close()
