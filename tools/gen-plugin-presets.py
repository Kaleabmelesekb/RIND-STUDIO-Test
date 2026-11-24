#!/usr/bin/env python3
"""Rebuilds src/content/plugin_presets.txt from Mango Studio's factory presets (GPL-3.0-or-later,
https://github.com/apikun/Mango-Studio). Run from the repo root: tools/gen-plugin-presets.py <Mango-Studio checkout>.
Skips the plugins Honeydew does not have (Mango 12, Mango Beat). A preset with no values is written as `=0`."""
import os,re,base64,sys
R=os.path.join(sys.argv[1],"crates/os-app/factory-presets")
SKIP={'Mango 12','Mango Beat'}
out=[]
n=0
for d in sorted(os.listdir(R)):
    p=os.path.join(R,d)
    if not os.path.isdir(p) or d in SKIP: continue
    for r,_,fs in sorted(os.walk(p)):
        for f in sorted(fs):
            if not f.endswith('.ospreset'): continue
            cat=os.path.relpath(r,p); cat='' if cat=='.' else cat
            if '/' in cat: continue
            b=open(os.path.join(r,f),'rb').read()
            assert b.startswith(b'OSPRESET 1\n')
            l2,rest=b[11:].split(b'\n',1)
            h=dict(kv.split('=',1) for kv in l2.decode().split('\t'))
            pid=h['id'].removeprefix('clap:')
            name=f[:-9]
            out.append(f'@@\t{pid}\t{cat}\t{name}')
            if h.get('body')=='params':
                for line in rest.decode().split('\n'):
                    line=line.strip()
                    if not line or line.startswith('#'): continue
                    k,v=line.rsplit('=',1)
                    out.append(f'{k.strip()}={v.strip()}')
            else:
                out.append('#state '+base64.b64encode(rest).decode())
            if out[-1].startswith('@@'): out.append('=0')
            n+=1
open('src/content/plugin_presets.txt','w').write('\n'.join(out)+'\n')
print(n)
