import json
from pathlib import Path
import numpy as np
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt

p=Path(__file__).resolve().parent
current=json.loads((p/'current.json').read_text())['cases']
motion=json.loads((p/'motion.json').read_text())['trials']
fig=plt.figure(figsize=(12,7.6),facecolor='#faf9f4')
grid=fig.add_gridspec(2,6,height_ratios=[1,1.6],hspace=.42,wspace=.62)
for i,(case,title) in enumerate([('wall_gray','Neutral gray wall'),('wall_red','Red wall'),('wall_yellow','Yellow wall')]):
    ax=fig.add_subplot(grid[0,i*2:(i+1)*2])
    frame=np.frombuffer((p/(case+'.rgb')).read_bytes(),np.uint8).reshape(96,128,3)
    ax.imshow(frame,interpolation='nearest');ax.set_axis_off();ax.set_title(title,fontsize=12)
ax=fig.add_subplot(grid[1,:3])
for case,label,color in [('wall_gray','Neutral','#66777a'),('wall_red','Red','#c44732'),('wall_yellow','Yellow','#be8f12')]:
    data=current[case][:12]
    ax.plot(np.arange(1,13)*.1,[v['attraction'] for v in data],label=label,color=color,linewidth=2.5,linestyle='--' if case=='wall_yellow' else '-')
ax.set(title='Current readout treats both colors as attraction',xlabel='Simulated time (s)',ylabel='Attraction signal (0–1)',ylim=(-.04,1.12))
ax.legend(frameon=False,loc='lower right');ax.grid(alpha=.15)
ax=fig.add_subplot(grid[1,3:])
for method,label,color in [('current','Current: pushes into wall','#c44732'),('candidate','Prototype: retreats','#176a77')]:
    trial=next(t for t in motion if t['scene']=='red_wall_right' and t['method']==method and t['seed']==101)
    ax.plot(np.arange(13)*.1,[340]+[v['x'] for v in trial['trace']],label=label,color=color,linewidth=2.5)
ax.axhline(370,color='#4b4740',linestyle=':',linewidth=1.5,label='Collision limit for fly center')
ax.set(title='Red wall: actual world_core movement',xlabel='Simulated time (s)',ylabel='World x position (px)',ylim=(175,390))
ax.legend(frameon=False,loc='lower left',fontsize=9);ax.grid(alpha=.15)
for ax in fig.axes:
    for spine in ax.spines.values():spine.set_alpha(.2)
fig.suptitle('Why the glowing red wall does not steer the fly away',fontsize=17,x=.07,ha='left',y=.99)
fig.text(.07,.018,'Full 164,606-neuron graph · five neural steps / ten physics ticks unchanged · controlled headless scenes, not a native feel test',fontsize=9,color='#555')
fig.subplots_adjust(left=.07,right=.98,top=.90,bottom=.10)
fig.savefig(p/'findings.png',dpi=160,facecolor=fig.get_facecolor())
print(p/'findings.png')
