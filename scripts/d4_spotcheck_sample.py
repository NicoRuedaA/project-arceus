#!/usr/bin/env python3
"""Stratified random sample for the D4 mechanical-upgrade quality spot-check. Fixed seed."""
import json,random,glob,os
SEED=20261020
R='/home/nico/work/decompilacion/work/d4/'
rnd=random.Random(SEED)
acc={}  # fid -> (stratum, text)
p=json.load(open(R+'pilot500/final.json'))['accepted']
for f,t in p.items(): acc[f]=('pilot',t)
for base in ('mech-rounds','mech-b'):
    for d in sorted(glob.glob(R+base+'/r*/final.json')):
        n=int(os.path.basename(os.path.dirname(d))[1:])
        for f,t in json.load(open(d))['accepted'].items(): acc[f]=(f'r{n:02d}',t)
strata={}
for f,(s,t) in acc.items(): strata.setdefault(s,[]).append(f)
sample=[];used=set()
for s in sorted(strata):
    k=30 if s=='pilot' else 6
    for f in rnd.sample(sorted(strata[s]),k): sample.append((f,s)); used.add(f)
spare=[]
rest=sorted(f for f in acc if f not in used)
for f in rnd.sample(rest,6): spare.append((f,acc[f][0])); used.add(f)
ctrl=[]
rest=sorted(f for f in acc if f not in used)
for f in rnd.sample(rest,30): ctrl.append((f,acc[f][0])); used.add(f)
json.dump({'seed':SEED,'counts':{s:len(v) for s,v in strata.items()},
 'sample':sample,'spare':spare,'control':ctrl},open(R+'spotcheck/sample.json','w'))
json.dump({f:acc[f][1] for f in used},open(R+'spotcheck/texts.json','w'))
print(len(sample),len(spare),len(ctrl),sum(len(v) for v in strata.values()))
