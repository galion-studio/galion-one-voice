# Crying detector

Decision: **emotion2vec+ `sad`**, not EfficientAT crying. EfficientAT crying is logged only (`extras.efficientat_crying_logged`) and is never the decision value.

Fixture: `scorer-probe/results/SUMMARY.json` (original-voice probe, read-only). Presence threshold 0.20. Numbers are exact; they are not rounded.

## Why

EfficientAT caught **0 of 8** crying lines. The same lines that actually sounded like crying were rated sad at 0.99999+ by emotion2vec+. EfficientAT also fires on non-crying speech (Jarvis neu01 0.2096705436706543, Jarvis sad01 0.4669448733329773, Samantha joy02 0.2898308336734772, and others below).

## Detection on the 8 crying lines (new = e2v sad, old = EfficientAT max)

| Voice | id | e2v sad (new) | ≥ 0.20 | EfficientAT crying (old) | ≥ 0.20 |
|---|---|---:|:---:|---:|:---:|
| Samantha | cry01 | 0.9999972581863403 | yes | 0.12454459071159363 | no |
| Samantha | cry02 | 0.9999998807907104 | yes | 0.0726918876171112 | no |
| Samantha | cry03 | 1.0 | yes | 0.11070552468299866 | no |
| Samantha | cry04 | 6.036923890472001e-13 | no | 0.0740409791469574 | no |
| Jarvis | cry01 | 0.999992847442627 | yes | 0.17580771446228027 | no |
| Jarvis | cry02 | 0.3633507192134857 | yes | 0.06953170895576477 | no |
| Jarvis | cry03 | 0.0028845658525824547 | no | 0.1651308536529541 | no |
| Jarvis | cry04 | 6.440859512402142e-10 | no | 0.07920458912849426 | no |

| | New (e2v sad) | Old (EfficientAT) |
|---|---|---|
| Detected | **5 / 8** (0.625) | **0 / 8** (0.0) |
| Samantha | 3 / 4 (0.75), mean 0.7499992847444136 | 0 / 4, mean 0.09549574553966522 |
| Jarvis | 2 / 4 (0.5), mean 0.34155703328819526 | 0 / 4, mean 0.12241871654987335 |

cry04 on both voices, and Jarvis cry03, stay below 0.20 under emotion2vec+ sad as well: those synthesised lines did not register as sad (cry04 tops as neutral). That is a synthesis miss, not the detector inverting a clearly sad line.

## False positives on non-crying lines (decision ≥ 0.20)

New false-positive lines (e2v sad ≥ 0.20, group ≠ crying):

| Voice | id | group | e2v sad |
|---|---|---|---:|
| Samantha | sad01 | sad | 0.9999569654464722 |
| Samantha | sad02 | sad | 0.39345070719718933 |
| Samantha | sur01 | surprise | 0.7830357551574707 |
| Jarvis | sur02 | surprise | 0.9988592863082886 |

Old false-positive lines (EfficientAT crying ≥ 0.20, group ≠ crying):

| Voice | id | group | EfficientAT crying |
|---|---|---|---:|
| Jarvis | sed03 | seductive | 0.23735004663467407 |
| Jarvis | neu01 | neutral | 0.2096705436706543 |
| Samantha | joy02 | joyful | 0.2898308336734772 |
| Samantha | sad01 | sad | 0.33986419439315796 |
| Jarvis | sad01 | sad | 0.4669448733329773 |
| Jarvis | lau01 | laughter | 0.2699253559112549 |

| Scope | New FP | Old FP |
|---|---|---|
| All non-crying (38 lines) | **4 / 38** (0.10526315789473684), mean 0.0869376917731856 | **6 / 38** (0.15789473684210525), mean 0.1281289677870901 |
| Non-crying and not `sad` (34 lines) | **2 / 34** (0.058823529411764705), mean 0.0561783628790636 | **4 / 34** (0.11764705882352941), mean 0.11115378930288203 |
| Samantha non-crying (16) | 3 / 16 (0.1875) — two of those are the sad probes | 2 / 16 (0.125) |
| Jarvis non-crying (22) | 1 / 22 (0.045454545454545456) | 4 / 22 (0.18181818181818182) |

Using emotion2vec+ sad as crying is the right trade: it catches the crying lines that are actually sad, it no longer fires on Jarvis's neutral line, and the remaining false positives are other negative/surprise synthesised lines (plus the sad probes, which share the same trained class). Crying is **not** in either persona's judged set, so these scores are diagnostic unless a future persona lists crying.
