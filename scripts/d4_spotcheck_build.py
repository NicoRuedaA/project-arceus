#!/usr/bin/env python3
"""Fetch LIVE bodies from Ghidra MCP (read-only GETs), build blind reader batches (decoys mixed in)."""
import json,random,urllib.request
from concurrent.futures import ThreadPoolExecutor
R='/home/nico/work/decompilacion/work/d4/spotcheck/'
S=json.load(open(R+'sample.json')); T=json.load(open(R+'texts.json'))
def get(u): return urllib.request.urlopen('http://127.0.0.1:8089/'+u,timeout=180).read()
ids=[f for f,_ in S['sample']+S['spare']+S['control']]
def one(f):
    j=json.loads(get(f'get_functions?function={f}')); d=json.loads(get(f'disassemble_function?address={f}'))
    return f,{'ins':[(i['address'],i['instruction']) for i in d['instructions']],'dec':j.get('decompiled_code') or '','size':j['size']}
with ThreadPoolExecutor(3) as ex: live=dict(ex.map(one,ids))
json.dump(live,open(R+'live.json','w'))
ctrl=sorted([f for f,_ in S['control']],key=lambda f:(len(live[f]['ins']),f))
decoy={ctrl[i]:ctrl[(i+1)%len(ctrl)] for i in range(len(ctrl))}  # text borrowed from a neighbour of similar length
items=[(f,T[f],False,f) for f,_ in S['sample']]+[(f,T[decoy[f]],True,decoy[f]) for f in ctrl]
rnd=random.Random(20261020); rnd.shuffle(items)
key={}; batches=[[] for _ in range(12)]
for n,(f,t,isd,src) in enumerate(items):
    iid=f'I{n+1:03d}'; key[iid]={'fid':f,'decoy':isd,'text_from':src}
    batches[n%12].append({'item':iid,'function_address':f,'instructions':[f'{a} {i}' for a,i in live[f]['ins']],'decompiler':live[f]['dec'][:6000],'stored_description':t})
json.dump(key,open(R+'key.json','w'))
for k,b in enumerate(batches,1): json.dump(b,open(R+f'batch{k:02d}.json','w'),indent=0)
print(len(items),[len(b) for b in batches],sum(1 for v in key.values() if v['decoy']))
