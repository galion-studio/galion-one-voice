# Galion products map — 2026-11-08

Pair 09, products map. Documentation only. Read from `galion-studio/galion-one-voice` at `3e1f5a12c53cd8e0dd47d111676d61d608540559` (`main`, merge of pull request 102). This file records what those documents and the tree actually name. It does not implement a product, start billing, edit `LOCKS.toml`, or claim a deploy.

## Owner pricing, now

The owner sets two **planned product tiers** on 2026-11-08. They supersede the older Plus / Ultra / unpriced-Research price mapping below. They are plan figures. Nothing in this repository charges them.

| Planned tier | Price | What the owner named |
|---|---:|---|
| Galion One | $20 | The companion product tier |
| Galion Research Ultra | $220 | The research product tier |

No runtime billing claim. `LOCKS.toml` `[licence]` still has `free_forever = true` and `paid_tiers = false` (lines 181–182). `[meta] program` is `"Galion Voice"` with the comment `not Galion One` (line 13). The brand page says the one-dot surface shows no price, and that a paid tier enters the lock only by a later owner edit, counsel, and a ledger line (`docs/brand/GALION-VOICE-BRAND.md`). This page does not make that edit.

### Older mapping this statement supersedes

Those older sentences stay in their files as history. They are no longer the current product-tier names.

| Older name | Figure in the 2026-11-03 notes | Where |
|---|---|---|
| Plus | $20 / month, CX23-class, about $6 COGS, 100 GB SSD, 16 GB RAM, voice-to-voice included | `docs/galion-voice-v2/symphony/PLUS-ULTRA-RESEARCH.md`, `docs/galion-voice-v2/symphony/PLAN-PLUS.md` |
| Ultra | $220 / month, CX43-class / Titanaxe Max 3XL; t284 also writes `Ultra $220 / 2GB SSD`, and the Ultra page leaves the workspace quota unnamed | `docs/galion-voice-v2/symphony/PLAN-ULTRA.md` |
| Research console | Its own hosted surface. The console plan says it describes a surface and a compute budget and does not create a price | `docs/galion-voice-v2/symphony/PLAN-RESEARCH-CONSOLE.md` §5 |
| V3 admin backlog item 5 | Voice-to-voice under Galion One, exclusive to paid Plus and Ultra, with **no price attached**. Research voice-to-voice is not included until the owner says so. Marked not in force while `paid_tiers = false` | `docs/galion-voice-v3/ADMIN-BACKLOG.md` §5 |

Staging CPU caps that reuse the words Plus and Ultra (`docs/galion-voice-v2/symphony/STAGING-CPU.md`, `docs/galion-voice-v2/GALION-VOICE-V2.md` version map) are host-session budgets for Titanaxe. They are not these planned tiers, and they are not a storefront.

Hardware class notes (CX23, CX43, disk sizes, EPYC fleet sizing) stay in the files that wrote them. This page does not move those shapes onto Galion One or Galion Research Ultra. The owner named prices and product tiers, not a new machine spec.

## How a name got onto this map

A row is here when this repository’s docs or code name it as a Galion product, an operating base, or a dependency those products already point at. Test voices, engines, and hostnames are listed only so they stay distinct.

The lineup the Research console plan already prints (`docs/galion-voice-v2/symphony/PLAN-RESEARCH-CONSOLE.md` §1):

| Name there | Role there |
|---|---|
| Galion One | Companion voice. Speaks on Galion Voice |
| Obsidian Open Mind | Rust agent stack. Its own repo. Observation, not the research answer |
| Galion Voice | The voice product. One dot |
| Galion Research | The research product, previously called Hermes Research. This console |

Galion OS is named beside that lineup as the tenant environment and the Linux desktop target. It is not a fifth row in that table.

## Voice

**Repo.** This one: `https://github.com/galion-studio/galion-one-voice`. Root `README.md` calls the tree Galion Voice V2. Slogan in the V2 plan and the brand doc: “The seamless connection with the human being, on the voice only.”

**What it is.** Full-duplex voice on one mark. You band, me band, feeling band. Clock locked at 80 ms, 12.5 Hz, 24 kHz (`LOCKS.toml` `[clock]`). The user surface is one filled circle, `#F5F5F7`, on ground `#0A0A0C`, amber `#F5A524` to violet `#7C3AED` on the background only.

**Implemented, in this tree.** The root README says V1 is the foundation and V2 is this program, and that nothing in this tree has been deployed.

| Piece | Evidence | State the docs give it |
|---|---|---|
| One-dot phone page, `/turn`, `/reply`, canned reply | `README.md`; `docs/galion-voice-v2/GALION-VOICE-V2.md` §1 and §6 | In the tree. Screen script sets `GALION_LLM=canned` |
| 80 ms heartbeat | `crates/galion-heartbeat`; symphony index | In the tree. `Heartbeat::step` is the clock |
| Emotion / feeling band, consent deny by default | `galion-emotion`; `docs/galion-voice-v2/EMPATHY.md` | In the tree. `sense` starts off, so `e` is 0 without a grant |
| English phone ASR | `crates/galion-asr-vosk`, `docs/asr/VOSK.md` | Out of process. Missing model means mic turns error. Not called ratified recognition |
| Speech choice | `crates/galion-ml-voice` `SpeechStack::detect` | Clone child, then Piper A21, then Flite. Weights are not in git |
| Orb spring | `galion-render`; `docs/galion-voice-v2/ORB.md` | Foundation in the tree. V2 plan: G4 golden not re-blessed |
| Symphony voice file | `crates/galion-symphony` | Compiler path for `.gq`. Rust crates are what play it |
| Separate cbx clone worker | `crates/galion-clone-cbx` | Loopback process. Not inside `SpeechStack::detect`. Does not download weights |
| Open Mind seam and Hermes side band | `crates/galion-openmind`, `crates/galion-sideband` | Types and `try_send`. Default transport is `NullTransport` (`crates/galion-sideband/src/client.rs`) |
| Nebula visualiser | `apps/nebula-voice-v2`, `crates/nebula-elements` | Own Cargo workspace, not a root workspace member. Visualiser for the demo lines. Not a priced product |

Workspace members are the crates in the root `Cargo.toml`. `apps/nebula-voice-v2` and `runtimes/galion-piper-rt` are outside that workspace on purpose.

**Plans.**

| Plan | File | Standing |
|---|---|---|
| V2 program | `docs/galion-voice-v2/GALION-VOICE-V2.md` | Architecture, orb, typed bus, harder edge. Bus and the real ASR engine are specified, not swapped in |
| V2 symphony index | `docs/galion-voice-v2/symphony/README.md` | Section owners and a gate ledger. A section is done when SELF-EVAL copies a PASS |
| V3 folder | `docs/galion-voice-v3/STATUS.md`, `GALION-VOICE-V3.md` | Docs only. No V3 code until six live witnesses exist (Samantha clone, Jarvis clone, memories, emotions, gradients/color-in-voice, colorful space). The older V2 sentence still defines V3 as production readiness (ratified EN ASR, reply past canned, warm speech, VPS edge, R0–R3, counsel). STATUS question 2 leaves those two lists side by side |
| Plan V4 / V5 | `docs/ops/PLAN-V5.md`, `docs/deploy/TITANAXE-VPS.md` | Ops series, not Voice V4/V5. Dry-run refuses `--execute`. No SSH from the plan |
| Emotion V2 | `docs/emotion-v2/HANDOFF.md` and the sibling specs | Taxonomy, CI, dataset, and capture notes now sit on `main`. The handoff text still says stub tests are not the rollout gates (WER, ECAPA, emotion_match) |

**Dependencies Voice already names.** Vosk (out of process), Piper A21 pack (fetched, sunset `2027-02-01` in the locks), Flite, Chatterbox/cbx out of process, the Galion student slot (`galion-student/`, configs only, nothing trained), Obsidian Open Mind (not vendored), Hermes on the side band, Galion OS as a future host group. External cloning APIs in `docs/galion-voice-v2/VOICE-CLONING.md` are marked superseded and dropped.

**TODOs the Voice docs still leave open.**

- Real EN ASR is not ratified. The V2 version map says the bus and the real ASR engine are specified, not swapped in.
- The live screen reply is the canned table until a student passes readiness (`galion-student/README.md`).
- Clone generate is not the live path. Rights clearance for Samantha and Jarvis is unfilled (`docs/galion-voice-v2/RIGHTS-CLEARANCE.md`). Reference audio stays out of git.
- V3’s six witnesses are not in the V3 ledger. V3 code is refused until they are.
- Deploy is approved-pending-setup on Titanaxe `51.38.137.193`. This tree holds no key and does not claim the service is up (`docs/brand/GALION-VOICE-BRAND.md`).
- `paid_tiers` stays false. The new $20 and $220 figures are planned tiers on this page, not a Voice checkout.

## Galion One

**Repo.** Same repository. There is no second Galion One repository named in this tree. The Open Mind voice snapshot’s unit name `galion-one` is a binary path in that other repo (`/opt/galion/openmind/dist/bin-ws/galion-one`). Galion Voice does not read that snapshot (`docs/galion-voice-v2/OPENMIND-NERVE-CORTEX.md` §1–2).

**What it is.** The product voice for a product session, and the companion in the lineup. Instruction file: `docs/galion-voice-v2/personas/GALION-ONE.md`. Status line: “Main product voice for Galion Voice / Galion One.” When asked, the voice says “I’m Galion.” Samantha and Jarvis are internal test voices in that file. The personas index says the same (`docs/galion-voice-v2/personas/README.md`).

**Implemented.** The instruction file is in git. The speaking enum that the V2 page actually compiles is Samantha and Jarvis only:

```49:60:galion-render/src/v2_persona.rs
pub enum PersonaId {
    Samantha,
    Jarvis,
}

impl PersonaId {
    pub fn as_str(self) -> &'static str {
        match self {
            PersonaId::Samantha => "samantha",
            PersonaId::Jarvis => "jarvis",
        }
    }
}
```

Those two registers are loaded from `docs/galion-voice-v2/personas/voice/samantha.toml` and `jarvis.toml`. There is no `galion-one.toml` beside them. The voice-config README says the session default is Samantha, and that `wired` means the `/v2` demo route reads the file, not that a film clone is live. `GALION-ONE.md` says a product session that has not been pinned to a test id speaks as Galion One, and that the Samantha default in `SAMANTHA-V2.md` does not rename that product voice. The bus id for Galion One is not added by this map. `SAMANTHA-V2.md` still names `samantha`, `jarvis`, and an empty future slot.

The root README’s cloning sentence already treats Galion One as a product voice on a normal call, and Galion Research as the research-path voice. The code path above is still the two test ids.

**Plans.** V3 admin backlog item 5 places voice-to-voice under Galion One and, in that backlog, limits it to paid Plus and Ultra, with Local free of voice-to-voice. That item is written and not in force. It attaches no dollar amount. The 2026-11-08 owner tier **Galion One at $20** is the planned price for this product. It replaces Plus-at-$20 as the name of the $20 tier. It does not turn backlog item 5 on, and it does not assert that a $20 session has voice-to-voice while `paid_tiers` is false.

**Dependencies.** Galion Voice is the surface it speaks on (one dot, heartbeat, speech stack). Emotion weight `e` and the wash are the Voice empathy path. A future bus id would sit in the persona slot `SAMANTHA-V2.md` already reserves. Open Mind’s `galion-one` unit is a different process in the other repository.

**TODOs.**

- No compiled persona id, no voice TOML, and no clip binding for Galion One.
- The wired demo default remains Samantha until an owner pins the product id.
- Backlog item 5 still needs an owner lock edit, counsel, a root ledger line, and an explicit Local feature list before it could be offered. This file does not supply that list.
- The $20 tier is planned. No checkout, meter, or invoice exists in this tree.

## Galion Research

**What it is.** A product in the same lineup, previously called Hermes Research. People who say “Hermes” are answered as Galion Research. The voice file is `docs/galion-voice-v2/personas/GALION-RESEARCH.md`. The desk is `docs/galion-voice-v2/symphony/PLAN-RESEARCH-CONSOLE.md`. The console is not built in this landing. The plan says so in the header.

**Hermes is the access layer, not this product’s repository.** Searches of this tree find no `hermes-research` repository URL and no `github.com/galion-studio/hermes` remote. The former name is the product rename recorded in `GALION-RESEARCH.md`, the personas index, `PLAN-RESEARCH-CONSOLE.md` §1, and `docs/galion-voice-v3/ADMIN-BACKLOG.md` §4. Hermes as code, in this repo, is:

| Piece | Path | Role |
|---|---|---|
| Side-band types | `crates/galion-sideband` | `Turn`, `Intent`, `Affect`. Retrieval cap 3 s. No I/O in the crate root |
| Null transport | `crates/galion-sideband/src/client.rs` `NullTransport` | Lookups come back unreachable until a human wires a transport |
| Voice handoff | `crates/galion-openmind` `CortexHandoff` | `try_send` only. Forbidden inside `Heartbeat::step` |
| Phone LLM hook | `crates/galion-cascade/src/hermes.rs` | V2-S15. `GALION_LLM=hermes`, loopback, 3 s cap. The approval record is not a call |

The seam note places the Hermes agent and gateway crates in the other repository: `crates/hermes-rs` and `crates/hermes-gateway` inside `obsidian-open-mind` (`docs/galion-voice-v2/OPENMIND-NERVE-CORTEX.md` §1). That is the named home of those crates. It is not a `hermes-research` repo, and it is not vendored here.

The persona’s bus id `galion-research` is a candidate for the empty slot. The Research plan says this landing does not pretend a `Persona` message already lists it. `PersonaId` above does not list it.

**Planned tier.** Galion Research Ultra at **$220**. That is the owner’s 2026-11-08 name for the research price. It supersedes “Ultra $220” as a Galion Voice host SKU and supersedes the console plan’s “does not create a price.” The console’s CPU band (nominal 250 decode tok/s and 1000 prefill tok/s per LLM node, marked indicative) stays a capacity sketch in that file. This page does not treat it as a measured Research Ultra result, and it does not attach the CX43 shape or the 2 GB SSD string to the new tier.

**TODOs from the console plan (§6–7).** Own process and own assets, not a route on the voice port. Seven state words only. `Unreachable` while the transport is null. No measured research tok/s. No console binary in this landing. Money actions stay refused (`Intent::voice_may_request` is false for `ActionTier::Money`).

## Obsidian Open Mind

**Repo named in source.** `https://github.com/galion-studio/obsidian-open-mind`. The seam was read on 2026-11-03 from `main` at `7ba85b603d8b0b87c4d4dede1056e1ee80c767ac`. This map does not re-fetch that repository. The description in the seam note: “Rust-first open agent stack for cheap CPU-only devices.”

**What this Voice tree implements toward it.** `crates/galion-openmind` matches the public nerve wire (schema version 1, loopback port 8670, kinds `voice.frame_miss`, `voice.backend_up`, `voice.fallback`). The slot file is `galion-student/configs/openmind-slot.toml` (`schema = "galion.slot.v2"`). Galion Voice does not link Open Mind crates. `/opt/galion/openmind` is the path a human deploy uses. The seam note says that path is absent on the Cursor host that wrote it, and that this program does not clone the tree.

**Two cortices.** Open Mind’s incident cortex (`crates/gaia-cortex` in that repo) correlates unit failures. Voice does not send it problems. In Voice, “cortex” means Hermes on the side band.

**Desktop.** The V3 admin backlog §4 aims “Hermes Research and the console” at a **Galion OS Linux desktop**. It says the nerve group `galion-os` is not, by itself, a desktop install, and that no image is named yet. Obsidian Open Mind is the agent stack in its own repo. The backlog does not call that repo the desktop image. Question 15 in `docs/galion-voice-v3/STATUS.md` still asks which image, and whether the one dot stays the only voice surface. `console_is_user_ui = false` stands.

**Dependencies.** Voice nerve notes, when a human runs `galion-nerve`. Memory tiers L0–L3 are described in the symphony stack plan as living in `obsidian-open-mind` docs (`ws-memory`, `memory-api`, `memory-schema`). They are not a crate in this workspace.

**TODOs.** Do not vendor Open Mind. Do not add a git dependency on it (`OPENMIND-NERVE-CORTEX.md`). Gateway choice C5 and memory surface C6 stay open in `crates/galion-sideband/src/lib.rs`. The student that would plug the Open Mind slot is config-only.

## Galion OS

**What the Voice docs say it is.** The tenant operating environment on the Ultra computer plan (`PLAN-ULTRA.md` §2): “This repository does not ship the image. The image spec stays where the OS work already put it.” The V3 backlog §4 calls it the Linux desktop target for Research and the developer console. Emotion consent copy names “Galion OS” as a host a person can allow a tone estimate to reach (`galion-emotion/src/store.rs`). The render window comment says Galion OS builds can swap in `/dev/fb0` (`galion-render/src/backends/window.rs`). The wave note `docs/agent-wave20/W20-05-STATUS.md` calls that framebuffer path spec only.

**Implemented here.** The string `galion-os` as a nerve group and a unit name `galion-os-voice` in `crates/galion-openmind/src/nerve.rs`. That is an inventory label on a voice event. It is not an OS image, a package, or a framebuffer driver.

**Plans.** One desktop where Research and the console have a home, with the voice surface still the one dot (`ADMIN-BACKLOG.md` §4). Ultra’s computer plan seats Galion OS, CPU agents, and Galion Voice on one CX43-class host. That seating is a plan. No image is in this repo.

**Dependencies.** The OS image, wherever that work lives, is outside this checkout. Voice would be a profile on it (symphony stack plan P9: “Galion OS Voice profile”). Open Mind’s nerve is the observer when a human runs it. Hermes stays off the 80 ms tick.

**TODOs.** Name the image (STATUS question 15). Keep `console_is_user_ui` false until an owner edits the lock. Do not promote the `/dev/fb0` note to a build from the V3 backlog.

## Other names, kept distinct

These are named in source. They are not extra priced products on this map.

| Name | What the source says | Not this |
|---|---|---|
| Samantha, Jarvis | Internal test voices. `PersonaId`. Rights hold closed | Shipping product personas (`personas/SAMANTHA.md`, `personas/JARVIS.md`) |
| Nebula Galion Voice V2 | Visualiser app and Demo 13 emission table | A SKU. Nebula “ultra” is a 240 fps pacing mode in `apps/nebula-voice-v2/src/quality.rs`, unrelated to the $220 tier |
| Galion student | `galion-student/` distill configs. S0 is the first size to attempt. Nothing trained | A product |
| Rika | Feeling-band reader. `rika_reads_transcript = false` | A product |
| Titanaxe | VPS name `51.38.137.193`, approved-pending-setup. “Titanaxe Max 3XL” is the owner’s name for the CX43 class in the CPU note | A product, and not identified as a Max 3XL by `PLAN-ULTRA.md` |
| G.A.L.I.O.N. | “General Autonomous Local Intelligence Open Network” in the README footer | A separate repo in this tree |
| PersonaPlex | Re-anchor note and a remote relay option in Open Mind docs | Not the phone path. Letter A stays Vosk |
| Gaia cortex | Incident engine inside Open Mind | Not the Voice research cortex |

## Dependency sketch

```text
Galion OS (image not in this repo; Linux desktop target)
  └── Obsidian Open Mind (github.com/galion-studio/obsidian-open-mind)
        ├── nerve :8670, incident cortex
        └── hermes-rs / hermes-gateway
              └── consumed, not vendored, by this repo
                    ├── crates/galion-sideband
                    └── crates/galion-openmind

Galion Voice (this repo, one dot, 80 ms)
  ├── speaks Galion One on a product session (instruction file only today)
  ├── test registers Samantha and Jarvis (the compiled PersonaId)
  └── at a turn boundary, CortexHandoff → Hermes side band
        └── Galion Research desk (plan only; own process when it exists)

Planned tiers (2026-11-08, not billed)
  ├── Galion One          $20
  └── Galion Research Ultra  $220
```

Voice does not depend on a `hermes-research` repository. Research’s lookup path in this tree is the side band plus, when a human deploys it, the Hermes crates named inside Obsidian Open Mind.

## Evidence index

| Claim | Path |
|---|---|
| Program is Galion Voice; paid tiers off | `LOCKS.toml` `[meta]`, `[licence]` |
| Nothing in this tree deployed; canned screen | `README.md` |
| V1 / V2 / V3 version map | `docs/galion-voice-v2/GALION-VOICE-V2.md` §1 |
| V3 docs-only gate | `docs/galion-voice-v3/STATUS.md` |
| Product lineup of four | `docs/galion-voice-v2/symphony/PLAN-RESEARCH-CONSOLE.md` §1 |
| Galion One voice | `docs/galion-voice-v2/personas/GALION-ONE.md` |
| Compiled personas are Samantha and Jarvis | `galion-render/src/v2_persona.rs` |
| Research voice, ex-Hermes Research | `docs/galion-voice-v2/personas/GALION-RESEARCH.md` |
| Open Mind repo and commit pin | `docs/galion-voice-v2/OPENMIND-NERVE-CORTEX.md` |
| Older $20 Plus and $220 Ultra | `PLAN-PLUS.md`, `PLAN-ULTRA.md`, `PLUS-ULTRA-RESEARCH.md` |
| Backlog prices not in force | `docs/galion-voice-v3/ADMIN-BACKLOG.md` §4–§5 |
| Brand: no price on the dot | `docs/brand/GALION-VOICE-BRAND.md` |

## What this file does not do

- No crate, script, feature, test, or workflow change.
- No edit of `LOCKS.toml`, `SOUL.md`, or `config.yaml`.
- No licence change and no claim that `paid_tiers` flipped.
- No checkout, invoice, meter, or “billed” state for $20 or $220.
- No deploy, SSH, package install, or image build.
- No weight, recording, or secret.
- No merge. The older plan files are left as written so their historical figures remain visible.
