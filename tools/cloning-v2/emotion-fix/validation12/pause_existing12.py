import pathlib,subprocess,json,datetime,os
B=pathlib.Path('/workspace/swarm/voice-v2-sprint/cloning-v2/emotion-fix');os.chdir(B)
os.environ.update(OMP_NUM_THREADS='2',OPENBLAS_NUM_THREADS='2',MKL_NUM_THREADS='2',HF_HUB_OFFLINE='1')
Q=B/'validation12';S=B.parent/'scorers';tool=B.parent/'pipeline/cbx-v2-prosody/pause-tool/target/release/cbx-pauses'
ids=[line.split('\t')[0] for line in (B/'test12.tsv').read_text().splitlines()]
def run(cmd,log):
 print(datetime.datetime.now().astimezone().isoformat(),cmd,flush=True)
 with open(log,'a') as f: subprocess.run(['nice','-n','10',*[str(v) for v in cmd]],stdout=f,stderr=subprocess.STDOUT,check=True)
for persona,prefix in [('samantha','sd'),('jarvis','jd')]:
 ref=B.parent.parent/'clip-store-own'/persona/'reference.wav'; other=B.parent.parent/'clip-store-own'/('jarvis' if persona=='samantha' else 'samantha')/'reference.wav'
 for mode in ['off','on']:
  tag=f'{prefix}_old_paired_pause_{mode}';out=Q/tag;out.mkdir(exist_ok=True);log=out/'audit.log'
  for ident in ids:
   dest=out/(ident+'.wav')
   if not dest.exists(): run([tool,B.parent.parent/'own-clone-analysis/wav'/persona/(ident+'.wav'),dest,ref,Q/'texts'/(ident+'.txt'),out/(ident+'.pauses.txt')]+(['--punctuation-pauses'] if mode=='on' else []),log)
  wavs=[out/(i+'.wav') for i in ids]
  commands=[
   [S/'emotion/target/release/galion-emotion','score','--threads','2','--compact','--ref',ref,*wavs,'--out',out/'emotion.json'],
   [S/'ecapa/target/release/galion-ecapa','--threads','2','--no-ceiling','--ref',ref,*wavs,'--out',out/'ecapa.json'],
   [S/'articulation/bin/galion-artic','profile','-j','2','--compact',*wavs,'--out',out/'artic.json'],
   [S/'quality/bin/galion-wer','--jobs','2','--text-dir',Q/'texts','--out',out/'wer.json',*wavs],
   [S/'quality/bin/galion-mos','--threads','2','--nmr',other,'--out',out/'mos.json',*wavs],
   [S/'quality/bin/galion-hygiene','--out',out/'hygiene.json',*wavs]]
  for cmd in commands:
   dest=pathlib.Path(cmd[cmd.index('--out')+1])
   if not dest.exists():run(cmd,log)
  e=json.loads((out/'emotion.json').read_text()); ec=json.loads((out/'ecapa.json').read_text());assert e['summary']['n_files']==ec['n']==12
  row=dict(tag=tag,n=12,emotion=e['summary']['mean_emotion_distance'],ecapa=ec['mean'],pause_ratio=sum(f['proxies']['pause_ratio'] for f in e['files'])/12,valence=e['summary']['mean_avd']['valence'])
  (out/'COMPLETE.json').write_text(json.dumps(row));
  with open(B/'REPORT-12LINE.md','a') as f:f.write('\n### Additional existing-baseline paired pause test '+tag+'\n\n```json\n'+json.dumps(row,indent=2)+'\n```\nAll six Rust scorers completed; original 12 baseline WAVs were reused, not re-rendered.\n')
  print('COMPLETE',row,flush=True)
