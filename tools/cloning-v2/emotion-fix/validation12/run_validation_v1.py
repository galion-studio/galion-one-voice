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
        if ('/cbx ' in args and (' speak ' in args or ' batch ' in args)) or any(x in args for x in ['bash /workspace/swarm/voice-v2-sprint/cloning-v2/similarity/scripts/run_synth','bash /workspace/swarm/voice-v2-sprint/cloning-v2/hf-jarvis/scripts/synth_']): result.append(line)
    return result

def wait_idle():
    # Wait for pre-existing render queues as well as their children. Never stop them.
    while True:
        active=busy()
        if not active: return
        log('WAIT external render/queue: '+repr(active))
        time.sleep(60)

def run(args, logfile):
    with open(logfile,'a') as f:
        log('RUN '+repr([str(x) for x in args]))
        subprocess.run(['nice','-n','10',*[str(x) for x in args]],stdout=f,stderr=subprocess.STDOUT,check=True)

configs=[]
for temp,seeds in [(0,[0]),(0.8,[0,1,2])]:
    for seed in seeds:
        for kind,voice in [('base','own-samantha'),('r2','own-samantha-v2-emo-r2')]:
            configs.append(('samantha',voice,f'sam_{kind}_t{temp}_s{seed}',temp,seed))
# Both new-reference profiles already exist. r6 is normalized and rotated;
# ln is the conservative zero-rotation ablation (not a smaller-gain claim).
for kind,voice in [('base','own-jarvis'),('r6','own-jarvis-v2-emo-r6'),('ln_no_rotation','own-jarvis-v2-emo-ln')]:
    configs.append(('jarvis',voice,f'jar_{kind}_t0_s0',0,0))
textdir=Q/'texts'; textdir.mkdir(exist_ok=True)
ids=[]
for line in (B/'test12.tsv').read_text().splitlines():
    ident,text=line.split('\t',1); ids.append(ident); (textdir/(ident+'.txt')).write_text(text)
assert len(ids)==12
for persona,voice,tag,temp,seed in configs:
    out=Q/tag; out.mkdir(exist_ok=True); lf=out/'audit.log'
    if (out/'COMPLETE.json').exists(): continue
    wait_idle()
    if any(out.glob('t*.wav')):
        raise RuntimeError('Partial directory requires explicit review, refusing overwrite: '+str(out))
    run([CB,'--intra-threads','2','batch','--dtype','fp32','--voice',voice,'--manifest',B/'test12.tsv','--out-dir',out,'--target',B/'targets'/f'{persona}.json','--temperature',temp,'--seed',seed,'--top-k','1000','--top-p','0.95','--repetition-penalty','1.2','--rep-unique','--punc-norm'],lf)
    wavs=[out/(i+'.wav') for i in ids]
    assert all(w.exists() for w in wavs)
    ref=B.parent.parent/'clip-store-own'/persona/'reference.wav'
    other=B.parent.parent/'clip-store-own'/('jarvis' if persona=='samantha' else 'samantha')/'reference.wav'
    S=B.parent/'scorers'
    commands=[
      [S/'emotion/target/release/galion-emotion','score','--threads','2','--compact','--ref',ref,*wavs,'--out',out/'emotion.json'],
      [S/'ecapa/target/release/galion-ecapa','--threads','2','--no-ceiling','--ref',ref,*wavs,'--out',out/'ecapa.json'],
      [S/'articulation/bin/galion-artic','profile','-j','2','--compact',*wavs,'--out',out/'artic.json'],
      [S/'quality/bin/galion-wer','--jobs','2','--text-dir',textdir,'--out',out/'wer.json',*wavs],
      [S/'quality/bin/galion-mos','--threads','2','--nmr',other,'--out',out/'mos.json',*wavs],
      [S/'quality/bin/galion-hygiene','--out',out/'hygiene.json',*wavs]]
    for command in commands: run(command,lf)
    (out/'COMPLETE.json').write_text(json.dumps(dict(persona=persona,voice=voice,temperature=temp,seed=seed,n=12)))
    log('COMPLETE '+tag)
log('ALL COMPLETE: results require matched aggregate review; no automatic voice promotion.')
