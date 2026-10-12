#!/usr/bin/env python3
"""Cross-check the terrain world scale with tree instances (.trins), independent of the descriptor.

Needs the dumps made with `.tools/gfdump` of archive/field/terrain/ha_areaNN.gfpak under
work/dump/field/terrain/ and the heightmaps decoded with `.tools/bntxdump` under work/dump/hm/.
For every area it resolves the `.trins` entries by path hash (see .tools/gfpack_hash.py), extracts the 4x4
instance matrices (row 3 = translation, row-vector convention) and compares the instance y with the
heightmap height (normalised 0..1 times the per-area height H) at (x, z) for two assumed extents
(1025 and 2050). Result on 2026-10-12: extent 1025 gives median |dy| 0.00-0.01 m for all instances of
areas 00-05 (6,211 trees), extent 2050 gives 21-112 m. Paths are hard-coded to the orchestrator dump layout.
"""
import struct,re,csv,array,subprocess,os
M=(1<<64)-1
def fnv(b):
    h=0xcbf29ce484222645
    for c in b: h=((h^c)*0x100000001b3)&M
    return h
R='/home/nico/work/decompilacion/work/dump/field/terrain/'
HMD='/home/nico/work/decompilacion/work/dump/hm/'
Hh={'00':185.0,'01':121.75599670410156,'02':200.23989868164062,'03':127.07900238037109,'04':302.0,'05':168.0,'06':36.66999816894531}
for a in ['00','01','02','03','04','05','06']:
    pack=f'ha_area{a}'; H={int(r['hash'],16):int(r['i']) for r in csv.DictReader(open(R+pack+'/index.tsv'),delimiter='\t')}
    pre=f'bin/field/terrain/{pack}/terrain/'
    # heightmap entry via hash
    hi=H.get(fnv((pre+f'area{a}_0_0_heightmap.bntx').encode()))
    f32=HMD+f'{pack}_area{a}_0_0_heightmap.f32'
    if hi is None: print(a,'heightmap entry not found by hash'); continue
    if not os.path.exists(f32):
        subprocess.run(['/tmp/claude-1000/-home-nico-work-decompilacion/db782c41-74ea-42cf-9f8c-f64f4eb67428/scratchpad/bntxdump/target/release/bntxdump',R+pack+f'/{hi:04d}.bin',HMD+pack],capture_output=True)
        f32=HMD+f'{pack}_area{a}_0_0_heightmap.f32'
    hm=array.array('f'); hm.frombytes(open(f32,'rb').read()); N=int(round(len(hm)**0.5))
    # descriptor = entry that holds 'detailmap'
    desc=None
    for i in sorted(set(H.values())):
        d=open(R+pack+f'/{i:04d}.bin','rb').read()
        if len(d)<200000 and d[:4]!=b'BNTX' and b'.trins' in d and b'detailmap' in d: desc=d;break
    names=sorted({m.group(0).decode() for m in re.finditer(rb'[A-Za-z0-9_\-]{3,90}\.trins',desc)}) if desc else []
    inst=[]
    for n in names:
        i=H.get(fnv((pre+n).encode()))
        if i is None: continue
        d=open(R+pack+f'/{i:04d}.bin','rb').read()
        for off in range(0,len(d)-63,4):
            f=struct.unpack_from('<16f',d,off)
            if f[15]==1.0 and f[3]==0.0 and f[7]==0.0 and f[11]==0.0 and f[13]>0 and abs(f[5])>0.1 and 0<=f[12]<=2100 and 0<=f[14]<=2100:
                inst.append((f[12],f[13],f[14]))
    def sample(u,v):
        u=min(max(u,0),N-1.001); v=min(max(v,0),N-1.001); x0=int(u); y0=int(v); fx=u-x0; fy=v-y0
        g=lambda x,y:hm[y*N+x]
        return g(x0,y0)*(1-fx)*(1-fy)+g(x0+1,y0)*fx*(1-fy)+g(x0,y0+1)*(1-fx)*fy+g(x0+1,y0+1)*fx*fy
    out=[]
    for E in (1025.0,2050.0):
        errs=sorted(abs(sample(x*(N-1)/E,z*(N-1)/E)*Hh[a]-y) for x,y,z in inst if x<=E and z<=E)
        if errs: out.append(f'extent {E:.0f}: n={len(errs)} median|dy|={errs[len(errs)//2]:.2f} within2m={sum(e<2 for e in errs)}/{len(errs)}')
    print(a,f'samples={N}','instances',len(inst),'|',' | '.join(out))
