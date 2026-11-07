import json, statistics as st
a=json.load(open('out/py_squim.json'))
for p in ('samantha','jarvis'):
    r=json.load(open(f'out/rs_mos_{p}.json'))
    md={k:0 for k in ('mos','stoi','pesq','si_sdr')}
    for f in r['files']:
        for k in md: md[k]=max(md[k],abs(f[k]-a[f['path']][k]))
    cl=[f for f in r['files'] if 'reference' not in f['path']]
    print(p,'rust clone MOS mean',round(st.mean(f['mos'] for f in cl),4),'py',round(st.mean(a[f['path']]['mos'] for f in cl),4),
          '| PESQ',round(st.mean(f['pesq'] for f in cl),4),'py',round(st.mean(a[f['path']]['pesq'] for f in cl),4),'| max|d|',md,r['timing'])
