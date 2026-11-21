<!-- SPDX-License-Identifier: MIT -->
<!-- Copyright (c) 2026 Galion Labs -->

# S03 ASR measurements

These blocks are the stdout of `s03-asr-eval` on this machine. The JSON files next to them keep every chunk time. The stdout was not retyped.

Host string from the runs: Linux x86_64, 4 CPUs, `Intel(R) Xeon(R) Processor` (the name this VM publishes). Vosk was a release build with rustc 1.83.0. Whisper was a release build with rustc 1.100.0 and `use gpu = 0`.

Audio: LJ Speech 1.1, clips LJ001-0001 through LJ001-0012, in dataset order. Public domain (Keith Ito / LibriVox). Mozilla Data Collective catalogs that dataset as CC0-1.0. 22050 Hz mono PCM, linearly resampled to 16000 Hz. 79.451 s after resample. 200 reference words after the normalizer. This is not a full-corpus WER.

From `eval/runs/vosk.json`, the one chunk over 80 ms is clip LJ001-0001, chunk index 4, 148067 microseconds.

## vosk small-en-us-0.15, 2 threads

```
measured_at_utc: 2026-11-21T02:51:58Z
host: linux x86_64 cpus=4 cpu=Intel(R) Xeon(R) Processor
rustc: rustc 1.83.0 (90b35a623 2024-11-26)
test_set: LJ Speech 1.1 clips LJ001-0001 through LJ001-0012, in dataset order. Not the full 24 hours.
license: Public domain (Keith Ito / LibriVox, Linda Johnson). Mozilla Data Collective catalogs the same set as CC0-1.0.
backend: vosk:vosk-model-small-en-us-0.15
model: /agent/repos/galion-one-voice/voice-stack/s03-asr/weights/asr/vosk-model-small-en-us-0.15
threads: 2
chunk_ms: 80
load_ms: 280.806
clips: 12
reference_words: 200 errors: 10 (sub 8 del 0 ins 2)
micro_wer: 0.050000
audio_s: 79.451
wall_s: 4.728
rtf: 0.059510
chunks: 999
chunk_wall_ms_p50: 0.307
chunk_wall_ms_p95: 18.116
chunks_over_80ms: 1
finish_ms_p50: 12.499
first_partial_wall_ms_p50: 24.262
first_partial_audio_ms_p50: 640.000
LJ001-0001 wer=0.000000 (0/27) audio_ms=9655 wall_ms=1004.6 rtf=0.104053 first_partial_wall_ms=260.792 first_partial_audio_ms=880 hyp=printing in the only sense with which we are at present concerned differs from most if not from all the arts and crafts represented in the exhibition
LJ001-0002 wer=0.000000 (0/4) audio_ms=1899 wall_ms=166.0 rtf=0.087381 first_partial_wall_ms=70.789 first_partial_audio_ms=640 hyp=in being comparatively modern
LJ001-0003 wer=0.083333 (2/24) audio_ms=9666 wall_ms=658.6 rtf=0.068129 first_partial_wall_ms=27.650 first_partial_audio_ms=400 hyp=for although the chinese took impressions from wood blocks engraved in relief for centuries before the would cutters of the netherlands by a similar process
LJ001-0004 wer=0.000000 (0/14) audio_ms=5138 wall_ms=262.2 rtf=0.051031 first_partial_wall_ms=40.377 first_partial_audio_ms=880 hyp=produced the block books which were the immediate predecessors of the true printed book
LJ001-0005 wer=0.000000 (0/25) audio_ms=8110 wall_ms=444.0 rtf=0.054740 first_partial_wall_ms=11.124 first_partial_audio_ms=400 hyp=the invention of movable metal letters in the middle of the fifteenth century may justly be considered as the invention of the art of printing
LJ001-0006 wer=0.214286 (3/14) audio_ms=5684 wall_ms=334.8 rtf=0.058895 first_partial_wall_ms=24.262 first_partial_audio_ms=640 hyp=and it is worth mentioned in passing that has an example of fine typographic
LJ001-0007 wer=0.052632 (1/19) audio_ms=8389 wall_ms=397.5 rtf=0.047375 first_partial_wall_ms=10.348 first_partial_audio_ms=400 hyp=the earliest book printed with movable types the guttenberg or forty two line bible of about fourteen fifty five
LJ001-0008 wer=0.250000 (1/4) audio_ms=1783 wall_ms=97.3 rtf=0.054584 first_partial_wall_ms=23.722 first_partial_audio_ms=400 hyp=that's never been surpassed
LJ001-0009 wer=0.000000 (0/19) audio_ms=7553 wall_ms=356.1 rtf=0.047144 first_partial_wall_ms=19.973 first_partial_audio_ms=640 hyp=printing then for our purpose may be considered as the art of making books by means of movable types
LJ001-0010 wer=0.111111 (2/18) audio_ms=8819 wall_ms=378.9 rtf=0.042968 first_partial_wall_ms=28.372 first_partial_audio_ms=880 hyp=now as all books not primarily intended as picture books consist principally of types composed to form letter press
LJ001-0011 wer=0.066667 (1/15) audio_ms=4511 wall_ms=252.8 rtf=0.056030 first_partial_wall_ms=9.653 first_partial_audio_ms=400 hyp=it is of the first importance that the letter used should be fine and form
LJ001-0012 wer=0.000000 (0/17) audio_ms=8238 wall_ms=375.3 rtf=0.045555 first_partial_wall_ms=37.297 first_partial_audio_ms=640 hyp=especially as no more time is occupied or cost incurred in casting setting or printing beautiful letters
```

## whisper.cpp tiny.en, 4 threads, CPU

```
measured_at_utc: 2026-11-21T02:53:48Z
host: linux x86_64 cpus=4 cpu=Intel(R) Xeon(R) Processor
rustc: rustc 1.100.0 (0ef1bc386 2026-11-09)
test_set: LJ Speech 1.1 clips LJ001-0001 through LJ001-0012, in dataset order. Not the full 24 hours.
license: Public domain (Keith Ito / LibriVox, Linda Johnson). Mozilla Data Collective catalogs the same set as CC0-1.0.
backend: whisper:ggml-tiny.en.bin
model: weights/whisper/ggml-tiny.en.bin
threads: 4
chunk_ms: 80
load_ms: 110.751
clips: 12
reference_words: 200 errors: 12 (sub 8 del 3 ins 1)
micro_wer: 0.060000
audio_s: 79.451
wall_s: 5.605
rtf: 0.070543
chunks: 999
chunk_wall_ms_p50: 0.000
chunk_wall_ms_p95: 0.001
chunks_over_80ms: 0
finish_ms_p50: 466.048
first_partial_wall_ms_p50: none
first_partial_audio_ms_p50: none
LJ001-0001 wer=0.037037 (1/27) audio_ms=9655 wall_ms=487.9 rtf=0.050530 first_partial_wall_ms=none first_partial_audio_ms=none hyp=Printing, in the only sense with which we are at present concern, differs from most, if not from all the arts and crafts represented in the exhibition.
LJ001-0002 wer=0.000000 (0/4) audio_ms=1899 wall_ms=434.3 rtf=0.228616 first_partial_wall_ms=none first_partial_audio_ms=none hyp=in being comparatively modern.
LJ001-0003 wer=0.083333 (2/24) audio_ms=9666 wall_ms=476.9 rtf=0.049336 first_partial_wall_ms=none first_partial_audio_ms=none hyp=For although the Chinese took impressions from wood blocks engraved in relief for centuries before the wood cutters of the Netherlands by a similar process,
LJ001-0004 wer=0.000000 (0/14) audio_ms=5138 wall_ms=507.0 rtf=0.098663 first_partial_wall_ms=none first_partial_audio_ms=none hyp=produced the block books, which were the immediate predecessors of the true printed book.
LJ001-0005 wer=0.040000 (1/25) audio_ms=8110 wall_ms=482.6 rtf=0.059501 first_partial_wall_ms=none first_partial_audio_ms=none hyp=The invention of movable metal letters in the middle of the 15th century may justly be considered as the invention of the art of printing.
LJ001-0006 wer=0.071429 (1/14) audio_ms=5684 wall_ms=455.4 rtf=0.080116 first_partial_wall_ms=none first_partial_audio_ms=none hyp=And it is worth mentioning in passing that as an example of fine typography
LJ001-0007 wer=0.315789 (6/19) audio_ms=8389 wall_ms=470.9 rtf=0.056133 first_partial_wall_ms=none first_partial_audio_ms=none hyp=The earliest book printed with movable types, the Gudenberg, or 42-line Bible, of about 1455.
LJ001-0008 wer=0.000000 (0/4) audio_ms=1783 wall_ms=432.4 rtf=0.242463 first_partial_wall_ms=none first_partial_audio_ms=none hyp=has never been surpassed.
LJ001-0009 wer=0.000000 (0/19) audio_ms=7553 wall_ms=487.4 rtf=0.064525 first_partial_wall_ms=none first_partial_audio_ms=none hyp=Printing then for our purpose may be considered as the art of making books by means of movable types.
LJ001-0010 wer=0.000000 (0/18) audio_ms=8819 wall_ms=466.1 rtf=0.052850 first_partial_wall_ms=none first_partial_audio_ms=none hyp=Now, as all books, not primarily intended as picture books, consist principally of types composed to form letterpress.
LJ001-0011 wer=0.066667 (1/15) audio_ms=4511 wall_ms=449.5 rtf=0.099635 first_partial_wall_ms=none first_partial_audio_ms=none hyp=It is of the first importance that the letter used should be fine and form.
LJ001-0012 wer=0.000000 (0/17) audio_ms=8238 wall_ms=454.3 rtf=0.055138 first_partial_wall_ms=none first_partial_audio_ms=none hyp=especially as no more time is occupied or cost incurred in casting, setting or printing beautiful letters.
```

## whisper.cpp base.en, 4 threads, CPU

```
measured_at_utc: 2026-11-21T02:54:07Z
host: linux x86_64 cpus=4 cpu=Intel(R) Xeon(R) Processor
rustc: rustc 1.100.0 (0ef1bc386 2026-11-09)
test_set: LJ Speech 1.1 clips LJ001-0001 through LJ001-0012, in dataset order. Not the full 24 hours.
license: Public domain (Keith Ito / LibriVox, Linda Johnson). Mozilla Data Collective catalogs the same set as CC0-1.0.
backend: whisper:ggml-base.en.bin
model: weights/whisper/ggml-base.en.bin
threads: 4
chunk_ms: 80
load_ms: 198.113
clips: 12
reference_words: 200 errors: 12 (sub 7 del 3 ins 2)
micro_wer: 0.060000
audio_s: 79.451
wall_s: 10.484
rtf: 0.131955
chunks: 999
chunk_wall_ms_p50: 0.000
chunk_wall_ms_p95: 0.001
chunks_over_80ms: 0
finish_ms_p50: 869.036
first_partial_wall_ms_p50: none
first_partial_audio_ms_p50: none
LJ001-0001 wer=0.000000 (0/27) audio_ms=9655 wall_ms=903.7 rtf=0.093604 first_partial_wall_ms=none first_partial_audio_ms=none hyp=Printing, in the only sense with which we are at present concerned, differs from most if not from all the arts and crafts represented in the exhibition
LJ001-0002 wer=0.000000 (0/4) audio_ms=1899 wall_ms=825.8 rtf=0.434761 first_partial_wall_ms=none first_partial_audio_ms=none hyp=in being comparatively modern.
LJ001-0003 wer=0.083333 (2/24) audio_ms=9666 wall_ms=881.7 rtf=0.091213 first_partial_wall_ms=none first_partial_audio_ms=none hyp=For although the Chinese took impressions from wood blocks engraved in relief for centuries before the wood cutters of the Netherlands by a similar process,
LJ001-0004 wer=0.000000 (0/14) audio_ms=5138 wall_ms=925.7 rtf=0.180142 first_partial_wall_ms=none first_partial_audio_ms=none hyp=produced the block books, which were the immediate predecessors of the true printed book,
LJ001-0005 wer=0.040000 (1/25) audio_ms=8110 wall_ms=894.0 rtf=0.110228 first_partial_wall_ms=none first_partial_audio_ms=none hyp=The invention of movable metal letters in the middle of the 15th century may justly be considered as the invention of the art of printing.
LJ001-0006 wer=0.071429 (1/14) audio_ms=5684 wall_ms=865.0 rtf=0.152171 first_partial_wall_ms=none first_partial_audio_ms=none hyp=And it is worth mentioning in passing that as an example of fine typography
LJ001-0007 wer=0.263158 (5/19) audio_ms=8389 wall_ms=903.5 rtf=0.107700 first_partial_wall_ms=none first_partial_audio_ms=none hyp=The earliest book, printed with movable types, the Gutenberg, or 42-line Bible of about 1455,
LJ001-0008 wer=0.000000 (0/4) audio_ms=1783 wall_ms=818.5 rtf=0.458969 first_partial_wall_ms=none first_partial_audio_ms=none hyp=has never been surpassed.
LJ001-0009 wer=0.000000 (0/19) audio_ms=7553 wall_ms=869.1 rtf=0.115057 first_partial_wall_ms=none first_partial_audio_ms=none hyp=Printing then, for our purpose, may be considered as the art of making books by means of movable types.
LJ001-0010 wer=0.111111 (2/18) audio_ms=8819 wall_ms=881.4 rtf=0.099938 first_partial_wall_ms=none first_partial_audio_ms=none hyp=Now, as all books, not primarily intended as picture books, consist principally of types composed to form letter press,
LJ001-0011 wer=0.066667 (1/15) audio_ms=4511 wall_ms=848.7 rtf=0.188118 first_partial_wall_ms=none first_partial_audio_ms=none hyp=It is of the first importance that the letter used should be fine and form.
LJ001-0012 wer=0.000000 (0/17) audio_ms=8238 wall_ms=866.6 rtf=0.105191 first_partial_wall_ms=none first_partial_audio_ms=none hyp=especially as no more time is occupied or cost incurred in casting, setting or printing beautiful letters.
```
