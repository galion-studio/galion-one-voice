#!/usr/bin/env python3
import json,math,statistics as st
from pathlib import Path
BASE=Path(__file__).resolve().parents[2]; OUT=Path(__file__).resolve().parent
load=lambda p:json.loads(Path(p).read_text())
LABELS=('neu','hap','ang','sad')
def entropy(p):return -sum(v*math.log2(v) for v in p if v>0)
def stats(fs):
    mean={k:st.mean(f['ser']['probs'][k] for f in fs) for k in LABELS}
    hs=[entropy(f['ser']['probs'].values()) for f in fs]
    return {'n':len(fs),'mean_probs':mean,'mean_entropy_bits':st.mean(hs),'entropy_of_mean_probs_bits':entropy(mean.values()),
       'temporal_js_bits':entropy(mean.values())-st.mean(hs),
       'mean_class_probability_spread':st.mean(max(f['ser']['probs'].values())-min(f['ser']['probs'].values()) for f in fs),
       'probability_ranges':{k:[min(f['ser']['probs'][k] for f in fs),max(f['ser']['probs'][k] for f in fs)] for k in LABELS},
       'probability_variance':{k:st.pvariance(f['ser']['probs'][k] for f in fs) for k in LABELS},
       'arousal_variance':st.pvariance(f['avd']['arousal'] for f in fs),
       'mean_avd':{k:st.mean(f['avd'][k] for f in fs) for k in ('arousal','valence','dominance')},
       'mean_proxies':{k:st.mean(f['proxies'][k] for f in fs if f['proxies'][k] is not None) for k in ['f0_median_hz','f0_sd_st','f0_range_st_p5_p95','energy_dr_db','speech_rate_syl_s','articulation_rate_syl_s','pause_ratio']}}
def art(f):return {'file':f['file'],'f0':f['f0']['pyin'],'intensity':f['intensity'],'rate':f['rate'],'pauses':f['pauses']}
raw=load(OUT/'original-emotion.json')['files']; pw=load(OUT/'production-window-emotion.json')['files']; wm=load(OUT/'production-window-manifest.json'); articulation=load(OUT/'articulation.json')['files']
profile={'schema':'galion-emotion-blend/2','status':'complete','method':{'emotion':'Rust galion-emotion fp32 SER neu/hap/ang/sad + A/V/D; threads=2; no int8',
'windows':'originals split into equal-duration ~2.5s windows (Samantha 7, Jarvis 6); production lines equal-duration windows nearest 2.5s',
'probability_caveat':'Model confidence/uncertainty, NOT literal emotion fractions. Sweet and playful are unavailable labels. Whole-file outputs are nonlinear and cannot be averaged into ground truth.',
'entropy':'Shannon bits, maximum 2. Mean H measures within-window uncertainty, H(mean p)-mean H is temporal JS diversity.',
'variance':'population variance across windows, unitless squared A probability scale. Production pooled variance includes between-line variability; within-line variance listed separately.',
'comparison_limit':'Original recording and 12 rendered lines have different text/duration. Do not infer causal emotion accuracy from aggregate entropy; pre/post timbre is matched-waveform ablation.',
'pace':'two independent syllable estimators; galion-emotion proxy and galion-artic values are not interchangeable; no transcript-derived words/s asserted.'},'voices':{}}
for persona,prefix in [('samantha','sam'),('jarvis','jar')]:
    whole=next(f for f in raw if f['file']==f'../clip-store-own/{persona}/original.wav')
    ws=load(OUT/f'{persona}-diverse-reference.windows/scores.json')['files']; duration=whole['duration_s']
    windows=[{**f,'start_s':i*duration/len(ws),'end_s':(i+1)*duration/len(ws),'entropy_bits':entropy(f['ser']['probs'].values())} for i,f in enumerate(ws)]
    a=next(f for f in articulation if f['file']==f'../clip-store-own/{persona}/original.wav')
    entry={'original':{'whole':whole,'whole_entropy_bits':entropy(whole['ser']['probs'].values()),'windows':windows,'window_summary':stats(ws),'articulation':art(a)}}
    for cohort in ['merged','pre_timbre']:
        fs=[f for f in pw if Path(f['file']).name.startswith(persona+'_'+cohort+'_')]
        timed=[]
        for f in fs:
            meta=next(m for m in wm if Path(m['path']).name==Path(f['file']).name)
            timed.append({**f,**meta,'entropy_bits':entropy(f['ser']['probs'].values())})
        cohortentry={'windows':timed,'window_summary':stats(fs),'within_line_arousal_variance':{line:st.pvariance(f['avd']['arousal'] for f in timed if f['line']==line) for line in sorted({f['line'] for f in timed})}}
        if cohort=='merged':
            full=load(BASE/f'merge-v2/scores/{prefix}_merge_v2_emo.json')['files'];cohortentry.update({'files':full,'per_file_summary':stats(full),'articulation':[art(f) for f in articulation if f['file'].startswith('merge-v2/wav/'+persona+'/')]})
        entry[cohort]=cohortentry
    entry['selected_reference']=load(OUT/f'{persona}-diverse-reference.json')
    profile['voices'][persona]=entry
if (OUT/'hypothesis-emotion.json').exists():profile['hypothesis_scores']=load(OUT/'hypothesis-emotion.json')
profile['temperature_existing_12line_ablation']={tag:stats(load(BASE/f'emotion-fix/scores/{tag}_emo.json')['files']) for tag in ['sd_r2_t012','sd_r2_up12']}
if (OUT/'conditioning-segments-emotion.json').exists():
    cs=load(OUT/'conditioning-segments-emotion.json')['files'];profile['conditioning_segment_validation']={}
    for persona in ['samantha','jarvis']:
        profile['conditioning_segment_validation'][persona]={}
        for label in ['prefix','montage']:
            ss=[f for f in cs if Path(f['file']).name.startswith(persona+'-'+label+'-')]
            summary=stats(ss)
            profile['conditioning_segment_validation'][persona][label]={'segments':ss,**summary}
(OUT/'profile.json').write_text(json.dumps(profile,indent=2))
print('updated profile.json')
for name,v in profile['voices'].items():
    print(name)
    for cohort in ['original','pre_timbre','merged']:
        s=v[cohort]['window_summary'];print(cohort,json.dumps(s,indent=2))
