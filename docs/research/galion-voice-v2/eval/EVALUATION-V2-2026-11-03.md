> **Research synthesis dated 2026-11-03 (Europe/Zagreb).** This page is desk synthesis, copied into the repo as docs only. It is not a measurement, not a LOCKS change, and not product code.
>
> **LOCKS.toml overrides any conflicting domain text:** no whisper.cpp, no GGML-family containers, no in-tree ORT, `paid_tiers=false`. `chrome_allowed=false`. Consent stays deny-default. Agents do not edit LOCKS.toml.
>
> Owner Maciej Grajczyk authorized this docs-only landing. Sentences below that say the tree is local-only, or "do not push", record the pre-landing state. Landing these documents does not ratify open decisions O1–O12 and does not authorize implementation.

# Galion Voice V2 — Evaluation layer

Europe/Zagreb · 2026-11-03 · Maciej Grajczyk preferences applied.
Spelling: **Galion**. Local research tree; **not pushed** until you approve.

Sources: live probe `127.0.0.1:7878` + ngrok, GitHub `main` `9a5ebe1` (PR #3), `docs/galion-voice-v2/GALION-VOICE-V2.md`, local `/workspace/galion-voice-code` (no `.git`).

## 1. Completion percentages

| Item | % | Basis |
|---|---:|---|
| **V1 stack** (one-dot, heartbeat, PhoneLane, /turn/reply, Piper A21 path, ngrok edge) | **82%** | E2E phone loop live; ASR not real; LLM canned; VPS not live |
| **V2-S1 Process bus** | **12%** | Spec + cancel seed; typed bus not in crates |
| **V2-S2 Orb / graphic engine** | **55%** | Spring-damper on `v1-screen.html`; not in `galion-render`; no shader |
| **V2-S3 Secure edge** | **68%** | Headers/CSP/loopback/tests; CORS still `*`; no VPS TLS |
| **V2-S4 Real EN ASR** | **5%** | Policy + interim placeholder only; letter A/B/C unset |
| **V2-S5 Reply intelligence** | **28%** | CannedLlm on phone; no `.gq` student; emotion unwired |
| **V2-S6 Speech runtime** | **42%** | Piper/flite path works; warm process / counsel open |
| **V2-S7 CI/CD track** | **62%** | Gates green; PR #3 merged; auto-deploy blocked (no SSH) |
| **V2-S8 Branding & docs** | **78%** | Brand + README on GitHub `main`; local README still V1-stale |
| **V2-S9 Student / Open Mind** | **10%** | Config stubs only; no weights; no train |
| **V2-S10 Gates R0–R3** | **18%** | Dry-run docs; PENDING without SSH |
| **V2-S11 Presence** | **38%** | Harness exists; phone barge-in not presence-spec’d |
| **V2-S12 Consent / Rika on bus** | **32%** | Rika + deny defaults work; not bus-enveloped |
| **V2-S13 Multilingual hold** | **95%** | Intentional EN-only hold (done *as hold*) |
| **Plan V5** (ops/deploy doc + track) | **48%** | `PLAN-V5.md` landed; execute/SSH human-blocked |
| **Live demo** | **70%** | Serving; hears speech→Piper WAV; does not understand words |
| **Branding applied** | **75%** | Repo brand/README yes; live page still one-dot (correct) |
| **GitHub state** | **88%** | `main`=`9a5ebe1`; PRs #1–#3 merged; 0 open; local tree unpushed delta drift |
| **Overall V2 program** | **36%** | Weighted: plans + phone + orb foundation in; bus/ASR/VPS missing |

## 2. What works

- Demo process on `127.0.0.1:7878` (HTTP 200); ngrok `https://unreeling-ventricle-deodorant.ngrok-free.dev`
- One-dot UI; mic → `POST /mic`; turn → `POST /turn` → worker → `GET /reply/<id>.wav`
- With `GALION_ASR=interim` + canned LLM + A21/flite: **real spoken replies** (intent table), not only “I heard you” *when* interim+canned are set — but transcripts are **length buckets**, not recognition
- Security headers / loopback bind / phone pipeline check scripts (on GitHub tree)
- Spring orb on phone page (listen/think/speak motion) per V2 land
- GitHub: PR [#3](https://github.com/galion-studio/galion-one-voice/pull/3) merged; V2 plan, Plan V5, brand, phone path on `main`

## 3. What does not work

- **Real English ASR** (default refuses; `interim` does not hear words)
- **VPS deploy** Titanaxe `51.38.137.193` (SSH key human)
- **Typed process bus** (still mutex/channel soup + docs)
- **Orb engine in `galion-render`** (page-only spring)
- **Warm Piper / counsel-clean A21 LOCKS text**; espeak-ng GPL hold
- **`.gq` reply model**; student untrained
- **Local `/workspace/galion-voice-code` has no `.git`** — drifts from `main`; local README still describes StubAsr/MockLlm era
- **R0–R3 live** on VPS
- Double public edge (ngrok + cloudflared both pointed at :7878) — redundant risk

## 4. Removal recommendations (first principles)

| Candidate | Keep / Remove / Shrink | Why |
|---|---|---|
| `GALION_ASR=interim` as product story | **Shrink** → label-only + sunset | Does not earn recognition claim; without it E2E breaks until A/B/C — keep as *named* scaffold only |
| `MockLlm` echo on phone | **Remove from phone default** | Echo teaches “you said:” robotics; CannedLlm already earns place |
| Second tunnel (`cloudflared` alongside ngrok) | **Remove one** | One public HTTPS; two edges double failure + CORS noise |
| PNG / terminal re-skin before bus | **Defer** | Orb in page is enough until G4 re-bless; don’t split engines |
| Engagement / dwell metrics in presence | **Keep rejected** | Already forbidden; reopening breaks LOCKS intent |
| Vendoring GGML / whisper.cpp “just for demo” | **Never add** | Breaks LOCKS; option A Vosk or C tensor path instead |
| Freemium Local/Plus/Ultra in README | **Keep out of repo** | `paid_tiers=false` lock; memory ≠ product lock |
| Extra chrome / menus on phone | **Never** | `chrome_allowed=false`; motion > UI chrome |
| Multilingual canned lines | **Hold** | EN-only until owner plan |
| Tone-only “reply” as success | **Reject as product** | Caller wants words; tones only as honest error signal |

## 5. Admin-recommended preferences (from your history)

Locked recommendations for V2 build order:

1. **Voice-first real-time** — never stall the 80 ms heartbeat; all heavy work on turn worker / future bus.
2. **Natural conversation** — barge-in cancel token → real ASR (letter **A Vosk** recommended) → Canned then `.gq` → warm Piper; prosody only with consent.
3. **Open-source / local-first** — Vosk A, Piper A21 out-of-process, no GGML; Rust tensor path for C long-term.
4. **Low latency** — streaming partials later; first win is warm TTS + cancel supersede (already seeded).
5. **Brand = Galion Voice** — one orb mark, lock colours, Maciej Grajczyk sign-off; no FRXDEL STUDIOS in product docs.
6. **Security** — loopback app, TLS at edge, ephemeral RAM turns, GDPR retention short, consent deny-by-default.
7. **Honesty** — never call interim “recognition”; never call spring page V3.

**Next sector to build:** **V2-S1 Process bus** (unblocks ASR sidecar + barge-in), in parallel **prep V2-S4 letter A** (research + scaffold, no LOCKS edit without you).

## 6. In progress after this eval

- Research swarm: 10 domains → `/workspace/galion-voice-research/domains/` (no GitHub push until you say yes)
- Implementation: V2-S1 bus types + tests via cloud agent PR (reviewable; merge only if you want)
- Demo stays up on the ngrok URL above

— Maciej Grajczyk

