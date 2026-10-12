#!/usr/bin/env python3
"""Decode a .trlgt (FlatBuffers) light/environment parameter file (recovered schema, see reports/data-progress/lighting.md).

root table: f0 = vector of Param tables (env params), f1 = vector of Light tables (probes / point lights).
Param: f0 group string, f1 name string, f2 = opaque (8..12 bytes, unknown), f3.. = 4 slots x 4 floats (slot s, comp c -> field 3+4*s+c; absent = 0).
"""
import struct, sys

def u32(b,o): return struct.unpack_from("<I",b,o)[0]
def i32(b,o): return struct.unpack_from("<i",b,o)[0]
def u16(b,o): return struct.unpack_from("<H",b,o)[0]
def f32(b,o): return struct.unpack_from("<f",b,o)[0]

def fields(b,t):
    v=t-i32(b,t); vs=u16(b,v)
    return {i:t+u16(b,v+4+2*i) for i in range((vs-4)//2) if u16(b,v+4+2*i)}

def s(b,o):
    p=o+u32(b,o); n=u32(b,p); return b[p+4:p+4+n].decode()

def vec(b,o):
    p=o+u32(b,o); n=u32(b,p)
    return [p+4+4*k+u32(b,p+4+4*k) for k in range(n)]

def decode(b):
    root=u32(b,0); rf=fields(b,root)
    params=[]
    for t in vec(b,rf[0]) if 0 in rf else []:
        f=fields(b,t)
        slots=[[f32(b,f[3+4*k+c]) if (3+4*k+c) in f else 0.0 for c in range(4)] for k in range(4)]
        params.append({"group":s(b,f[0]),"name":s(b,f[1]),"slots":slots,"fields":sorted(f)})
    return params

if __name__=="__main__":
    for p in decode(open(sys.argv[1],"rb").read()):
        sl=p["slots"]
        same=all(x==sl[0] for x in sl)
        print(p["group"],p["name"],"ALL" if same else "VAR",[[round(x,4) for x in v] for v in (sl[:1] if same else sl)])
