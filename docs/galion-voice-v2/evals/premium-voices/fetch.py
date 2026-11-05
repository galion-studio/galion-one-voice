import json,subprocess,hashlib,os
d=json.load(open('voices.json'))
want="en_US-ljspeech-high en_GB-cori-high en_US-kristin-medium en_GB-alba-medium en_US-libritts_r-medium en_US-libritts-high en_GB-vctk-medium en_US-norman-medium en_US-john-medium en_GB-aru-medium en_US-joe-medium en_US-ljspeech-medium".split()
B='https://huggingface.co/rhasspy/piper-voices/resolve/main/'
for k in want:
  for path,meta in d[k]['files'].items():
    if path.endswith('MODEL_CARD'): continue
    fn=os.path.basename(path)
    if not (os.path.exists(fn) and os.path.getsize(fn)==meta['size_bytes']):
      subprocess.run(['curl','-sL','--retry','3','-m','900','-o',fn,B+path],check=True)
    data=open(fn,'rb').read()
    md5=hashlib.md5(data).hexdigest(); sha=hashlib.sha256(data).hexdigest()
    ok = md5==meta['md5_digest']
    open(fn+'.sha256','w').write(sha+'\n')
    print(k,fn,len(data),'md5_ok' if ok else 'MD5_MISMATCH',sha,flush=True)
