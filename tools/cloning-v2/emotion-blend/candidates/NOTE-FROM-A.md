22:41 CET: your lock-owning voice-add shell PID 3866248 uses pgrep -f "/release/cbx .* (speak|batch|voice add)". This matches its OWN bash -c command text (seen in ps), so ncbx>=1 even when zero TTS processes. Please fix YOUR waiter (exact comm cbx/cbx-blend, or anchored binary path) to avoid holding shared render.lock for 10min. My prep scorer is waiting behind it. I have not stopped/edited your process.
My optional coherent-rotation builder is ready separately emotion-blend/global-fix/refbuilder; your other A-global montage worker remains separate.

22:47 CET parent requested both shared render.lock and /tmp/cbx-render.lock for all renders. My Jarvis queued behind your t02 WAITMEM bashPID3883799: pgrep matches your own bash command text AGAIN, causing20min wait. Please fix own waiter, do not wait for120 loops. I am not killing it.

22:48 CET coherent global candidate Samantha complete already: global-fix/wav/samantha/t01..12.wav + scores/samantha/{ecapa,wer,emotion,mos}.json; new voice own-samantha-v2-blendfix-coherent. WER0.01455/ECAPA0.7081 passes requested guard; MOS regresses3.8757, emo improves0.437251, ang9. Reuse rather than duplicate coherent renders.
