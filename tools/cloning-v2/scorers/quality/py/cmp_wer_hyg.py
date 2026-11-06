import json
S='/workspace/swarm/voice-v2-sprint/own-clone-analysis/scores/'
w=json.load(open(S+'wer.json')); b=json.load(open(S+'naturalness_artifacts_prosody.json'))
for p in ('samantha','jarvis'):
    rw=json.load(open(f'out/rs_wer_{p}.json')); rh=json.load(open(f'out/rs_hyg_{p}.json'))
    print(p,'rust',rw['summary'],rw['timing']); print(p,'py',{k:w[p][k] for k in ('mean','sd','frac_le_10pct')})
    for f in rw['files']:
        t=f['path'].split('/')[-1][:3]; pw=w[p]['per_clip'][t]
        if f['hyp']!=pw['hyp'] or f['wer']!=pw['wer']: print('  DIFF',t,f['wer'],pw['wer'],'|',f['hyp'],'|',pw['hyp'])
    for f in rh['files']:
        t=f['path'].split('/')[-1][:3]
        if t in b[p]['per_clip']:
            a=b[p]['per_clip'][t]['artifacts']
            if (f['clipped_samples'],f['clicks'],round(f['sample_peak_dbfs'],2))!=(a['clipping_samples'],a['clicks_big_jumps'],a['peak_dbfs']) or abs(f['max_sample_jump']-a['max_sample_jump'])>1e-3:
                print('  HDIFF',t,f['clipped_samples'],a['clipping_samples'],f['clicks'],a['clicks_big_jumps'],f['sample_peak_dbfs'],a['peak_dbfs'],f['max_sample_jump'],a['max_sample_jump'])
        else: print('  ',f['path'][-30:],f['sample_peak_dbfs'],f['true_peak_dbtp'],f['clipped_samples'],f['clicks'],f['bandwidth']['f99_hz'],f['loudness']['integrated_lufs'],f['loudness']['lra_lu'])
    print(rh['timing'])
