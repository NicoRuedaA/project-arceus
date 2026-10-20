#!/usr/bin/env python3
"""Stratified random sample for the D4 quality spot-check of depth rounds 45-51 (mech-c1) and 60-66 (mech-c2). Fixed seed."""
import json,random,glob,os
SEED=20261021
R='/home/nico/work/decompilacion/'; D=R+'work/d4/'; O=D+'spotcheck2/'
rnd=random.Random(SEED)
own={}
for l in open(R+'reports/function-progress/d2-function-owner.tsv'):
    if l.startswith('#') or l.startswith('function_id'): continue
    p=l.rstrip('\n').split('\t'); own[p[0]]=p[1]
LIB={'NET','HAVOK','WWISE','NN_SDK','LUA_CORE','OODLE'}
CONF={'high':2,'medium':1,'low':0}
info={}; held={}
for base in('mech-c1','mech-c2'):
    for d in sorted(glob.glob(D+base+'/r*/final.json')):
        rd=os.path.dirname(d); n=int(os.path.basename(rd)[1:]); j=json.load(open(d))
        auth={}
        for a in glob.glob(rd+'/author/A*.json'):
            for r in json.load(open(a)): auth[r['function_id']]=r
        for f,t in j['accepted'].items():
            dis=json.load(open(f'{rd}/snap/{f}.dis.txt'))['instructions']
            info[f]=dict(round=n,text=t,ins=len(dis),owner=own.get(f,'?'),conf=(auth.get(f) or {}).get('confidence','?'),
                         plt=t.startswith('PLT stub'))
        for f,reason in j['held'].items():
            held[f]=dict(round=n,reason=reason,text=(auth.get(f) or {}).get('summary'),conf=(auth.get(f) or {}).get('confidence','?'))
byr={}
for f,v in info.items(): byr.setdefault(v['round'],[]).append(f)
rounds=sorted(byr); quota={r:21 for r in rounds}
for r in rnd.sample(rounds,300-21*len(rounds)): quota[r]+=1
sample=[];used=set()
def take(pool,k,r):
    pool=[f for f in pool if f not in used]
    for f in rnd.sample(sorted(pool),min(k,len(pool))): sample.append((f,r)); used.add(f)
for r in rounds:
    fs=sorted(byr[r]); q=quota[r]
    take([f for f in fs if info[f]['ins']>=5],5,r)
    take([f for f in fs if info[f]['owner'] in LIB],4,r)
    take([f for f in fs if info[f]['plt']],3,r)
    low=[f for f in fs if f not in used and info[f]['conf'] in('low','medium')]
    take(low,3,r)
    take(fs,q-sum(1 for _,x in sample if x==r),r)
ctrl=[]
rest=sorted(f for f in info if f not in used)
for f in rnd.sample(rest,30): ctrl.append((f,info[f]['round'])); used.add(f)
hs=sorted(held); special=[f for f in hs if not held[f]['reason'].startswith('boundary') and held[f]['text']]  # the non-boundary holds, always included
hsample=[(f,held[f]['round']) for f in special+rnd.sample([f for f in hs if held[f]['text'] and f not in special],30-len(special))]
json.dump({'seed':SEED,'quota':quota,'sample':sample,'control':ctrl,'held_sample':hsample,
  'counts':{r:len(byr[r]) for r in rounds},'held_counts':{r:sum(1 for v in held.values() if v['round']==r) for r in rounds},
  'info':{f:info[f] for f in used},'held_info':{f:held[f] for f,_ in hsample}},open(O+'sample.json','w'))
json.dump({f:info[f]['text'] for f in used},open(O+'texts.json','w'))
json.dump({f:held[f]['text'] for f,_ in hsample},open(O+'held_texts.json','w'))
print(len(sample),len(ctrl),len(hsample),len(info),len(held))
