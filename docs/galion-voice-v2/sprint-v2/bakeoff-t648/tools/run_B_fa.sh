#!/bin/bash
B=/workspace/swarm/voice-v2-sprint/bakeoff-t648; PT=$B/src/pocket-bin/pocket-tts-3.3.0-x86_64-unknown-linux-gnu/pocket-tts; LOCK=/workspace/swarm/voice-v2-sprint/latency-240/BENCH.lock
while pgrep -f tools/run_B.sh >/dev/null; do sleep 10; done
for cfg in "english 1" "english 2" "english 4" "english_18m_2026-10 1" "english_18m_2026-10 4"; do set -- $cfg
flock $LOCK bash -c "
  HF_HOME=$B/hf RAYON_NUM_THREADS=$2 $PT serve --host 127.0.0.1 --port 18765 --language $1 --default-voice alba > $B/logs/B-serve-$1-T$2.log 2>&1 & SP=\$!
  for i in \$(seq 60); do curl -s -o /dev/null http://127.0.0.1:18765/health && break; sleep 1; done
  python3 $B/tools/fa_client.py 18765 warm $B/text/s1.txt > /dev/null
  python3 $B/tools/fa_client.py 18765 'B $1 alba T$2' $B/text/s1.txt $B/text/s2.txt $B/text/s3.txt >> $B/logs/B-fa.jsonl
  kill \$SP; wait \$SP 2>/dev/null"
done
