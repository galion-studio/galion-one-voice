# A21 — Counsel memo: weights path (ready for review)

*Galion Voice V4 · W20-12 · 2026-11-02, 12:40 CET (Europe/Zagreb, UTC+1) · LOCAL DRAFT. Not sent to counsel.*
*Prepared for: counsel, via the owner (Maciej Grajczyk / FRXDEL STUDIOS). **Not legal advice.** This memo asks for a written opinion; it does not claim any rights.*

Source drafts: `galion-voice-20/v4/agents/A21-licence-weights.md` (full A21 memo), `A21-STATUS.md`, `plans/14-LICENCE-WEIGHTS.md`, `plans/09-STUDENT-ADAPTATION.md`, `BACKLOG.md` #9.

## 1. What we are asking for
1. **Written sign-off on Path P-A** (PersonaPlex used for outputs only, §3).
2. Confirmation that the **CC-BY-4.0 attribution checklist** (§4) is enough for Kyutai Mimi/Moshi/Moshiko.
3. Answers to the **open questions** (§5), mainly the W4 student-weights licence.

BACKLOG #9 (P1, BLOCKED) is waiting on this. It blocks #10 (A7 `gq-pack` + G5), #5 (real Mimi weights) and #13, and it gates every weight on disk for #11 (adapt, don't train) and #20.

## 2. Facts
- **Code:** Perpetual Open MIT, © 2026 Maciej Grajczyk. This covers code only; weights are excluded.
- **No weights in the repo** (S10). Weights are only linked, and each link needs a licence row (`docs/WEIGHTS.md` / `weights/LICENCES.toml`).
- **Current plan: no training.** We load published Mimi + Moshiko as they are, apply int8 post-training quantisation to `.gq`, use SenseVoice-Small as an inbound SER reference (adapter only, no fine-tune), and use PersonaPlex outputs as eval data only. A knowledge-distillation (KD) student (A8) is a *later* option.
- **The loader refuses real weights today** (`galion-mimi` `NotImplementedStub`). The format allowlist is `{safetensors, gq}`.
- **No DRM or licence keys** (K4).

## 3. Path P-A: PersonaPlex outputs only (recommended)
PersonaPlex weights (`nvidia/personaplex-7b-v1`) are under the **NVIDIA Open Model License (OML)** and gated on Hugging Face. The Moshi base parts are also CC-BY-4.0.

| Path | Resulting weights licence | Status |
|---|---|---|
| **P-A** Fresh init. Use only PersonaPlex *generated audio/tokens* plus public/CC data, for eval and possibly KD later | Our own licence (W4) | **Recommended. Sign-off requested** |
| P-B Init, prune or layer-copy from PersonaPlex | NVIDIA OML (derivative), never "MIT" | Allowed but not chosen |
| P-C Redistribute PersonaPlex weights | n/a | **No** (link only) |
| P-D Runtime loading of weights the user downloaded | User's OML | OK. Loader shows the notice; no auto-download that skips the click-through |

**Conditions we will keep under P-A:**
- [ ] Log the teacher model version, the OML version/date accepted, and the generation date.
- [ ] Do not bypass or disable guardrails while generating (OML termination risk).
- [ ] Add a courtesy NOTICE line, "teacher outputs from PersonaPlex (NVIDIA OML)". This is not a licence claim.
- [ ] Re-check the OML text whenever we generate (NVIDIA may update it).

**Questions for counsel on P-A:** (a) The OML says outputs are not Derivative Models. Does that hold when outputs are used to *train* a model? (b) Do OML use restrictions carry over to a P-A student? (c) Is the version/date log enough as evidence?

## 4. CC-BY-4.0 attribution checklist (Kyutai Mimi / Moshi / Moshiko)
CC-BY-4.0 §3(a) applies to each weight we share or adapt.
- [ ] **Creator** named: "Kyutai (kyutai-labs)"
- [ ] **Copyright notice** kept, if upstream supplies one: TODO, copy it verbatim from the HF model card
- [ ] **Licence notice + link:** https://creativecommons.org/licenses/by/4.0/
- [ ] **Disclaimer notice** kept (CC-BY §5)
- [ ] **URI** to the source: TODO, real HF URI (§6)
- [ ] **Modification stated:** "not modified" for raw weights; "**Modified: converted to .gq int8 by Galion Voice**" after quantisation
- [ ] **No extra restrictive terms or DRM** (§2(a)(5)(B)). K4 covers this.
- [ ] Where it appears: `NOTICE`, `.gq` header (`licence`, `attribution`, `source_uri`, `modified`, `sha256_upstream`), console `--about` whenever a CC-BY weight is loaded, and a credit line in docs and the demo video ("Mimi/Moshi © Kyutai, CC-BY-4.0")

**Question for counsel:** Is a `.gq` int8 re-quantisation "Adapted Material"? We plan to treat it as modified either way.

## 5. Open questions for counsel and the owner
1. **W4 student-weights licence:** MIT or CC-BY-4.0, for P-A weights we trained ourselves. MIT is simpler and matches the code. CC-BY-4.0 is a better fit for data/weights and requires attribution. Owner picks; counsel confirms it is compatible with P-A.
2. **A5:** Replace `LICENSE` with verbatim SPDX MIT and move the "perpetual" intent and the weights exclusion into `NOTICE`? (Drafts are in the A21 memo §3. Not applied.)
3. **A4/K8 copyright line:** Keep © 2026 Maciej Grajczyk. The OOM "©3000 Galion" notice uses a future year. Is "Galion / galion-studio" a legal entity, or a trade name of Maciej Grajczyk or FRXDEL STUDIOS?
4. **OOM code crossing (C1):** Who licenses OOM code into an MIT repo? Until this is answered, no OOM code goes into the public repo.
5. **Contributors:** DCO or CLA?
6. **Croatian/EU moral rights** vs the "perpetual/irrevocable" wording.
7. **SenseVoice-Small:** What is its licence, and are there conditions on inference-only use? TODO: read the upstream model card. We have not verified it, so this memo makes no assertion.

## 6. Weight licence table: placeholders (do not fill without verifying)
All URIs and hashes are **TODO**. The owner fills them from the real files. No hash has been computed or invented.

| id | model | upstream URI | licence | holder | modified | redistribute | sha256 |
|---|---|---|---|---|---|---|---|
| W1 | Mimi codec | TODO (hf.co/kyutai/…) | CC-BY-4.0 | Kyutai | yes (.gq) | no (link) | TODO |
| W2 | Moshiko | TODO (hf.co/kyutai/…) | CC-BY-4.0 | Kyutai | no / .gq TBD | no | TODO |
| W3 | SenseVoice-Small | TODO (hf.co/FunAudioLLM/…?) | TODO, verify | TODO | no | no | TODO |
| W4 | Galion student (P-A, later) | n/a (ours) | TODO: MIT or CC-BY-4.0 | TODO | n/a | yes | TODO |
| — | PersonaPlex | hf.co/nvidia/personaplex-7b-v1 (gated) | NVIDIA OML | NVIDIA | n/a, outputs only | **no** | n/a |

## 7. After sign-off
Owner and counsel sign off. The owner places a weight locally and fills §6. Then A7 `gq-pack` + G5, then the A16 CI licence checks (no weight files, a licence row for every link, NOTICE present).

---

© GOATSI — Galion Open Automated Technarchy Society Inc. All rights reserved and claims reserved. · CC0 voices: you must tick the consent box agreeing to the CC0 licence before using any CC0 voice; use is at your own responsibility. · No cookies. · De-Googled: no Google services. · All current Galion builds are open source under the Galion perpetual open MIT licence.
