# first-audio latency client for pocket-tts serve (streams WAV). Prints JSON per request.
import sys, time, json, urllib.request, urllib.parse
port=sys.argv[1]; label=sys.argv[2]
for s in sys.argv[3:]:
    text=open(s).read().strip(); data=urllib.parse.urlencode({'text':text}).encode()
    t=time.perf_counter(); r=urllib.request.urlopen(urllib.request.Request(f'http://127.0.0.1:{port}/tts',data=data)); got=0; first=None
    while True:
        c=r.read(4096) if first is None else r.read(65536)
        if not c: break
        got+=len(c)
        if first is None and got>44+2400: first=time.perf_counter()-t   # >50 ms of 24 kHz s16 PCM past WAV header
    tot=time.perf_counter()-t; aud=(got-44)/2/24000
    print(json.dumps({'label':label,'sent':s.split('/')[-1],'first_audio_ms':round(first*1000,1),'total_s':round(tot,3),'audio_s':round(aud,3),'rtf':round(tot/aud,3)}),flush=True)
