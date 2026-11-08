# Pair 06 — pricing and open source

2026-11-08. Galion Voice repository `galion-one-voice`. This file is the product-map note for prices, `paid_tiers`, and what may be published. It records an owner direction. It does not implement billing, grant an entitlement, edit `LOCKS.toml`, or change a licence.

Checked against `main` at `3e1f5a12c53cd8e0dd47d111676d61d608540559` (merge of pull request #102, 2026-11-07 20:02 +0100).

## Owner direction recorded on this date

The explicit owner prices for this note are:

| Owner name, as written on 2026-11-08 | Price |
|---|---|
| Galion Research ULTRA subscription | $220 |
| GalionOne | $20 |

Those two sentences are the owner direction this file is allowed to treat as current. They are not copied from a checkout, a payment provider, or a customer ledger. Nothing below turns them into a running subscription.

Spelling in the tree is **Galion One** and **Galion Research**, with a space. The owner strings **GalionOne** and **Galion Research ULTRA** are kept here as written. This file does not rename either product.

## What the repository enforces today

`LOCKS.toml` `[licence]` is the licence lock:

- `code = "Perpetual Open MIT"`
- `copyright = "(c) 2026 Maciej Grajczyk"`
- `free_forever = true`
- `paid_tiers = false`

Sources named on that table are `Q-BRANDING.Q9`, `Q-BRANDING.Q10`, and `Q-BRANDING.Q13` (`LOCKS.toml` lines 177–183).

The root `README.md` repeats that lock: code is Perpetual Open MIT, free forever, no paid tiers, and weights stay out of git (`README.md` lines 154–156). The root `LICENSE` is the standard MIT text, copyright 2026 Maciej Grajczyk, including the usual MIT right to sell copies of the software. That grant is a copyright licence on the code. It is not a hosted-subscription product, and it is not a waiver of `paid_tiers = false`.

`docs/brand/GALION-VOICE-BRAND.md` (lines 65–81) says the same lock, says weights are outside the grant, and says the one-dot page shows no price, no trial, and no checkout. It points at the Ultra host plan and says putting that plan into `[licence]` is a later owner edit of the lock, with counsel and a `LEDGER.md` line. Until that edit, `paid_tiers` stays false.

This pull request does not make that edit.

## Where $20 and $220 already appear

The same dollar amounts exist as host-class notes. They are attached to the names Plus and Ultra, not to the 2026-11-08 names GalionOne and Galion Research ULTRA.

| Place | What it states | What it is |
|---|---|---|
| `crates/galion-symphony/src/host_cpu.rs` lines 46–66 | `Cx23` role `plus`, `price_usd_mo: Some(20)`, note "Plus. CX23-class. $20/mo. COGS about $6. 16 GB RAM, 100 GB SSD." `Cx43` role `ultra`, `price_usd_mo: Some(220)`, note "Ultra. CX43-class, Titanaxe Max 3XL. $220/mo. 2 GB SSD." `Epyc9655p` has `price_usd_mo: None`. | A static host table inside the symphony pack. `HostId::parse` accepts `plus` as `cx23` and `ultra` as `cx43`. |
| `crates/galion-symphony/src/eval.rs` lines 421–448, gate `cpu-budgets` | Asserts CX23 price 20 and CX43 price 220, CX23 RAM 16 GB and SSD 100 GB, CX43 SSD 2 GB and RAM unset. | A pack self-check. Passing it means the table still holds those literals. It does not charge anyone. |
| `docs/galion-voice-v2/symphony/PLAN-PLUS.md` lines 9–25 | Plus is $20 per month, COGS about $6, quota 16 GB RAM and 100 GB SSD, voice-to-voice inside that $20. The page says the licence table stays as it is and the README stays without a price list. | A symphony sizing plan dated 2026-11-03, source note t284. |
| `docs/galion-voice-v2/symphony/PLAN-ULTRA.md` lines 11–31 and 109–138 | Ultra is $220 per month. Galion Research is "a separate hosted console" and "Ultra does not include a second machine for it." t284 storage string recorded verbatim: "Ultra $220 / 2GB SSD". The page leaves `paid_tiers` untouched and says the one-dot page shows no price. | A symphony host plan, not a booted machine and not a benchmark. |
| `docs/galion-voice-v2/symphony/STAGING-CPU.md` lines 94–118 | Owner list: Local free forever; Plus $20/mo with voice-to-voice and an entitlement line of 100 GB SSD + 16 GB RAM; Ultra $220/mo with an entitlement line of 2 GB SSD. An earlier Ultra figure of 590 € is described as overridden by the $220 / CX43 line. The CX43 invoice band on that page is €15.99–€16.49, which is an infrastructure cost, not the $220 list. `paid_tiers` stays false. Galion Research is a separate hosted surface, off the 80 ms path. | Capacity notes. The word "entitlement" on that page is a quota string from t284. There is no entitlement record in code. |
| `docs/galion-voice-v2/GALION-VOICE-V2.md` lines 496–507, section V2-S8 | Acceptance requires licence text to match the lock, including `paid_tiers = false`. The same section names the Ultra host plan at $220/month and says that plan does not edit the lock and the one-dot page does not show a price. | Program plan. The dollar figure is a pointer at `PLAN-ULTRA.md`. |

No source file in this pass implements a payment provider. A search of `*.rs`, `*.md`, `*.toml`, `*.yml`, `*.py`, and `*.js` for `stripe`, `paddle`, `lemonsqueezy`, `chargebee`, and `recurly` found no integration. The only `stripe` and `paddle` hits were tokenizer vocabulary bytes under `tools/cloning-v2/scorers/universal/clap/`, which are not billing. `subscription` in product docs is the V3 statement that an external API, if it ever exists, is not a flat subscription (`docs/galion-voice-v3/GALION-VOICE-V3.md` lines 107 and 140).

There is no customer account, invoice, checkout, or entitlement type in this tree that grants GalionOne or a Galion Research ULTRA subscription. Saying otherwise would invent billing.

## Name map: owner direction against the current tree

| 2026-11-08 owner direction | Closest existing object | Agreement | Gap |
|---|---|---|---|
| GalionOne at $20 | Plus / CX23 at $20 in `host_cpu.rs` and `PLAN-PLUS.md` | The dollar amount matches a figure already written for Plus. | Galion One in `docs/galion-voice-v2/personas/GALION-ONE.md` and `personas/README.md` is the main product voice, not a priced SKU. Nothing names a product "GalionOne" or bills $20 for that voice. |
| Galion Research ULTRA subscription at $220 | Ultra / CX43 at $220 in `host_cpu.rs` and `PLAN-ULTRA.md` | The dollar amount matches a figure already written for Ultra. | `PLAN-ULTRA.md` section 2 says Galion Research is a separate hosted console and that Ultra does not include a second machine for it. `STAGING-CPU.md` repeats that Research is outside the voice-session budget. The persona file `GALION-RESEARCH.md` is a voice (formerly Hermes Research), not a $220 subscription. |

Until the owner maps those names, this file refuses both silent equivalences: GalionOne is not hereby Plus, and Galion Research ULTRA is not hereby the CX43 Ultra computer.

## Contradictions already in the tree

These are disagreements between files that are already on `main`. This note lists them. It does not pick a winner by editing the lock or the host table.

1. **Unpaid lock versus printed prices.** `paid_tiers = false` and `free_forever = true` (`LOCKS.toml` lines 181–182; `README.md` line 156; brand doc lines 70–71). `PLAN-PLUS.md`, `PLAN-ULTRA.md`, `STAGING-CPU.md`, `GALION-VOICE-V2.md` V2-S8, and `host_cpu.rs` still print $20 and $220. The plan pages say the dollars do not flip the lock. The code still stores the dollars and `eval.rs` fails `cpu-budgets` if they change. Both facts are true at once: the lock says there is no paid tier, and the pack stores two monthly figures.

2. **V3 backlog says Plus and Ultra have no price and are not products.** `docs/galion-voice-v3/ADMIN-BACKLOG.md` lines 163–174: Plus and Ultra are "Not a product" while `paid_tiers = false`, and "No price is attached to Plus or Ultra." `PLAN-PLUS.md` line 14 and `PLAN-ULTRA.md` line 13 attach $20 and $220 and call the pages SKU plans. `docs/galion-voice-v3/GALION-VOICE-V3.md` lines 114–129 and 140–143 say `paid_tiers` stays false and list "a price list, a SKU, or a public Plus / Ultra offer" as a non-goal. The research brief says the same restraint: Local/Plus/Ultra are deployment profiles, and prices stay out of product docs (`docs/research/galion-voice-v2/master/GALION-VOICE-V2-RESEARCH-BRIEF.md` line 24; `domains/10-hardware-edge.md` lines 14 and 59).

3. **Voice-to-voice exclusivity is backlog, not the running product.** `ADMIN-BACKLOG.md` lines 153–169 and `GALION-VOICE-V3.md` line 110 say voice-to-voice under Galion One would be exclusive to paid Plus and Ultra, with Local free forever and without voice-to-voice. The same backlog says that statement is not in force, and that the running tree is a voice program the row does not strip. `PLAN-PLUS.md` line 16 says voice-to-voice sits inside the $20. Neither page is an entitlement check in code.

4. **External API meter versus a flat subscription.** V3 item 2 is "an external paid Galion API, billed per job" and "not a flat subscription for the API" (`GALION-VOICE-V3.md` lines 107 and 140; `ADMIN-BACKLOG.md` lines 66–74; `VERIFICATION.md` line 102). The 2026-11-08 owner direction is two flat prices, $220 and $20, on named products. Those are different sentences. This file does not merge the API meter into either subscription, and it does not claim the API exists.

5. **Host shape versus the quota glued to the price.** `host_cpu.rs` gives CX23 `ram_gb: Some(16)` and `ssd_gb: Some(100)` at $20, and CX43 `ssd_gb: Some(2)` with RAM unset at $220. `PLAN-ULTRA.md` lines 45–50 reads a published CX43 class as 8 shared vCPU, 16 GB RAM, 160 GB NVMe, and then refuses to adopt "2GB SSD" as the workspace because it conflicts with that class and with the Plus 100 GB line. `STAGING-CPU.md` lines 103–112 says the Plus entitlement is larger than a CX23 (4 GB, 40 GB) and that the CX43 invoice band is about €16, not $220. `PLAN-PLUS.md` lines 31–40 treats 16 GB and 100 GB as ceilings on a pooled host whose COGS is about $6, and cites dedicated 16 GB boxes priced far above $6. The monthly figures $20 and $220 are stable across these pages. The machine that earns them is not.

6. **Research console says it does not create a price, then cites the freemium disks.** `docs/galion-voice-v2/symphony/PLAN-RESEARCH-CONSOLE.md` line 209 says Plus and Ultra disks belong to those SKU plans, `[licence] paid_tiers` stays false, and the plan does not create a price. That is consistent with the lock and inconsistent with reading Galion Research as the $220 subscription.

7. **Licence wording.** `LOCKS.toml` calls the code "Perpetual Open MIT". `LICENSE` is the standard MIT licence. `docs/galion-voice-v2/personas/README.md` lines 32–33 adds "All rights reserved and claims reserved" in the same footer that says current builds are open source under the Galion perpetual open MIT licence. This pull request does not reconcile that footer and does not change either licence file.

8. **Reference-clip lock versus the rights page.** This is a publication constraint, not a price. The comment above `[formats.exceptions.v2s14_chatterbox]` says "No reference clips" (`LOCKS.toml` lines 140–141). The key in that table is `reference_clips = true`, scoped to CC0 / public-domain LibriVox donors Amy Koenig and Alan Davis Drake, stored outside the tree, with no celebrity voice, no film audio, and no Armstrong (`LOCKS.toml` lines 152–154). `docs/galion-voice-v2/RIGHTS-CLEARANCE.md` lines 52–53 still says that same table sets `reference_clips = false`. `personas/README.md` lines 23–24 says a public demo in either film voice stays closed, and also says `reference_clips` still needs an owner flip. Film-voice demos stay unpublished. This note does not flip the key.

## Open source and publication policy

Code publication stays on the licence already in the tree: Perpetual Open MIT for Galion code, copyright 2026 Maciej Grajczyk, weights separate. Third-party trees under `tools/cloning-v2/pipeline/` carry their own `LICENSE-MIT` and `LICENSE-APACHE` files. This policy does not relicense them and does not change repository visibility.

Owner publication rule for this swarm, 2026-11-08:

- Swarms stay at two or three agents.
- Each agent owns one Markdown file in `docs/galion-context/2026-11-08/`. This agent owns only `09-pricing-and-open-source.md`.
- Research, docs, cleared datasets, and build artifacts may be pushed to the GitHub repository they belong to. This note belongs in `galion-one-voice`.
- Opening a pull request is allowed.
- Merging is not allowed unless the owner explicitly asks. This pull request is not a merge request that should be completed by an agent.
- Licensing and visibility stay as they are. `LOCKS.toml` stays as it is, including `paid_tiers`, `free_forever`, and `reference_clips`.
- Private recordings, credentials, uncleared datasets, and weights are excluded from the push.
- A public artifact needs a rights check and a provenance check before it is inventoried as public.
- Raw logs and secrets are not copied into docs, pull requests, or artifacts.

Public-artifact inventory for this repository, from files already cited above. This is a gate list, not a new clearance.

| Class | Current evidence | Push? |
|---|---|---|
| Galion source and docs already under the MIT / Perpetual Open MIT grant | `LICENSE`, `README.md` licence section, `LOCKS.toml` `[licence]` | Allowed as source. A price note does not make a paid tier. |
| Piper and other fetched weights | `README.md` line 156; `LOCKS.toml` `weights_vendored = false`; `docs/galion-voice-v2/CLONE-WEIGHTS.md` | Excluded. Separate grants. Not in this pull request. |
| Clone weights and safetensors | `CLONE-WEIGHTS.md`; fetch only behind `GALION_CLONE_FETCH=1` | Excluded. |
| Own-voice and private recordings | `docs/emotion-v2/HANDOFF.md` consent scope; persona wavs gitignored (`personas/README.md`) | Excluded. |
| Film-voice Samantha or Jarvis audio | `RIGHTS-CLEARANCE.md`; `VOICE-CLONING.md` as cited there | Excluded. Public demo stays closed. |
| CC0 / LibriVox donor clips named in the chatterbox exception | `LOCKS.toml` lines 152–154 require storage outside the tree with `consent.txt` and a sha256 sidecar | Excluded from this pull request. A later public copy needs the sidecar and the named scope, and still has to survive the rights page, which does not match the boolean. |
| Credentials, tokens, raw logs | Owner rule for this folder | Excluded. This file quotes no log, no host address, and no secret. |
| Uncleared datasets | `docs/emotion-v2/EMOTION-LANDSCAPE-20261107.md` already warns that a commercial speech corpus is not an open licence | Excluded unless a later note cites a cleared grant. |

## Evidence-linked TODOs

No item below was executed in this change. They are audit and mapping work for a later owner decision.

| ID | Todo | Evidence | Done here |
|---|---|---|---|
| BILL-MAP-20 | Owner states whether GalionOne at $20 is the existing Plus / CX23 note, a new product, or neither. Leave `host_cpu.rs` and `eval.rs` unchanged until that statement is explicit. | `host_cpu.rs` lines 46–55; `PLAN-PLUS.md` lines 14–16; `personas/GALION-ONE.md` status line (voice, not a price); `eval.rs` lines 430–433 | Not done |
| BILL-MAP-220 | Owner states whether the Galion Research ULTRA subscription at $220 is the existing Ultra / CX43 note, a Research console on its own machine, or neither. `PLAN-ULTRA.md` section 2 currently separates Research from the Ultra computer. | `host_cpu.rs` lines 57–66; `PLAN-ULTRA.md` lines 11–31; `GALION-RESEARCH.md` status; `STAGING-CPU.md` lines 117–119; `eval.rs` lines 434–436 | Not done |
| BILL-LOCK | Any later decision to sell a tier still starts as an owner edit of `[licence]`, counsel, and a root `LEDGER.md` line, which is the path the brand doc already requires. Do not flip `paid_tiers` or `free_forever` in a documentation swarm. | `LOCKS.toml` lines 177–183; `GALION-VOICE-BRAND.md` lines 77–81; `ADMIN-BACKLOG.md` lines 176–181 | Not done |
| BILL-AUDIT | Audit for a real billing implementation: payment provider, customer id, invoice, and an entitlement check that can deny a call. This pass found host literals and plan prose only. A future audit must look again after new commits, and must not treat `price_usd_mo` as a charge. | Search described in the section "Where $20 and $220 already appear"; `host_cpu.rs` `HostClass.price_usd_mo` | Not done |
| BILL-QUOTA | Reconcile 2 GB versus the CX43 class disk, and 16 GB / 100 GB versus a CX23 shape, without inventing a quota. The monthly prices stay owner figures until the owner changes them. | `PLAN-ULTRA.md` lines 108–138; `STAGING-CPU.md` lines 97–112; `host_cpu.rs` lines 51–65 | Not done |
| BILL-API | Keep the V3 per-job API meter off these two flat prices unless the owner writes one sentence that joins them. | `GALION-VOICE-V3.md` lines 107, 140–143 | Not done |
| ART-RIGHTS | Before any dataset, weight, recording, or build artifact is called public, check the grant, the provenance, and the exclusion table above. Do not inventory private recordings or uncleared sets by copying them into git. | Publication policy in this file; `RIGHTS-CLEARANCE.md`; `CLONE-WEIGHTS.md`; `LOCKS.toml` lines 140–154 | Not done |

## What this change does

It adds this Markdown file and nothing else. `LOCKS.toml`, `LICENSE`, `README.md`, `jarvis.toml`, `REFS.sha256`, host prices, and visibility are untouched. No billing code, checkout, or entitlement was added. No deployment, build, dataset, weight, recording, or log was added.
