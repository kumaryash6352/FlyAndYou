"""Build the investigation driver and derive channel connectivity, without changing game assets."""
from pathlib import Path
import hashlib
import json
import subprocess
import numpy as np
from controller.brainworker.model import Brain

root=Path(__file__).resolve().parents[2]
out=Path(__file__).resolve().parent
deps=root/'target/debug/deps'
args=['rustc','--edition=2024','-O',str(out/'probe_world.rs'),'-L',f'dependency={deps}','-o',str(out/'probe_world')]
for name in ['world_core','serde_json']:
    lib=max(deps.glob(f'lib{name}-*.rlib'),key=lambda p:p.stat().st_mtime)
    args.extend(['--extern',f'{name}={lib}'])
subprocess.run(args,check=True)
b=Brain.load(root/'data/cache/malecns-v1')
mask=np.arange(len(b.inputs))%2==0
wy=np.asarray(b.w[:,b.inputs[mask]].maximum(0).sum(axis=1)).ravel()
wr=np.asarray(b.w[:,b.inputs[~mask]].maximum(0).sum(axis=1)).ravel()
np.savez_compressed(out/'connectivity_channels.npz',yellow_mask=mask,yellow_weight=wy,red_weight=wr)
print('Built driver and derived connectivity channels in',out)
