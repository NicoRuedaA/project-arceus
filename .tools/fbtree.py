#!/usr/bin/env python3
"""Schema-free FlatBuffers tree dump (heuristic guesses). Usage: fbtree.py FILE [maxdepth]"""
import struct, sys

def u32(b,o): return struct.unpack_from("<I",b,o)[0]
def i32(b,o): return struct.unpack_from("<i",b,o)[0]
def u16(b,o): return struct.unpack_from("<H",b,o)[0]

def tfields(b,t):
    if t<0 or t+4>len(b): return None
    v=t-i32(b,t)
    if v<0 or v+4>len(b): return None
    vs=u16(b,v); ts=u16(b,v+2)
    if vs<4 or vs%2 or v+vs>len(b) or ts<4 or t+ts>len(b) or vs>400: return None
    return {i:t+u16(b,v+4+2*i) for i in range((vs-4)//2) if u16(b,v+4+2*i)}, ts

def isstr(b,p):
    if p<0 or p+4>len(b): return None
    n=u32(b,p)
    if 0<n<256 and p+5+n<=len(b) and b[p+4+n]==0 and all(32<=c<127 for c in b[p+4:p+4+n]): return b[p+4:p+4+n].decode()

def show(b,o,sz,depth,ind):
    # scalar fallback
    s=""
    if sz>=4 and sz%4==0 and sz<=64:
        s=" f32s="+",".join("%.4g"%x for x in struct.unpack_from("<%df"%(sz//4),b,o))
        s+=" u32="+",".join(str(x) for x in struct.unpack_from("<%dI"%(sz//4),b,o))[:60]
    elif sz==1: s=f" u8={b[o]}"
    elif sz==2: s=f" u16={u16(b,o)}"
    return s

def dump(b,t,depth,ind=""):
    r=tfields(b,t)
    if r is None: print(ind+"<bad table>"); return
    fl,ts=r
    ends=sorted(set(fl.values())|{t+ts})
    for i,o in sorted(fl.items()):
        nxt=min([x for x in ends if x>o]+[t+ts]); sz=nxt-o
        line=f"{ind}f{i} @{o} sz<={sz}"
        if sz>=4:
            p=o+u32(b,o)
            st=isstr(b,p) if p>o else None
            if st is not None: print(line+f" STR '{st}'"); continue
            if p>o and p+4<=len(b) and u32(b,o)>=4:
                n=u32(b,p)
                if 0<=n<100000 and sz<=4+0 or (sz==4 and 0<n<100000 and p+4+n<=len(b)):
                    # vector? check first element as offset to table
                    if depth>0 and p+8<=len(b) and n*4+p+4<=len(b):
                        e=p+4+u32(b,p+4)
                        if tfields(b,e) and n>0 and u32(b,p+4)>=4*n and u32(b,p+4)<len(b):
                            print(line+f" VEC[{n}] of tables"); 
                            for k in range(min(n,int(sys.argv[3]) if len(sys.argv)>3 else 3)):
                                print(f"{ind}  [{k}]"); dump(b,p+4+4*k+u32(b,p+4+4*k),depth-1,ind+"    ")
                            continue
                    if tfields(b,p) and depth>0 and sz==4:
                        print(line+" TABLE"); dump(b,p,depth-1,ind+"  "); continue
                    print(line+f" VEC? n={n} first={b[p+4:p+4+min(n*4,32)].hex()}"+show(b,o,min(sz,4),0,ind)); continue
        print(line+show(b,o,sz,0,ind))
if __name__=="__main__":
    b=open(sys.argv[1],"rb").read()
    dump(b,u32(b,0),int(sys.argv[2]) if len(sys.argv)>2 else 2)
