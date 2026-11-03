> **Research synthesis dated 2026-11-03 (Europe/Zagreb).** This page is the domain brief, copied into the repo as docs only. It is not a measurement, not a LOCKS change, and not product code. Citations below are only those in the source brief. None were added on landing.
>
> **LOCKS.toml overrides any conflicting domain text:** no whisper.cpp, no GGML-family containers, no in-tree ORT, `paid_tiers=false`. `chrome_allowed=false`. Consent stays deny-default. Agents do not edit LOCKS.toml.

# Domain 07 — Security & privacy for voice agents (Galion Voice V2)

Date: 2026-11-03 (Europe/Zagreb). Scope: loopback app → ngrok interim → Caddy/VPS later; ephemeral turn buffers; GDPR/EU + Croatia; consent deny-by-default; voice biometrics/cloning.
Baseline: eval/EVALUATION-V2-2026-11-03.md, **V2-S3 Secure edge = 68%** ("Headers/CSP/loopback/tests; CORS still `*`; no VPS TLS").

## 1. Legal frame (EU / Croatia)

| Topic | Finding | Galion implication |
|---|---|---|
| Voice = personal data | EDPB Guidelines 02/2021 on VVAs: recordings, transcripts, request history are personal data; ePrivacy Art. 5(3) applies to accessing data on the device (consent unless strictly necessary for the requested service). [S1] | Mic capture for a user-started turn is "strictly necessary"; anything beyond (retention, training, analytics) needs separate opt-in. |
| Voice → biometric only by purpose | Voice becomes Art. 4(14)/Art. 9 biometric data when processed to **uniquely identify/authenticate** a person; EDPB says only **explicit consent** (Art. 9(2)(a)) fits, with a non-biometric alternative, and voiceprints should stay on-device. [S1][S2] | Do **not** build speaker ID/voiceprints in V2. If ever added: explicit opt-in, local-only template, PIN alternative. |
| Emotion inference | Covered in domain 06 (AI Act Art. 5(1)(f) bans emotion recognition in workplace/education). | Keep 06 deny-default. |
| AI Act Art. 50 (applies 2 Aug 2026) | Users must be told they're talking to an AI (Art. 50(1)); synthetic audio must be machine-readably marked (50(2), grace to 2 Dec 2026 only for systems on market before 2 Aug 2026); deepfake audio resembling real persons needs an audible label at first exposure. [S3][S4][S5] | Spoken/visible "AI assistant" disclosure on first turn; never clone a real person's voice; evaluate watermark for TTS output before 2 Dec 2026. |
| Croatia | AZOP is the sole SA; enforces GDPR + Act on Implementation of GDPR (NN 42/2018). 97 fines / €10.5M total; 2025: 13 fines / €6.73M; €4.5M telco fine (Nov 2025) for third-country transfers w/o SCCs/TRA and no processor checks. Final decisions on special-category data are published **non-anonymised**. [S6][S7][S8][S9] | Any non-EU processor (ngrok, cloud ASR/LLM/TTS) needs DPA + SCCs + TRA documented. Biometric misuse = public naming. |

## 2. Threat model (concrete)

1. **Exposed loopback via tunnel.** HTTPS on ngrok authenticates the server, not the caller; anyone with the URL reaches the mic/agent. ngrok docs explicitly: add auth (Basic, OAuth/OIDC, IP restrict, JWT, mTLS). [S10][S11]
2. **Cross-site WebSocket hijacking (CSWSH).** Server must validate `Origin` on the WS handshake against an allow-list; WS has no built-in authN/Z. [S12] With CORS `*` today, a hostile page can drive the local API.
3. **Retention leakage.** Turn audio/transcripts in logs, crash dumps, tmp files, ngrok inspector (request replay UI records bodies).
4. **Voice cloning.** 3–30 s of audio can produce a usable clone; voice is no longer an authentication factor. FTC: no single fix; watermarking is removable. [S13][S14][S15] → stored user audio is a cloning asset; Galion's own TTS output is a misuse vector.
5. **Prompt injection via speech** (ASR text → LLM → tools): treat transcripts as untrusted input; no tool side effects without confirm.
6. **Transfers.** ngrok/cloud processors outside EU → Chapter V (cf. AZOP €4.5M case). [S9]

## 3. Recommendations vs V2-S3 (68%)

| # | Item | Status in S3 | Action | Gate |
|---|---|---|---|---|
| R1 | CORS | `*` (gap) | Replace with exact allow-list (`http://127.0.0.1:<port>`, tunnel host, later Caddy host); no credentials with wildcard. | Test: foreign Origin → 403 |
| R2 | WS Origin check | not stated | Reject WS upgrade unless `Origin` ∈ allow-list; plus per-session token (short-lived, in first WS message, not URL). [S12] | Test: CSWSH PoC fails |
| R3 | Bind | loopback ✓ | Keep `127.0.0.1` only; refuse `0.0.0.0` unless env flag + auth on. | Startup assertion |
| R4 | ngrok interim | none | Traffic Policy: OAuth (Google/GitHub, restrict to owner email) or Basic Auth + `restrict-ips`; disable/clear inspector; reserved domain; kill tunnel when idle. [S10][S11][S16] | Unauth curl → 401/redirect |
| R5 | Caddy later | no VPS TLS | Caddy automatic HTTPS (ACME), `reverse_proxy 127.0.0.1:<port>` (WS native), HSTS, keep CSP; `forward_auth` or app auth; app still loopback-bound behind Caddy. [S17][S18] | SSL Labs A, WS through proxy |
| R6 | Ephemeral turn buffers | design | Audio in RAM only, zeroise on turn end/barge-in; no PCM/transcript in logs (log lengths/latency only); tmpfs if files needed; `RLIMIT_CORE=0`; TTL ≤ session for text context. | grep-test logs for PCM/transcript |
| R7 | Consent deny-default | design | Defaults: retention OFF, training OFF, emotion OFF (06), speaker ID not built. Each opt-in separate, revocable, logged (timestamp+version, no audio). Mic only on explicit user action, visible indicator. | Config test: fresh install = all off |
| R8 | AI disclosure | missing | First-turn audible + visible "I'm an AI assistant" (HR/EN). Art. 50(1). [S3] | Acceptance test |
| R9 | Voice/cloning | — | Use only licensed stock voices (Piper models with licence recorded); no zero-shot cloning feature; ToS ban impersonation; plan TTS watermark eval (AudioSeal-class) before 2 Dec 2026. [S4][S15] | Voice licence manifest |
| R10 | GDPR paperwork | — | RoPA, privacy notice (HR+EN), DPIA (voice + AI → likely), processor list with SCC/TRA for ngrok/any cloud; breach runbook (72 h to AZOP). [S1][S9] | Docs present |
| R11 | LLM/tool safety | — | Transcript = untrusted; tools allow-listed; destructive actions need confirmation. | Red-team prompts |

Suggested re-score: R1+R2+R4 closes most of the gap to ~85%; R5 (blocked on VPS/SSH) to ~95%.

## 4. Gaps / uncertain
- No legal review; DPIA need assumed, not confirmed with AZOP's DPIA list.
- ngrok data location/inspector retention not verified this pass.
- Watermark robustness on 8 kHz phone audio unknown.

## Sources
- [S1] EDPB Guidelines 02/2021 on VVAs v2.0 — https://www.edpb.europa.eu/system/files/documents/2021-07/edpb_guidelines_202102_on_vva_v2.0_adopted_en.pdf
- [S2] EDPB VVA §3.4.3 user identification — https://service.betterregulation.com/document/520502
- [S3] EC FAQ, Art. 50 transparency — https://digital-strategy.ec.europa.eu/en/faqs/transparency-obligations-under-article-50-ai-act
- [S4] EC Code of Practice on AI-generated content — https://digital-strategy.ec.europa.eu/en/policies/code-practice-ai-generated-content
- [S5] Stibbe, labelling deepfakes — https://www.stibbe.com/publications-and-insights/deep-fakes-real-transparency-labelling-deep-fakes-and-ai-generated-public
- [S6] AZOP About — https://azop.hr/about-the-agency/
- [S7] AZOP National legislation — https://azop.hr/national-legislation/
- [S8] AZOP fines overview — https://azop.hr/upravne-novcane-kazne-3/
- [S9] AZOP €4.5M telco fine — https://azop.hr/administrative-fine-of-eur-4-5-million-imposed-on-a-telecommunications-operator-14-november-2025/
- [S10] ngrok Add Authentication — https://ngrok.com/docs/gateway/traffic-policy/examples/add-authentication
- [S11] ngrok IP + Basic Auth — https://ngrok.com/docs/gateway/examples/ip-restrictions-basic-auth
- [S12] OWASP WSTG WebSockets — https://wstg.owasp.org/latest/4-Web_Application_Security_Testing/11-Client-side/10-WebSockets/
- [S13] Adaptive Security (McAfee 3 s figure) — https://www.adaptivesecurity.com/blog/the-ultimate-guide-to-ai-voice-cloning-scams-how-to-detect-prevent-and-protect-against-them
- [S14] ThreatLocker vishing — https://www.threatlocker.com/blog/ai-voice-cloning-and-vishing-attacks-what-every-business-must-know
- [S15] FTC approaches to voice cloning — https://www.ftc.gov/policy/advocacy-research/tech-at-ftc/2024/04/approaches-address-ai-enabled-voice-cloning
- [S16] ngrok OAuth action — https://ngrok.com/docs/gateway/traffic-policy/actions/oauth
- [S17] Caddy Automatic HTTPS — https://caddyserver.com/docs/automatic-https
- [S18] Caddy reverse proxy quick-start — https://caddyserver.com/docs/quick-starts/reverse-proxy

— Maciej Grajczyk
