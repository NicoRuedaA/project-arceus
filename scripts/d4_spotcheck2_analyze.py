#!/usr/bin/env python3
"""Aggregate blind-reader verdicts for the depth rounds 45-66 spot-check (real / decoy / held), with Clopper-Pearson CIs."""
import json,glob,sys
from collections import Counter,defaultdict
from math import comb
R='/home/nico/work/decompilacion/work/d4/spotcheck2/'
key=json.load(open(R+'key.json')); S=json.load(open(R+'sample.json')); live=json.load(open(R+'live.json'))
I=S['info']; LIB={'NET','HAVOK','WWISE','NN_SDK','LUA_CORE','OODLE'}
res={}
for k in sorted(glob.glob(R+'out*.json')):
    for o in json.load(open(k)): res[o['item']]=o
V=('correct','wrong_detail','unsupported','wrong')
def cdf(k,n,p): return sum(comb(n,i)*p**i*(1-p)**(n-i) for i in range(k+1))
def bis(f,t):
    lo,hi=0.0,1.0
    for _ in range(60):
        m=(lo+hi)/2
        if f(m)>t: lo=m
        else: hi=m
    return lo
def cp(k,n,a=0.05):
    hi=1 if k==n else bis(lambda p:cdf(k,n,p),a/2)
    lo=0 if k==0 else bis(lambda p:-(1-cdf(k-1,n,p)),-a/2)
    return lo,hi
def row(name,items):
    c=Counter(res[i]['verdict'] for i in items); n=len(items); bad=n-c['correct']
    lo,hi=cp(bad,n) if n else (0,0)
    print(f'{name:28s} n={n:3d} '+' '.join(f'{v}={c[v]}' for v in V)+f'  non-correct {bad}/{n}={bad/max(n,1):.1%} CI95 [{lo:.1%},{hi:.1%}]')
real=[i for i,k in key.items() if k['kind']=='real' and i in res]
print('answered',len(res),'of',len(key))
row('REAL all',real)
byr=defaultdict(list)
for i in real: byr[I[key[i]['fid']]['round']].append(i)
for r in sorted(byr): row(f'round r{r}',byr[r])
g=lambda f:I[key[f]['fid']]
row('library-owned',[i for i in real if g(i)['owner'] in LIB])
for fam in sorted({g(i)['owner'] for i in real}): row(' family '+fam,[i for i in real if g(i)['owner']==fam])
row('PLT stubs',[i for i in real if g(i)['plt']])
row('>=5 instructions',[i for i in real if g(i)['ins']>=5])
row('<5 instructions',[i for i in real if g(i)['ins']<5])
row('conf medium',[i for i in real if g(i)['conf']=='medium'])
row('c1 rounds 45-51',[i for i in real if g(i)['round']<=51]); row('c2 rounds 60-66',[i for i in real if g(i)['round']>=60])
dec=[i for i,k in key.items() if k['kind']=='decoy' and i in res]
cd=Counter(res[i]['verdict'] for i in dec); print('decoys',len(dec),dict(cd),'detected',len(dec)-cd['correct'])
for i in dec:
    if res[i]['verdict']=='correct': print('  decoy missed',i,key[i]['fid'],len(live[key[i]['fid']]['ins']),'ins')
hold=[i for i,k in key.items() if k['kind']=='held' and i in res]; row('HELD',hold)
print('non-correct real items:')
for i in real:
    if res[i]['verdict']!='correct': print(' ',i,key[i]['fid'],'r%d'%g(i)['round'],g(i)['owner'],g(i)['ins'],'ins',res[i]['verdict'],'|',res[i].get('issue','')[:160])
print('non-correct held items:')
for i in hold:
    if res[i]['verdict']!='correct': print(' ',i,key[i]['fid'],res[i]['verdict'],'|',res[i].get('issue','')[:160])
