#!/usr/bin/env python3
import json,glob,re
from collections import Counter,defaultdict
from math import comb
R='/home/nico/work/decompilacion/work/d4/spotcheck/'
key=json.load(open(R+'key.json')); S=json.load(open(R+'sample.json')); T=json.load(open(R+'texts.json')); live=json.load(open(R+'live.json'))
strat={f:s for f,s in S['sample']}
res={}
for k in sorted(glob.glob(R+'out*.json')):
    for o in json.load(open(k)): res[o['item']]=o
st=defaultdict(Counter)
for i,k in key.items():
    if k['decoy']: continue
    st[strat[k['fid']]][res[i]['verdict']]+=1
print('stratum correct wrong_detail unsupported wrong')
for s in sorted(st): print(s,*(st[s][v] for v in('correct','wrong_detail','unsupported','wrong')))
def cdf(k,n,p): return sum(comb(n,i)*p**i*(1-p)**(n-i) for i in range(k+1))
def bis(f,t):
    lo,hi=0.0,1.0
    for _ in range(60):
        m=(lo+hi)/2
        if f(m)>t: lo=m
        else: hi=m
    return lo
def cp(k,n,a=0.05):
    # Clopper-Pearson: upper p with P(X<=k)=a/2 ; lower p with P(X>=k)=a/2
    hi=1 if k==n else bis(lambda p:cdf(k,n,p),a/2)
    lo=0 if k==0 else bis(lambda p:-(1-cdf(k-1,n,p)),-a/2)
    return lo,hi
n=sum(sum(c.values()) for c in st.values())
for lab,k in(('all non-correct (blind verdict)',5),('after adjudication: literal errors',1),('after adjudication: real errors',0)):
    lo,hi=cp(k,n); print(lab,k,n,f'{k/n:.2%} CI95 [{lo:.2%},{hi:.2%}]  x22035 -> {k/n*22035:.0f} [{lo*22035:.0f},{hi*22035:.0f}]')
# deterministic cross-check on pure tail-branch functions
bad=0;tot=0
for i,k in key.items():
    if k['decoy']: continue
    ins=live[k['fid']]['ins']
    if len(ins)==1 and ins[0][1].startswith('b '):
        tot+=1; tgt=ins[0][1].split()[1].replace('0x','').lstrip('0'); 
        if tgt not in T[k['fid']].lower().replace('0x','').replace('fun_','').lstrip('0') and tgt not in T[k['fid']].lower(): bad+=1; print('target missing in text',i,k['fid'],tgt,T[k['fid']][:100])
print('pure-b items',tot,'target not found',bad)
