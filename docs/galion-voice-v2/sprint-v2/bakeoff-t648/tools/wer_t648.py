# Eval only: intelligibility WER with local vosk-model-small-en-us-0.15 (libvosk C API, same as clone-src/vosk_words.py).
# usage: python wer_t648.py out.json wav...   (text picked from filename s1/s2/s3)
import ctypes, json, sys, subprocess, re, os
R='/workspace/swarm/voice-v2-sprint/repo/weights/asr/'; B='/workspace/swarm/voice-v2-sprint/bakeoff-t648'
L=ctypes.CDLL(R+'libvosk/libvosk.so'); L.vosk_set_log_level(-1)
L.vosk_model_new.restype=ctypes.c_void_p; L.vosk_model_new.argtypes=[ctypes.c_char_p]
L.vosk_recognizer_new.restype=ctypes.c_void_p; L.vosk_recognizer_new.argtypes=[ctypes.c_void_p, ctypes.c_float]
L.vosk_recognizer_accept_waveform.argtypes=[ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
L.vosk_recognizer_free.argtypes=[ctypes.c_void_p]
for f in ('vosk_recognizer_result','vosk_recognizer_final_result'):
    getattr(L,f).restype=ctypes.c_char_p; getattr(L,f).argtypes=[ctypes.c_void_p]
m=L.vosk_model_new((R+'vosk-model-small-en-us-0.15').encode())
norm=lambda s: re.sub(r"[^a-z' ]"," ",s.lower()).split()
def wer(ref,hyp):
    d=list(range(len(hyp)+1))
    for i,a in enumerate(ref,1):
        p=d[:]; d[0]=i
        for j,b in enumerate(hyp,1): d[j]=min(p[j]+1,d[j-1]+1,p[j-1]+(a!=b))
    return d[-1]/len(ref)
out={}
for p in sys.argv[2:]:
    s=re.search(r'(s[123])',os.path.basename(p)).group(1); ref=norm(open(f'{B}/text/{s}.txt').read())
    pcm=subprocess.check_output(['ffmpeg','-v','error','-i',p,'-ac','1','-ar','16000','-f','s16le','-'])
    r=L.vosk_recognizer_new(m,16000.0); txt=[]
    for i in range(0,len(pcm),8000):
        if L.vosk_recognizer_accept_waveform(r,pcm[i:i+8000],len(pcm[i:i+8000])): txt.append(json.loads(L.vosk_recognizer_result(r))['text'])
    txt.append(json.loads(L.vosk_recognizer_final_result(r))['text']); L.vosk_recognizer_free(r)
    hyp=norm(' '.join(txt)); out[p]={'wer':round(wer(ref,hyp),3),'hyp':' '.join(hyp)}
json.dump(out,open(sys.argv[1],'w'),indent=1); print(len(out),'done')
