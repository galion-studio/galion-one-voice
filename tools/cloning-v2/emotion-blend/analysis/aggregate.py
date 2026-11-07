import json,math,statistics,collections
from pathlib import Path
ROOT=Path('/workspace/swarm/voice-v2-sprint/cloning-v2'); OUT=ROOT/'emotion-blend/analysis'
def entropy(p):return -sum(v*math.log2(v) for v in p if v>0)
def summary(rows):
 ps={k:statistics.mean(r['ser']['probs'][k] for r in rows) for k in ('neu','hap','ang','sad')}
 counts=dict(collections.Counter(r['ser']['top'] for r in rows))
 out={'n':len(rows),'mean_ser_probabilities':ps,'top_label_counts':counts,'label_mix_entropy_bits':entropy([v/len(rows) for v in counts.values()]),'mean_probability_entropy_bits':entropy(ps.values()),'mean_within_window_entropy_bits':statistics.mean(entropy(r['ser']['probs'].values()) for r in rows)}
 total_duration=sum(r['duration_s'] for r in rows)
 out['duration_weighted_ser_probabilities']={k:sum(r['duration_s']*r['ser']['probs'][k] for r in rows)/total_duration for k in ps}
 hard_time={k:sum(r['duration_s'] for r in rows if r['ser']['top']==k)/total_duration for k in counts}
 out['duration_weighted_label_mix_entropy_bits']=entropy(hard_time.values())
 out['duration_weighted_top_label_share']=hard_time
 for group,keys in [('avd',['arousal','valence','dominance']),('proxies',['f0_mean_hz','f0_sd_st','f0_range_st_p5_p95','rms_dbfs','energy_dr_db','pause_ratio','spectral_tilt_db_oct','speech_rate_syl_s','articulation_rate_syl_s'])]:
  for k in keys:
   vals=[r[group][k] for r in rows];out[k]={'mean':statistics.mean(vals),'population_sd':statistics.pstdev(vals),'population_variance':statistics.pvariance(vals),'min':min(vals),'max':max(vals)}
 return out
raw=json.load(open(ROOT/'emotion-blend/A-global/original-emotion.json'))['files'];man=json.load(open(ROOT/'emotion-blend/A-global/window-manifest.json'));cont={str(Path(r['file']).resolve()):r for r in json.load(open(OUT/'contours.json'))}
md=['# Original blend versus merged-production clones','', 'Measured 2026-11-05 Europe/Zagreb (CET). Original fp32 Rust emotion scores reused from `../A-global/original-emotion.json`; merged-production scores reused from `merge-v2/scores/*_merge_v2_emo.json`. Contour supplement is Rust YIN, the identical scorer tracker. No original altered.','', '**Caution:** neu/hap/ang/sad are classifier outputs, not a human emotional inventory. “Sweet” and “playful” are not classes. Mean probabilities measure evidence/uncertainty, not literal emotional percentages. 2.5s original windows versus full different-text lines confound duration/text; therefore do not call any drop proven temporal emotion collapse.','']
for voice,short in [('samantha','sam'),('jarvis','jar')]:
 originals=[]
 for m in man:
  if m['persona']!=voice:continue
  r=next(r.copy() for r in raw if Path(r['file']).name==Path(m['path']).name);r.update({k:m[k] for k in ['start_s','end_s']});r.update(cont.get(str(Path(m['path']).resolve()),{}));r['label_entropy_bits']=entropy(r['ser']['probs'].values());originals.append(r)
 clones=json.load(open(ROOT/f'merge-v2/scores/{short}_merge_v2_emo.json'))['files']
 for r in clones:
  r['line_id']=Path(r['file']).stem;r.update(cont.get(str(Path(r['file']).resolve()),{}));r['label_entropy_bits']=entropy(r['ser']['probs'].values())
 whole=next(r for r in raw if r['file']==f'../clip-store-own/{voice}/original.wav')
 a,b=summary(originals),summary(clones)
 profile={'schema':'galion-emotion-blend/profile-1','voice':voice,'methods':{'emotion':'galion-emotion fp32 2 threads','f0':'Rust YIN 60-500Hz,10ms hop; SD and ranges in semitones; supplement SD/min/max in Hz; slope OLS semitones/s of voiced frames','energy':'RMS dBFS and active-frame dynamic range dB','rate':'acoustic syllable-nuclei/s, not ASR words/s','entropy':'bits; hard-top label mix and mean-probability entropy both supplied','limitations':'different texts; original windows vs clone lines differ in duration; small n; no fine-affect classifier'},'original_whole':whole,'original_windows':originals,'clone_lines':clones,'original_summary':a,'clone_summary':b}
 primary=OUT/f'profile-{voice}.json'
 existing=json.load(open(primary)) if primary.exists() else {}
 if existing.get('schema')=='galion-voicecheck-profile/1':
  profile['analysis_schema']=profile.pop('schema');profile={**existing,**profile}
 primary.write_text(json.dumps(profile,indent=2)+'\n')
 (OUT/f'profile-{voice}-detailed.json').write_text(json.dumps(profile,indent=2)+'\n')
 md += [f'## {voice.title()}', '',f'Original top labels: {a["top_label_counts"]}; clone: {b["top_label_counts"]}.', '', '| Metric | Original 2.5s windows | Merged clone lines |','|---|---:|---:|']
 for k in ['label_mix_entropy_bits','mean_probability_entropy_bits','mean_within_window_entropy_bits']:
  md.append(f'| {k} | {a[k]:.6f} | {b[k]:.6f} |')
 for k in ['arousal','valence','dominance','f0_mean_hz','f0_sd_st','f0_range_st_p5_p95','energy_dr_db','pause_ratio','spectral_tilt_db_oct','speech_rate_syl_s']:
  md.append(f'| {k}, mean ± population SD | {a[k]["mean"]:.6f} ± {a[k]["population_sd"]:.6f} | {b[k]["mean"]:.6f} ± {b[k]["population_sd"]:.6f} |')
 md+=['','Mean SER probabilities (neu/hap/ang/sad): original '+str({k:round(v,5) for k,v in a['mean_ser_probabilities'].items()})+'; clone '+str({k:round(v,5) for k,v in b['mean_ser_probabilities'].items()}), '', '| Original seconds | top | neu | hap | ang | sad | A | V | D |','|---|---|---:|---:|---:|---:|---:|---:|---:|']
 for r in originals:
  md.append(f'| {r["start_s"]:.2f}–{r["end_s"]:.2f} | {r["ser"]["top"]} | '+ ' | '.join(f'{r["ser"]["probs"][k]:.4f}' for k in ['neu','hap','ang','sad'])+' | '+' | '.join(f'{r["avd"][k]:.4f}' for k in ['arousal','valence','dominance'])+' |')
 md+=['']
md+=['## Interpretation / immediate guidance for candidate worker','', 'Read the tables rather than assuming the owner theory is established. The whole-recording Samantha score has P(sad)=0.318, yet 2.5s windows average P(sad)=0.012; this is strong context-length dependence, NOT evidence that sadness was removed by cloning. Original windows contain neutral/angry detector modes and one happy-evidence interval at 10–12.5s (P(hap)=0.363). Hard-label entropy rises, rather than collapses, in merged Samantha. Her between-window F0-mean SD falls 20.03→8.67 Hz and energy-DR SD falls 2.15→0.92 dB; arousal variance falls 0.003577→0.002485, while valence variance does not fall. Jarvis F0 ranges grow rather than flatten. Clones can shift toward angry detector evidence without losing all F0 movement. Jarvis is an essential negative control: neutral hard labels can conceal valence loss. A reference fix should preserve multiple source regions and their unnormalised relative dynamics, not force a hand-authored happy/angry mixture. Do not average discrete speech tokens or splice unrelated prompt tokens.','', '**Important duration check:** the same-window comparison below does NOT reproduce an arousal-variance collapse for Samantha (SD 0.059805→0.062946). The stronger line-level flattening estimates partly reflect averaging longer lines. Prefer the duration-matched table: F0 range 8.63→7.19 st, between-window F0-mean SD 20.03→15.96 Hz, pause ratio 0.154→0.070. Neither categorical entropy nor Samantha arousal variation collapses.','Root-cause audit and opt-in global implementation follow in `../global-fix/REPORT.md`.']
md+=['## Duration-matched production-window cross-check','', 'Additional fp32 same-method source: `../A-global/production-window-emotion.json`; ~2.5s splitting merges/redistributes short tails. Still different text and unequal numbers of windows.','']
pmanifest=json.load(open(ROOT/'emotion-blend/A-global/production-window-manifest.json'))
praw=json.load(open(ROOT/'emotion-blend/A-global/production-window-emotion.json'))['files']
for voice in ['samantha','jarvis']:
 primary=OUT/f'profile-{voice}.json';profile=json.load(open(primary));windows=[]
 for m in pmanifest:
  if m['persona']!=voice or m['cohort']!='merged':continue
  found=next((r.copy() for r in praw if Path(r['file']).name==Path(m['path']).name),None)
  if found is None:continue
  found.update(m);found.update(cont.get(str((ROOT/m['path']).resolve()),{}));windows.append(found)
 if windows:
  profile['clone_windows']=windows;profile['clone_window_summary']=summary(windows)
  for path in [primary,OUT/f'profile-{voice}-detailed.json']:path.write_text(json.dumps(profile,indent=2)+'\n')
  a=profile['original_summary'];b=profile['clone_window_summary'];md += [f'### {voice.title()} ({len(windows)} merged windows)','', '| Metric | Original windows | Clone windows |','|---|---:|---:|']
  for k in ['label_mix_entropy_bits','mean_probability_entropy_bits']:
   md.append(f'| {k} | {a[k]:.6f} | {b[k]:.6f} |')
  for k in ['arousal','valence','f0_mean_hz','f0_range_st_p5_p95','energy_dr_db','pause_ratio']:
   md.append(f'| {k}, mean ± population SD | {a[k]["mean"]:.6f} ± {a[k]["population_sd"]:.6f} | {b[k]["mean"]:.6f} ± {b[k]["population_sd"]:.6f} |')
  md+=['']
(OUT/'FINDINGS.md').write_text('\n'.join(md)+'\n')
print('\n'.join(md))
