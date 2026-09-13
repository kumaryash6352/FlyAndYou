"""Measure the existing step + inspect exchange in a separate real worker."""
import base64,hashlib,json,socket,subprocess,time
from pathlib import Path
import numpy as np
from reference.contracts import encode_packet,read_packet,validate_action

out=Path('runs/candle-evaluation')
latencies=[]; compute=[]; overhead=[]
def stats(x):
    return dict(n=len(x),median_ms=float(np.median(x)),p95_ms=float(np.percentile(x,95)),min_ms=float(np.min(x)),max_ms=float(np.max(x)))
with (out/'socket-worker.log').open('w') as err:
    p=subprocess.Popen(['.venv/bin/python','-m','controller.brainworker.service','--runs','runs/candle-evaluation/socket'],stdout=subprocess.PIPE,stderr=err,text=True)
    try:
        port=json.loads(p.stdout.readline())['port']
        with socket.create_connection(('127.0.0.1',port),timeout=30) as sock:
            sock.setsockopt(socket.IPPROTO_TCP,socket.TCP_NODELAY,1)
            stream=sock.makefile('rb'); loaded=read_packet(stream)
            def call(r):
                sock.sendall(encode_packet(r)); return read_packet(stream)
            epoch='9'*32
            assert call(dict(kind='restore',epoch=epoch,checkpoint='initial'))['kind']=='restored'
            for i in range(45):
                color=((100,100,100),(232,186,60),(195,80,57))[(i//10)%3]
                rgb=bytes(color)*(128*96)
                r=dict(kind='step',version=1,epoch=epoch,step_id=i,physics_tick=i*10,world_revision=1,profile_sha256=loaded['profile_sha256'],rgb_sha256=hashlib.sha256(rgb).hexdigest(),width=128,height=96,format='rgb8',neural_steps=5,frame_b64=base64.b64encode(rgb).decode())
                start=time.perf_counter()
                action=call(r); validate_action(action,r)
                inspect=call(dict(kind='inspect',epoch=epoch,step_id=i))
                elapsed=(time.perf_counter()-start)*1000
                assert inspect['telemetry']['ticks']==(i+1)*5
                if i>=5:
                    latencies.append(elapsed); compute.append(inspect['elapsed_ms']); overhead.append(elapsed-inspect['elapsed_ms'])
            assert call(dict(kind='shutdown',epoch=epoch))['kind']=='shutdown'
        assert p.wait(timeout=10)==0
    finally:
        if p.poll() is None: p.kill(); p.wait()
        p.stdout.close()
result=dict(scope='40 measured requests after 5 warmup; existing separate Python worker; step + inspect, including validation and JSON/TCP, excluding image generation',step_and_inspect=stats(latencies),worker_compute=stats(compute),paired_non_compute_overhead=stats(overhead))
(out/'socket.json').write_text(json.dumps(result,indent=2))
print(json.dumps(result,indent=2))
