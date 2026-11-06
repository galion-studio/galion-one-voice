#!/bin/bash
cd /workspace/swarm/voice-v2-sprint/cloning-v2/emotion-fix
while kill -0 3658430 2>/dev/null; do sleep 10; done
./run_cfg.sh samantha own-samantha sd_base_up dev4.tsv --temperature 0.8 --top-k 1000 --top-p 0.95 --rep-unique --punc-norm
for c in "own-jarvis:jd_base_up:--temperature 0.8 --top-k 1000 --top-p 0.95 --rep-unique --punc-norm" "own-jarvis-v2-emo-ln:jd_ln_up:--temperature 0.8 --top-k 1000 --top-p 0.95 --rep-unique --punc-norm" "own-jarvis-v2-emo-r6:jd_r6_up:--temperature 0.8 --top-k 1000 --top-p 0.95 --rep-unique --punc-norm" "own-jarvis-v2-emo-r10:jd_r10_up:--temperature 0.8 --top-k 1000 --top-p 0.95 --rep-unique --punc-norm" "own-jarvis:jd_base_grU:--rep-unique" "own-jarvis:jd_base_leg:"; do IFS=: read v t a <<< "$c"; ./run_cfg.sh jarvis $v $t dev4.tsv $a; done
echo QUEUE1_DONE
