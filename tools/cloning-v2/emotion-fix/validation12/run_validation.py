#!/usr/bin/env python3
"""Box-local serialized audit. Never changes profiles or source code."""
import os, subprocess, time, json, pathlib, datetime
B=pathlib.Path('/workspace/swarm/voice-v2-sprint/cloning-v2/emotion-fix')
os.chdir(B)
os.environ.update(HF_HUB_OFFLINE='1',OMP_NUM_THREADS='2',OPENBLAS_NUM_THREADS='2',MKL_NUM_THREADS='2')
Q=B/'validation12'; CB=B/'cbx-emo/target/release/cbx'

def log(s): print(datetime.datetime.now().astimezone().isoformat(),s,flush=True)
def busy():
    lines=subprocess.check_output(['ps','-eo','pid,args'],text=True).splitlines()
    result=[]
    for line in lines:
        a=line.split(None,1)
        if len(a)!=2: continue
        args=a[1]
        if '/cbx ' in args and (' speak ' in args or ' batch ' in args): result.append(line)
    return result

def wait_idle():
    # Wait for pre-existing render queues as well as their children. Never stop them.
    while True:
        active=busy()
        available=int(next(line.split()[1] for line in pathlib.Path('/proc/meminfo').read_text().splitlines() if line.startswith('MemAvailable:')))//1024
        if (not active and available>=3800) or (len(active)<=1 and available>=4800): return
        log('WAIT external render/queue: '+repr(active))
        time.sleep(60)

def run(args, logfile):
    with open(logfile,'a') as f:
        log('RUN '+repr([str(x) for x in args]))
        subprocess.run(['nice','-n','10',*[str(x) for x in args]],stdout=f,stderr=subprocess.STDOUT,check=True)

configs=[('jarvis','own-jarvis-v2-emo-m2','jar_mild_warm10_legacy_t0_s0',0,0),
('jarvis','own-jarvis-v2-emo-m1','jar_mild_halfnorm_legacy_t0_s0',0,0)]
textdir=Q/'texts'; textdir.mkdir(exist_ok=True)
ids=[]
for line in (B/'test12.tsv').read_text().splitlines():
    ident,text=line.split('\t',1); ids.append(ident); (textdir/(ident+'.txt')).write_text(text)
assert len(ids)==12
for persona,voice,tag,temp,seed in configs:
    out=Q/tag; out.mkdir(exist_ok=True); lf=out/'audit.log'
    if (out/'COMPLETE.json').exists(): continue
    ref=B.parent.parent/'clip-store-own'/persona/'reference.wav'
    if voice.startswith('PAUSE:'):
        source=Q/voice.split(':',1)[1]
        assert (source/'COMPLETE.json').exists()
        pausebin=B.parent/'pipeline/cbx-v2-prosody/pause-tool/target/release/cbx-pauses'
        for ident in ids:
            dest=out/(ident+'.wav')
            if not dest.exists(): run([pausebin,source/(ident+'.wav'),dest,ref,textdir/(ident+'.txt'),out/(ident+'.pauses.txt'),'--punctuation-pauses'],lf)
    elif not (out/'RENDERED.json').exists():
        wait_idle()
        if any(out.glob('t*.wav')):
            raise RuntimeError('Partial directory requires explicit review, refusing overwrite: '+str(out))
        run([CB,'--intra-threads','2','batch','--dtype','fp32','--voice',voice,'--manifest',B/'test12.tsv','--out-dir',out,'--target',B/'targets'/f'{persona}.json','--temperature',temp,'--seed',seed,'--repetition-penalty','1.2'],lf)
        (out/'RENDERED.json').write_text('{}')
    wavs=[out/(i+'.wav') for i in ids]
    assert all(w.exists() for w in wavs)
    other=B.parent.parent/'clip-store-own'/('jarvis' if persona=='samantha' else 'samantha')/'reference.wav'
    S=B.parent/'scorers'
    commands=[
      [S/'emotion/target/release/galion-emotion','score','--threads','2','--compact','--ref',ref,*wavs,'--out',out/'emotion.json'],
      [S/'ecapa/target/release/galion-ecapa','--threads','2','--no-ceiling','--ref',ref,*wavs,'--out',out/'ecapa.json'],
      [S/'articulation/bin/galion-artic','profile','-j','2','--compact',*wavs,'--out',out/'artic.json'],
      [S/'quality/bin/galion-wer','--jobs','2','--text-dir',textdir,'--out',out/'wer.json',*wavs],
      [S/'quality/bin/galion-mos','--threads','2','--nmr',other,'--out',out/'mos.json',*wavs],
      [S/'quality/bin/galion-hygiene','--out',out/'hygiene.json',*wavs]]
    for command in commands:
        destination=pathlib.Path(command[command.index('--out')+1])
        if not destination.exists(): run(command,lf)
    (out/'COMPLETE.json').write_text(json.dumps(dict(persona=persona,voice=voice,temperature=temp,seed=seed,n=12)))
    log('COMPLETE '+tag)
    e=json.loads((out/'emotion.json').read_text()); ec=json.loads((out/'ecapa.json').read_text())
    assert e['summary']['n_files']==12 and ec['n']==12
    row=dict(tag=tag,n=12,emotion=e['summary']['mean_emotion_distance'],ecapa=ec['mean'],pause_ratio=sum(f['proxies']['pause_ratio'] for f in e['files'])/12,valence=e['summary']['mean_avd']['valence'])
    with open(B/'REPORT-12LINE.md','a') as report:
        report.write('\n### Completed '+tag+' — '+datetime.datetime.now().astimezone().isoformat()+' (Europe/Zagreb)\n\n```json\n'+json.dumps(row,indent=2)+'\n```\nAll six Rust scorers completed; full outputs: `validation12/'+tag+'/`.\n')
    with open(Q/'completed.jsonl','a') as f: f.write(json.dumps(row)+'\n')
    if tag=='jar_mild_halfnorm_legacy_t0_s0':
        (Q/'PRIORITY_COMPLETE').write_text(datetime.datetime.now().astimezone().isoformat())
        log('PRIORITY COMPLETE: Jarvis mild 12-line cohort scored; paired pauses scored separately; Samantha reused from VALIDATE12.md.')
log('ALL COMPLETE: results require matched aggregate review; no automatic voice promotion.')
