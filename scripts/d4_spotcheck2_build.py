#!/usr/bin/env python3
"""Fetch LIVE bodies from Ghidra MCP (read-only GETs), build blind reader batches (real + decoy + held items mixed)."""
import json,random,urllib.request
from concurrent.futures import ThreadPoolExecutor
R='/home/nico/work/decompilacion/work/d4/spotcheck2/'
S=json.load(open(R+'sample.json')); T=json.load(open(R+'texts.json')); H=json.load(open(R+'held_texts.json'))
def strip(c):  # drop the leading '// D4 ... mechanical class' comment block: it would leak the earlier description
    L=c.split('\n')
    while L and (L[0].strip()=='' or L[0].lstrip().startswith('//')): L.pop(0)
    return '\n'.join(L)
def get(u): return urllib.request.urlopen('http://127.0.0.1:8089/'+u,timeout=180).read()
ids=[f for f,_ in S['sample']+S['control']+S['held_sample']]
def one(f):
    j=json.loads(get(f'get_functions?function={f}')); d=json.loads(get(f'disassemble_function?address={f}'))
    return f,{'ins':[(i['address'],i['instruction']) for i in d['instructions']],'dec':strip(j.get('decompiled_code') or ''),'size':j['size']}
with ThreadPoolExecutor(3) as ex: live=dict(ex.map(one,ids))
json.dump(live,open(R+'live.json','w'))
body=lambda f:[i for _,i in live[f]['ins']]
ctrl=sorted([f for f,_ in S['control']],key=lambda f:(len(live[f]['ins']),f))
decoy={}
for i,f in enumerate(ctrl):  # nearest neighbour in length whose code differs (stricter than the first spot-check)
    for k in range(1,len(ctrl)):
        g=ctrl[(i+k)%len(ctrl)]
        if body(g)!=body(f): decoy[f]=g; break
items=[(f,T[f],'real',f) for f,_ in S['sample']]+[(f,T[decoy[f]],'decoy',decoy[f]) for f in ctrl]+[(f,H[f],'held',f) for f,_ in S['held_sample']]
rnd=random.Random(20261021); rnd.shuffle(items)
NB=12; key={}; batches=[[] for _ in range(NB)]
for n,(f,t,kind,src) in enumerate(items):
    iid=f'I{n+1:03d}'; key[iid]={'fid':f,'kind':kind,'text_from':src}
    batches[n%NB].append({'item':iid,'function_address':f,'instructions':[f'{a} {i}' for a,i in live[f]['ins']],'decompiler':live[f]['dec'][:6000],'stored_description':t})
json.dump(key,open(R+'key.json','w'))
for k,b in enumerate(batches,1): json.dump(b,open(R+f'batch{k:02d}.json','w'),indent=0)
print(len(items),[len(b) for b in batches],sum(1 for v in key.values() if v['kind']=='decoy'))
