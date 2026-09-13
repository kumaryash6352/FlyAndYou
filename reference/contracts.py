"""Small, independently authored specifications for the implementation guide.

These are reference checks, not a platformer, neural simulator, or complete IPC
client/server. Python 3.12+ and the standard library are sufficient.
"""
from __future__ import annotations

import base64
import binascii
import hashlib
import json
import math
import re
import struct
from collections.abc import Mapping, Sequence
from typing import BinaryIO

MAX_MESSAGE = 1_048_576
SENSOR_WIDTH, SENSOR_HEIGHT = 128, 96
STEP_FIELDS = {
    'kind','version','epoch','step_id','physics_tick','world_revision',
    'profile_sha256','width','height','format','neural_steps',
    'rgb_sha256','frame_b64',
}
ACTION_FIELDS = {
    'kind','version','epoch','step_id','physics_tick','world_revision',
    'profile_sha256','rgb_sha256','neural_steps_done','action',
}
IDENTITY_FIELDS = ('version','epoch','step_id','physics_tick','world_revision',
                   'profile_sha256','rgb_sha256')


def _integer(value: object, lo: int, hi: int, name: str) -> int:
    if type(value) is not int or not lo <= value <= hi:
        raise ValueError(f'{name} must be an integer in [{lo}, {hi}]')
    return value


def _hex(value: object, digits: int, name: str) -> str:
    if not isinstance(value,str) or re.fullmatch('[0-9a-f]{'+str(digits)+'}',value) is None:
        raise ValueError(f'{name} must be {digits} lowercase hex characters')
    return value


def capsule_cells(points: Sequence[tuple[int,int]], radius_q: int,
                  width: int, height: int, cell_q: int) -> set[tuple[int,int]]:
    """Union of polyline capsules sampled at cell centers. Coordinates are
    integers in 1/256-pixel units. No event-rate-dependent circle stamping.
    This reference favors clarity; the engine should also track dirty bounds.
    """
    _integer(width,1,2048,'width'); _integer(height,1,2048,'height')
    _integer(radius_q,1,1<<30,'radius_q'); _integer(cell_q,2,1<<30,'cell_q')
    if cell_q % 2 or not 1 <= len(points) <= 4096:
        raise ValueError('even cell size and 1..4096 polyline points required')
    for point in points:
        if len(point)!=2: raise ValueError('point must contain x and y')
        for value in point: _integer(value,-(1<<30),1<<30,'coordinate')
    segments=list(zip(points,points[1:])) or [(points[0],points[0])]
    result:set[tuple[int,int]]=set()
    r2=radius_q*radius_q
    half=cell_q//2
    for (ax,ay),(bx,by) in segments:
        minx=max(0,(min(ax,bx)-radius_q-half)//cell_q)
        maxx=min(width-1,(max(ax,bx)+radius_q-half)//cell_q)
        miny=max(0,(min(ay,by)-radius_q-half)//cell_q)
        maxy=min(height-1,(max(ay,by)+radius_q-half)//cell_q)
        dx,dy=bx-ax,by-ay
        length2=dx*dx+dy*dy
        for y in range(miny,maxy+1):
            for x in range(minx,maxx+1):
                px,py=x*cell_q+half,y*cell_q+half
                ux,uy=px-ax,py-ay
                projection=ux*dx+uy*dy
                if length2==0 or projection<=0:
                    inside=ux*ux+uy*uy<=r2
                elif projection>=length2:
                    inside=(px-bx)**2+(py-by)**2<=r2
                else:
                    cross=ux*dy-uy*dx
                    inside=cross*cross<=r2*length2
                if inside: result.add((x,y))
    return result


def apply_delta(base: bytes, paint: bytes, updates: Mapping[int,int],
                protected: set[int], actor_guard: set[int]) -> bytes:
    """Validate a complete solid-layer transaction before changing anything.
    Immutable base cells are ignored. Protected mutable cells reject changes.
    The actor guard forbids adding solids, not erasing supporting terrain.
    """
    if len(base)!=len(paint) or not base:
        raise ValueError('masks must have equal nonzero size')
    if any(x not in (0,1) for x in base+paint):
        raise ValueError('masks must be binary')
    planned=[]
    for index,value in updates.items():
        _integer(index,0,len(base)-1,'cell index')
        _integer(value,0,1,'cell value')
        if base[index] or paint[index]==value: continue
        if index in protected: raise ValueError('protected region')
        if value and index in actor_guard: raise ValueError('solid intersects actor guard')
        planned.append((index,value))
    result=bytearray(paint)
    for index,value in planned: result[index]=value
    return bytes(result)


def crop_rgb(world: bytes, width: int, height: int, center: tuple[int,int],
             out_width: int, out_height: int, scale: int=2,
             padding: tuple[int,int,int]=(16,18,22)) -> bytes:
    """Egocentric cutaway image, no HUD argument and no semantic object input.
    Each output pixel is an integer box average of scale x scale RGB8 pixels.
    The engine supplies a canonical actor-free RGB world composition.
    """
    for key,value in [('width',width),('height',height),('out_width',out_width),
                      ('out_height',out_height),('scale',scale)]:
        _integer(value,1,4096,key)
    if len(world)!=width*height*3 or len(center)!=2 or len(padding)!=3:
        raise ValueError('bad RGB frame, center, or padding')
    for value in center: _integer(value,-(1<<30),1<<30,'center')
    for value in padding: _integer(value,0,255,'padding')
    x0=center[0]-(out_width*scale)//2
    y0=center[1]-(out_height*scale)//2
    result=bytearray(out_width*out_height*3)
    for oy in range(out_height):
        for ox in range(out_width):
            total=[0,0,0]
            for dy in range(scale):
                for dx in range(scale):
                    x,y=x0+ox*scale+dx,y0+oy*scale+dy
                    pixel=world[(y*width+x)*3:(y*width+x)*3+3] if 0<=x<width and 0<=y<height else padding
                    for channel in range(3): total[channel]+=pixel[channel]
            at=(oy*out_width+ox)*3
            result[at:at+3]=bytes(value//(scale*scale) for value in total)
    return bytes(result)


def encode_packet(message: dict) -> bytes:
    if type(message) is not dict: raise ValueError('message must be an object')
    body=json.dumps(message,allow_nan=False,separators=(',',':'),sort_keys=True).encode('utf-8')
    if not 1<=len(body)<=MAX_MESSAGE: raise ValueError('message exceeds framing limit')
    return struct.pack('!I',len(body))+body


def _read_exact(stream: BinaryIO, length: int) -> bytes:
    parts=[]
    remaining=length
    while remaining:
        part=stream.read(remaining)
        if not part: raise EOFError('incomplete framed message')
        parts.append(part); remaining-=len(part)
    return b''.join(parts)


def _unique_object(pairs):
    result={}
    for key,value in pairs:
        if key in result: raise ValueError('duplicate JSON key')
        result[key]=value
    return result


def _invalid_constant(value):
    raise ValueError(f'nonstandard JSON constant: {value}')


def read_packet(stream: BinaryIO) -> dict:
    length=struct.unpack('!I',_read_exact(stream,4))[0]
    if not 1<=length<=MAX_MESSAGE: raise ValueError('invalid frame length')
    message=json.loads(_read_exact(stream,length).decode('utf-8'),
                       object_pairs_hook=_unique_object,parse_constant=_invalid_constant)
    if type(message) is not dict: raise ValueError('message must be an object')
    return message


def _validate_identity(message: dict) -> None:
    _integer(message['version'],1,1,'version')
    _hex(message['epoch'],32,'epoch')
    _hex(message['profile_sha256'],64,'profile_sha256')
    _hex(message['rgb_sha256'],64,'rgb_sha256')
    for field in ('step_id','physics_tick','world_revision'):
        _integer(message[field],0,(1<<63)-1,field)
    if message['physics_tick'] % 10:
        raise ValueError('observation must be at a 10-tick decision boundary')


def validate_step(message: dict) -> bytes:
    if type(message) is not dict or set(message)!=STEP_FIELDS:
        raise ValueError('unexpected or missing step fields')
    if message['kind']!='step': raise ValueError('expected step')
    _validate_identity(message)
    _integer(message['width'],SENSOR_WIDTH,SENSOR_WIDTH,'width')
    _integer(message['height'],SENSOR_HEIGHT,SENSOR_HEIGHT,'height')
    _integer(message['neural_steps'],5,5,'neural_steps')
    if message['format']!='rgb8' or type(message['frame_b64']) is not str:
        raise ValueError('expected base64 RGB8')
    if len(message['frame_b64'])!=4*((SENSOR_WIDTH*SENSOR_HEIGHT*3+2)//3):
        raise ValueError('wrong encoded frame length')
    try: frame=base64.b64decode(message['frame_b64'],validate=True)
    except (ValueError,binascii.Error) as error: raise ValueError('invalid base64') from error
    if len(frame)!=SENSOR_WIDTH*SENSOR_HEIGHT*3:
        raise ValueError('wrong decoded frame length')
    if hashlib.sha256(frame).hexdigest()!=message['rgb_sha256']:
        raise ValueError('frame hash mismatch')
    return frame


def validate_action(message: dict, expected_step: dict) -> tuple[float,bool]:
    validate_step(expected_step)
    if type(message) is not dict or set(message)!=ACTION_FIELDS or message['kind']!='action':
        raise ValueError('unexpected or missing action fields')
    _validate_identity(message)
    if any(message[k]!=expected_step[k] for k in IDENTITY_FIELDS):
        raise ValueError('action does not match outstanding observation')
    _integer(message['neural_steps_done'],5,5,'neural_steps_done')
    action=message['action']
    if type(action) is not dict or set(action)!={'steer','jump'}:
        raise ValueError('invalid action object')
    steer=action['steer']
    if type(steer) not in (int,float) or not math.isfinite(steer) or not -1<=steer<=1:
        raise ValueError('invalid steering value')
    if type(action['jump']) is not bool: raise ValueError('jump must be boolean')
    return float(steer),action['jump']


def wilson(successes: int, trials: int, z: float=1.959963984540054) -> tuple[float,float]:
    """Two-sided Wilson interval for a binomial proportion. Not evidence that
    correlated game runs are independent or that the biological model is valid.
    """
    _integer(trials,1,(1<<31)-1,'trials')
    _integer(successes,0,trials,'successes')
    if not math.isfinite(z) or z<=0: raise ValueError('z must be positive and finite')
    p=successes/trials
    denominator=1+z*z/trials
    center=(p+z*z/(2*trials))/denominator
    radius=z*math.sqrt(p*(1-p)/trials+z*z/(4*trials*trials))/denominator
    return max(0,center-radius),min(1,center+radius)
