#!/usr/bin/env python3
"""Reject reader outputs that look script-generated: duplicate rationale, quotes not in the body, missing fields."""
import json,sys,glob,re
R='/home/nico/work/decompilacion/work/d4/spotcheck2/'
bad={}
for k in sorted(glob.glob(R+'out*.json')):
    n=k[-7:-5]; out=json.load(open(k)); b=json.load(open(R+f'batch{n}.json')); by={x['item']:x for x in b}
    probs=[]
    if [o['item'] for o in out]!=[x['item'] for x in b]: probs.append('item list mismatch')
    from collections import Counter
    c=Counter(o.get('rationale','') for o in out); c2=Counter(o.get('own_summary','') for o in out)
    for o in out:
        it=by.get(o['item']); 
        if not it: continue
        body=' '.join(it['instructions']).lower()
        ev=o.get('evidence') or []
        if not ev: probs.append(f"{o['item']}: no evidence")
        hit=0
        for e in ev:
            toks=re.findall(r'[a-z]{2,6}\s+[wx]\d+|0x[0-9a-f]+|#0x[0-9a-f]+',str(e).lower())
            if any(t in body for t in toks) or str(e).lower()[:25] in body: hit+=1
        if ev and hit==0: probs.append(f"{o['item']}: evidence quotes not found in body")
        if c[o.get('rationale','')]>1 and len(o.get('rationale',''))>0: probs.append(f"{o['item']}: duplicated rationale")
        if c2[o.get('own_summary','')]>1 and len(it['instructions'])>3: probs.append(f"{o['item']}: duplicated own_summary")
        if o['verdict'] not in('correct','wrong_detail','unsupported','wrong'): probs.append(f"{o['item']}: bad verdict")
        if len(o.get('rationale',''))<25 or len(o.get('own_summary',''))<12: probs.append(f"{o['item']}: too short")
    bad[n]=probs
    print(n,len(out),len(probs)); [print('  ',p) for p in probs[:12]]
