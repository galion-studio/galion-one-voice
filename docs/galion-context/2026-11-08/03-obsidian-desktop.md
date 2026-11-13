# Obsidian desktop — product context

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. 2026-11-08T04:30:49+01:00. Pair 04, Open Mind. This file is
the architecture note for the desktop question. It does not add code, a
package, a boot image, a price in the lock file, or a deploy.

**Read this first.** No Galion repository checked on this date carries a
product whose title is “Obsidian Desktop.” Galion uses “Obsidian” for a
private desk, for the public agent stack Obsidian Open Mind, and for the
colour token `#0A0A0C`. Those three are different things. The third-party
Obsidian notes application, and Obsidian Security, are outside this family.
Galion’s own notes already say so.

Merged-PR audit is pair 08. This note cites the commits it actually read.
It does not certify every merge.

## 1. Verified repository and product map

Read 2026-11-08 from the GitHub API. `main` SHAs below are the commits the
API returned that morning. A repository `pushed_at` later than `main` only
means some branch moved. Other branches were not audited.

| Product name in the sources | Repository | `main` read here | What that tree actually is |
|---|---|---|---|
| Obsidian Open Mind | [galion-studio/obsidian-open-mind](https://github.com/galion-studio/obsidian-open-mind) | [`7ba85b60`](https://github.com/galion-studio/obsidian-open-mind/commit/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac) (2026-10-28, “publish 2026-10-28”) | Public Rust workspace. Agent stack, operator desk, console. API `private: false`. Repo `pushed_at` 2026-11-07; `main` was still this commit. |
| Galion Operating System | [galion-studio/galion-operating-system](https://github.com/galion-studio/galion-operating-system) | [`e48f3d4f`](https://github.com/galion-studio/galion-operating-system/commit/e48f3d4f6b7f2f0e0ae77ba4784ab427fb54587d) (2026-11-02) | Nine paths: `LICENSE`, `README.md`, and `docs/voice-profile/*`. Documentation. No image, no kernel, no PID 1 binary. |
| Hermes Research (public line) | [galion-studio/hermes-research](https://github.com/galion-studio/hermes-research) | [`e204789c`](https://github.com/galion-studio/hermes-research/commit/e204789ce73306c645dcb95b4b509cc764ca2c47) (2026-10-07) | Public research face. Three docs. The private desk is declared absent. |
| Galion Voice, Galion One (product voice), Galion Research (plan) | [galion-studio/galion-one-voice](https://github.com/galion-studio/galion-one-voice) | [`3e1f5a12`](https://github.com/galion-studio/galion-one-voice/commit/3e1f5a12c53cd8e0dd47d111676d61d608540559) (2026-11-07) | Voice program. One-dot surfaces. Seam to Open Mind. OS image is not in this tree. |
| Public notes | [galion-studio/community](https://github.com/galion-studio/community), [galion-studio/galion-studio](https://github.com/galion-studio/galion-studio) | READMEs read 2026-11-08 | “The desk is not in this repository.” |

No repository named `obsidian-desktop`, and no file path containing
`obsidian-desktop`, appeared in the org listing or in the Open Mind tree
(2,938 paths at `7ba85b60`). **Gap:** a desktop product under that exact
title was not found. This note does not invent a repo or a download URL
for it.

Org listing the same morning also shows other public Galion repos
(`galion-marine`, `Corpus-Nova`, `galion-initiative-web`, and older
projects). None of those READMEs was used as the desktop source of truth.
`galion-agi-web` returned an empty repository.

## 2. Three Galion uses of the word, kept apart

### 2.1 Obsidian, the private desk

[hermes-research `docs/BRAND.md`](https://github.com/galion-studio/hermes-research/blob/e204789ce73306c645dcb95b4b509cc764ca2c47/docs/BRAND.md)
(2026-10-07) is the naming note:

- Galion Studio is the company.
- **Obsidian** is “our name for the private desk: plain notes a person can
  open and read. Markdown, not a hidden database.”
- That paragraph also says it is not the Obsidian note-app company, not
  Obsidian Security, and not a public product. “The desk is not in this
  repository.”
- **Open Mind**, in that same file, is “the stance, not a second product:
  the research is public, under MIT, and the desk stays private.”

[community `README.md`](https://github.com/galion-studio/community/blob/main/README.md)
and [galion-studio `README.md`](https://github.com/galion-studio/galion-studio/blob/main/README.md)
repeat that the desk is not on those pages. The September 2026 Grok Build
note in those READMEs uses “desk” for an operator sitting. It is a review
of a rented CPU, not a desktop product specification.

The private desk’s files are therefore **not** in the public trees read
here. **Gap:** this note cannot describe that desk’s schema, sync, or
ship status, because the authoritative line says the files are elsewhere
and were not fetched.

### 2.2 Obsidian Open Mind, the public agent stack

[Open Mind `README.md`](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/README.md)
calls it “a lightweight, Rust-only foundation for running an AI agent
stack from a CPU box.” The same page says it is not affiliated with the
Obsidian note-taking app. Status on that page: **pre-release**, prepared
for a first public release dated 2026-10-31. “Nothing below claims to be
live in production.”

That README’s feature table mixes crates it says are built with a
parenthetical that some of those crates “live in the development tree and
are synced in before the release.” The tree at `7ba85b60` does contain
`openmind-config`, `openmind-survival`, `guardian-tick`, `openmind-sandbox`,
`persona-plex-relay`, and `edge/openmind-node`. Treat the README sentence
about a missing snapshot as older than the published commit, not as a
second inventory.

[docs/SOURCE-OF-TRUTH.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/SOURCE-OF-TRUTH.md)
(2026-10-20) still says GitHub `obsidian-open-mind` is **private**. The
API on 2026-11-08 reports `private: false`, and `main` is the 2026-10-28
publish merge. **Gap:** the source-of-truth privacy line and the live
visibility disagree. This note does not edit that file.

Voice-side description, written against the same `7ba85b60` commit:
[`docs/galion-voice-v2/OPENMIND-NERVE-CORTEX.md`](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/galion-voice-v2/OPENMIND-NERVE-CORTEX.md).
Galion Voice does not vendor the stack. The crate
[`crates/galion-openmind`](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/crates/galion-openmind/src/lib.rs)
is a consume contract (nerve notes and a Hermes side-band handoff). It
does not clone the other repo and does not open SSH.

### 2.3 Obsidian, the colour

Galion OS and Galion Voice use `obsidian` as the background token
`#0A0A0C`, with amber `#F5A524` and violet `#7C3AED` only in the ground,
and the mark `#F5F5F7` in front. Sources:
[OS `docs/voice-profile/voice-first.md`](https://github.com/galion-studio/galion-operating-system/blob/e48f3d4f6b7f2f0e0ae77ba4784ab427fb54587d/docs/voice-profile/voice-first.md),
[`galion-render` crate docs](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/galion-render/src/lib.rs).
The token is a palette entry. It is not a product.

## 3. Implemented desktop-adjacent surfaces, versus plans

“Implemented” here means source present in the pinned commit. It does not
mean a 2026-11-08 production probe. This agent did not SSH, did not read
`/opt/galion`, and did not restart anything.

### 3.1 In source now

**`galion-desk`** — operator desk endpoint, not a window manager.
[`crates/galion-desk`](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/galion-desk/src/lib.rs)
serves `GET /desk` (JSON snapshot) and `POST /desk` (one act: note, drop,
voice, stamp, hold, wake, rest, focus). Bind default in the cutover note
is loopback `127.0.0.1:8646`. The crate `description` says “Reads the pod,
never writes it.” The module docs say POST applies an act. Both sentences
are in that commit. **Gap:** the crate contradicts itself on write
behaviour; a reader should treat POST as a write of those named acts until
the owner picks one sentence.

[docs/cutover/desk.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/cutover/desk.md)
(2026-10-20) compares a box build with a live binary and states **no
cutover was done**. Its live row, dated that day, names
`galion-desk.service` active since 2026-10-12, binary
`/opt/galion/bin/galion-desk` built 2026-10-10 from a VPS source path that
“is not in any git repo,” and a Python `desk-hermes.py` on every desk
request. The GitHub crate replaces that Python fork with Rust
`desk-hermes`. Those are the document’s measurements. They were not
re-checked on 2026-11-08.

**`desk-hermes`** — read-only JSON snapshot of Hermes state. Wired as the
mindpack child of `galion-desk`.

**`room`** — “a read-only view of the operator’s files as one page”
([`crates/room/Cargo.toml`](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/room/Cargo.toml)).

**`console-ui` plus `console-static`** — server-rendered HTML for the
operator console, std only, no JavaScript on the ported rooms.
[docs/console-ui-decision.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/console-ui-decision.md)
(2026-10-20) says the first pages were built on a branch and **nothing
was deployed** by that decision. It also says two rooms cannot be fully
JavaScript-free: Hermes Research (`/hermes`, microphone and streamed
answers) and the Desk (`/console`, the embedded chat). The JS ruling is
listed as Slavik’s decision and is not recorded as made in that file.
The same note says the Vite dev server was still the production console
on the day of the read, and that `/status` in that tree was a client-side
simulation. The Rust page was written to read `/api/v1/health` instead.

**`galion-one` inside Open Mind** — “the operator’s own ear, mind and mouth
on one box”
([`crates/galion-one/Cargo.toml`](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/galion-one/Cargo.toml)).
[docs/ws-one-desk-tuner.md](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/ws-one-desk-tuner.md)
(2026-10-18) records a Rust port, staged units, and a switch checklist
that “nothing below has been run.” Speech there is an `Unavailable` stub
until `galion-voice-core` supplies a mouth. Persona text in that crate is
Samantha and Jarvis. That is the Open Mind box binary. It is not the
product-voice file in Galion Voice (section 4).

**`apps/phone-voice`** — PWA, “v0.1, not deployed,” streams to
`persona-plex-relay`
([README](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/apps/phone-voice/README.md)).
A phone page, not a desktop shell. The relay itself is described on the
Open Mind README as a scaffold: it loads config and prints a plan, and
opens no socket.

**`galion-boot`** — boot/keep tool (hardware probe, secret restore, unit
renderer, supervisor fallback). It is present. It is not `galion-init`.

**Galion Voice surfaces in this repository** (commit `3e1f5a12`):

- User surface is one dot. [`docs/galion-voice-v2/ONE-UI.md`](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/galion-voice-v2/ONE-UI.md): one screen, one mark, phone page `crates/galion-console/assets/v1-screen.html`.
- [`galion-render`](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/galion-render/src/lib.rs): terminal surface working; PNG sequence working; web JSON working; **`WindowSurface` is a stub** that returns `Unsupported` ([`backends/window.rs`](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/galion-render/src/backends/window.rs)). The comment names a future `softbuffer` + `winit` thread and says a Galion OS build could swap in `/dev/fb0`. The swap is not in the tree. A search of this repository finds no `Fb0DotSurface` and no `galion-init`.
- `galion-console` is the developer TUI. Brand rule: `console_is_user_ui = false` ([`docs/brand/GALION-VOICE-BRAND.md`](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/brand/GALION-VOICE-BRAND.md)).

**Hermes Research laptop window.**
[hermes-research `docs/PLAN.md`](https://github.com/galion-studio/hermes-research/blob/e204789ce73306c645dcb95b4b509cc764ca2c47/docs/PLAN.md)
(7 October 2026): “The window on the laptop is named Hermes Research. It
is a light, not the agent.” The same page says “No new product” for that
cut. The window is named. Its source is not in that three-file docs tree.
**Gap:** no desktop implementation was found next to the name.

### 3.2 Dated live claims (not re-probed)

[Open Mind `docs/STATUS.md`](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/STATUS.md)
computes backlog percent on 2026-10-21: 89 items, done 60.7% unweighted.
The header says “Done” does not mean live, and names only WS-HERMES,
WS-MEM, and B59 as running in production at that writing. Rows used here:

| ID | Status sentence in that file | What this note takes from it |
|---|---|---|
| WS-ODT | D, “partly live.” Live `galion-one` is an 18 October `ws/ui` build; desk and tuner are older binaries. | GitHub `galion-desk` and the process that was listening are different builds, as of that row. |
| WS-HERMES | D, live. `hermes-rs.service` active since 18 October 2026 22:00 CEST; Python `hermes.service` inactive. | A dated claim about the Hermes process, not about a desktop session. |
| WS-MEM | D, live. `openmind-memory.service` on port 8420 since 18 October 2026 22:14 CEST. | A dated claim about memory, not about a desktop session. |
| B05, B06, B53 | D, “not live.” Config-driven desk, One, and tuner imported. “Spare runs only.” | Source landed; the row itself says the spare run is the run. |
| LAUNCH-1 | O. Older text says the public URL returned 404. | Stale relative to the 2026-10-28 public `main`. Left as a row that needs a human refresh. Not treated as current visibility. |

Voice doc, 2026-11-03: on the Cursor host, `/opt/galion/openmind` was not
present, and this voice repository does not create it. Intended VPS
checkout, when a human has deployed it, is `/opt/galion/openmind`, with
`galion-one` recorded as
`ExecStart /opt/galion/openmind/dist/bin-ws/galion-one` in Open Mind’s
voice snapshot. **Gap:** no deploy log after 2026-10-21 was read. Current
production desktop status is unknown.

### 3.3 Plans, not images

**Galion OS** is the direction. The root README says downloads will be
listed once the first image is ready, and that the work starts with Open
Mind and Hermes.
[README](https://github.com/galion-studio/galion-operating-system/blob/e48f3d4f6b7f2f0e0ae77ba4784ab427fb54587d/README.md),
[voice-profile README](https://github.com/galion-studio/galion-operating-system/blob/e48f3d4f6b7f2f0e0ae77ba4784ab427fb54587d/docs/voice-profile/README.md)
(Maciej Grajczyk, 2026-11-02).

| Profile | Ships | Display in the spec | Hermes in the spec | Shared base named in the spec |
|---|---|---|---|---|
| Galion OS Voice | First | No X11, Wayland, or compositor. At most one dot on `/dev/fb0`. | Off the device, side band, outside the 80 ms audio path. | `galion-init`, Obsidian Open Mind contract |
| Hermes GUI | Later | One window, one gradient, one prompt field. No menu and no folder tree. | Centre of that window for knowledge and tools. | `galion-init`, Obsidian Open Mind contract |

The Hermes GUI page is the closest written “desktop”: one window on
obsidian black, agents as presences, important actions ask first
([hermes-later.md](https://github.com/galion-studio/galion-operating-system/blob/e48f3d4f6b7f2f0e0ae77ba4784ab427fb54587d/docs/voice-profile/hermes-later.md)).
It is explicitly later than Voice. It does not choose the Hermes binary
(Rust gateway or the published Python line). It does not place a binary
in the tree.

[PACKAGING.md](https://github.com/galion-studio/galion-operating-system/blob/e48f3d4f6b7f2f0e0ae77ba4784ab427fb54587d/docs/voice-profile/PACKAGING.md)
says nothing in the OS repository is a published binary, and remote gates
R0–R3 stay pending. The Voice archive it specifies would contain three
programs: `galion-init` (static musl PID 1), `galion-voiced` (voice daemon,
T-Thin relay first), and `galion-probe` (machine probe; local run
non-authoritative; remote probe still pending; local label probe v0.1.0).
The archive is a contract in that file. It is not a tarball in the OS git
tree. **Gap:** `galion-init` and `galion-voiced` are absent from the OS
tree, from the Open Mind crate list at `7ba85b60`, and from this Voice
repository. `galion-boot` does not fill the `galion-init` name. This Voice
repository does contain [`galion-probe`](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/galion-probe/src/main.rs),
a std-only CPU readiness probe whose binary string is `galion-probe 0.1.0`.
Ledger row A15 records that probe as landed locally with R0 pending and no
SSH. That row is not a statement that the OS archive has been built, and
this note did not re-run the probe.

[FB0-SURFACE.md](https://github.com/galion-studio/galion-operating-system/blob/e48f3d4f6b7f2f0e0ae77ba4784ab427fb54587d/docs/voice-profile/FB0-SURFACE.md)
is a writer spec. It says the OS repository has no render crate, lists
`Fb0DotSurface` as out of scope for that change, and leaves the optional
window backend (softbuffer / winit) out of scope. The Voice repo’s
`WindowSurface` stub matches that “not yet” on the window side. The
framebuffer path is specified and not present as `Fb0DotSurface`.

Galion Voice V3 admin item 4 records an intent that Hermes Research and
the console integrate into Galion OS as a Linux desktop. The same file
says the item is not in force, does not install a package or a
framebuffer, and that “Galion OS Linux desktop” is the target until an
image is named
([`docs/galion-voice-v3/ADMIN-BACKLOG.md`](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/galion-voice-v3/ADMIN-BACKLOG.md),
2026-11-03).

The Research console plan in Voice is a separate page: one document, a
queue, an open job, one ask field. It says the console is not built in
that landing
([`PLAN-RESEARCH-CONSOLE.md`](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/galion-voice-v2/symphony/PLAN-RESEARCH-CONSOLE.md)).
`galion-console` stays the developer tool. The plan refuses to hang the
desk off the dot server.

## 4. How OS, Open Mind, One, and Voice meet

```text
Galion OS image profiles          not published (docs only, e48f3d4f)
  Voice profile (first)           no display server; dot spec on /dev/fb0
  Hermes GUI (later)              one window + prompt; not built
        │
        │  both name galion-init (missing) and the Open Mind contract
        ▼
Obsidian Open Mind                public Rust stack, main 7ba85b60
  galion-nerve                    loopback :8670 when a human runs it
  hermes-rs / hermes-gateway      analytical agent; not vendored into Voice
  galion-desk :8646               operator JSON desk (source newer than the
                                  Oct 10 live binary in the cutover note)
  console-ui                      operator HTML, partial, JS ruling open
  galion-one                      box voice surface; mouth stub in the
                                  Oct 18 workstream note
  openmind-memory                 dated live claim, port 8420
        │
        │  Voice does not link those crates
        ▼
Galion Voice (this repo, 3e1f5a12)
  one dot                         phone / terminal / PNG; window stub
  galion-openmind                 nerve notes + CortexHandoff
  galion-sideband                 Hermes off the 80 ms tick
  Galion One                      product voice (personas/GALION-ONE.md)
  Galion Research                 product voice + unbuilt console plan
  Samantha, Jarvis                internal test voices
```

Plain Linux is a supported place for Galion Voice. The OS README says
neither profile is required to use the voice program. A desktop image is
not a dependency of the heartbeat.

Two cortices stay separate, as the nerve note already says: Open Mind’s
incident cortex (`gaia-cortex` / nerve detectors) observes and may restart
an allowlisted unit; Voice’s “cortex” is the Hermes side band. The voice
does not ask the incident cortex to solve a question.

Two “Galion One” binaries stay separate:

| | Open Mind `galion-one` | Voice product Galion One |
|---|---|---|
| Where | `crates/galion-one` in `obsidian-open-mind` | Persona [`GALION-ONE.md`](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/galion-voice-v2/personas/GALION-ONE.md) spoken by Galion Voice |
| Role in the sources | Ear, mind, and mouth process on the box | “Main product voice for Galion Voice / Galion One” |
| Persona text in the Open Mind port note | Samantha and Jarvis byte-for-byte | Samantha and Jarvis are test voices, not the product |

The OS voice-profile README says the name Galion One belongs to a separate
product. This note does not merge the box binary and the product voice.

`stack/hermes/DESK.md` in Open Mind is an operator prompt (notes path,
local speech ports, “do not copy secrets”). This context file does not
repeat those paths or any secret location.

## 5. Issues and dependencies

1. **Name split.** Hermes Research BRAND.md (Obsidian = private
   non-public desk; Open Mind = a stance) and the Open Mind README plus
   the OS README (Obsidian Open Mind = the Rust stack the OS contract
   depends on) do not use the words the same way. New desktop text has to
   pick one sentence per name or readers will file the notes app, the
   private desk, and the agent stack as one repo.
2. **Missing PID 1.** Every OS profile depends on `galion-init`. The file
   is not in either pinned tree. `galion-boot` is a different program.
3. **Desk cutover still an owner checklist.** 2026-10-20 cutover doc:
   not done. STATUS WS-ODT: live desk binary older than GitHub. Shipping
   the GitHub crate as if it were already the listening process would
   overwrite that distinction.
4. **Console is an operator surface.** Voice locks keep
   `console_is_user_ui` false. The Research console plan is a different
   product page and is not built in that landing. The Open Mind JS ruling
   for `/hermes` and `/console` is open in the decision record.
5. **Window and framebuffer are specs plus a stub.** OS `FB0-SURFACE.md`
   and Voice `WindowSurface` agree the native window is not the running
   user surface. The running user surface is the dot.
6. **Hermes binary is unpicked.** OS profile notes leave Rust gateway
   versus the published Python line to the owner. Voice talks to Hermes
   through the side band, with `NullTransport` still a real state in the
   Research console plan (lookups `Unreachable` until a human wires
   transport).
7. **Source-of-truth drift.** Privacy line says private; API says public.
   LAUNCH-1 still reads like a 404 era. STATUS percent is 2026-10-21.
8. **Phone PWA and PersonaPlex relay** are scaffold / not deployed in
   their own READMEs. They are not a desktop fallback.
9. **Secrets.** Desk and console docs mention live paths and key files.
   This note does not copy them. A desktop design that needs those paths
   belongs in a human deploy note, not in a context PR.

## 6. Pricing the owner named, and pricing the plans already contain

This task’s owner line, recorded here and **not implemented**: planned
prices **Research Ultra $220** and **One $20**. No billing, meter,
checkout, or `[licence]` edit is part of this file.

What the Voice plans already say, still as plans, with `paid_tiers`
false:

| Written label | Figure in the plan | What the plan says it buys | Billing state in the same pages |
|---|---|---|---|
| Ultra | $220 per month | One CX43-class computer (Titanaxe Max 3XL): Galion OS tenant, CPU agents, Galion Voice. Research is a separate hosted console and does not get a second machine inside Ultra. | Plan only. [`PLAN-ULTRA.md`](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/galion-voice-v2/symphony/PLAN-ULTRA.md) does not boot a machine. `LOCKS.toml` not edited. |
| Plus | $20 per month | CX23-class, voice-to-voice included. | Plan only. [`PLAN-PLUS.md`](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/galion-voice-v2/symphony/PLAN-PLUS.md). Gates in that file are marked open. |
| Galion Research console | no price | Queue, job, ask field. | “This plan does not create a price.” [`PLAN-RESEARCH-CONSOLE.md`](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/galion-voice-v2/symphony/PLAN-RESEARCH-CONSOLE.md) § near the disk note. |
| Brand lock | no public price | Code grant stays free forever. | [`GALION-VOICE-BRAND.md`](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/brand/GALION-VOICE-BRAND.md): `free_forever = true`, `paid_tiers = false`. The $220 host plan is “not the brand.” |
| V3 admin item 5 | no number in that file | Intent that voice-to-voice sits under Galion One and is exclusive to paid Plus and Ultra. | “Not in force.” [`ADMIN-BACKLOG.md`](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/galion-voice-v3/ADMIN-BACKLOG.md). |

**Gap on the labels.** “Research Ultra $220” and “One $20” are the names
in this task. The symphony pages use “Ultra $220” and “Plus $20,” and the
Research console page refuses a price. This note does not rename Plus to
One, and does not rename Ultra to Research Ultra. Storage lines inside
those plans (Ultra 2 GB SSD versus CX43 160 GB, Plus 100 GB) stay in
`PLAN-ULTRA.md` as an unresolved copy. They are not restated here as a
decided disk.

## 7. Recommendations

These are readings of the gaps above. They are not a build order, and
they are not started by this PR.

1. Publish one naming card the OS README, Open Mind README, and Hermes
   Research BRAND.md can all point at: private desk, public agent stack,
   colour token, third-party notes app, Obsidian Security. Until that
   card exists, desktop work should say “Galion OS Hermes GUI” or
   “galion-desk” and avoid a new title.
2. Leave “Obsidian Desktop” unused in repos and download pages until the
   owner says which of those existing things the title means. No tree
   reviewed here is that product.
3. Keep the Voice image profile first, as the OS docs already order it.
   The one-window profile stays a document until `galion-init` and
   `galion-voiced` exist in a repo and an image is listed on the OS README.
   The local `galion-probe` crate does not by itself publish that image.
4. Treat `galion-desk` cutover as an owner action with the 2026-10-20
   checklist. Do not describe the GitHub binary as the live listener.
5. Keep the operator console and the future Research desk off the
   one-dot page. `console_is_user_ui` stays false unless the owner edits
   the lock.
6. Decide the two-room JavaScript question (Research mic, Desk chat)
   before calling `console-ui` the desktop UI. The decision record leaves
   it open.
7. Reconcile Research Ultra / Ultra / One / Plus in an owner pricing note
   before any ledger line. Do not add billing code, a checkout, or a
   `[licence]` change as part of desktop documentation.
8. Refresh STATUS.md live rows and the source-of-truth privacy line with
   a human reading of GitHub and of the box. This file is not that
   refresh. Pair 08 owns the merged-PR audit.
9. Any later desktop process stays off `Heartbeat::step`, the same rule
   the Voice seam and the OS Voice profile already share.

## 8. What this note does not do

- No code, no package install, no image build, no SSH, no training, no
  weight or dataset publication, no `LOCKS.toml` edit, no secret copy.
- No claim that a desktop is deployed, and no claim that it is not
  deployed, past the dated sentences in section 3.2.
- No merge. The pull request that carries this file stays open for a
  human.

## Sources

Pinned blobs. Dates are the dates in the files or on the commits, not
guesses.

- [Obsidian Open Mind README](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/README.md) (commit 2026-10-28)
- [Open Mind source of truth](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/SOURCE-OF-TRUTH.md) (2026-10-20)
- [Open Mind STATUS](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/STATUS.md) (computed 2026-10-21)
- [galion-desk](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/galion-desk/src/lib.rs)
- [desk cutover](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/cutover/desk.md) (2026-10-20)
- [one-desk-tuner workstream](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/ws-one-desk-tuner.md) (2026-10-18)
- [console UI decision](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/docs/console-ui-decision.md) (2026-10-20)
- [galion-one crate manifest](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/galion-one/Cargo.toml)
- [room crate manifest](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/room/Cargo.toml)
- [galion-boot crate manifest](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/crates/galion-boot/Cargo.toml)
- [phone-voice README](https://github.com/galion-studio/obsidian-open-mind/blob/7ba85b603d8b0b87c4d4dede1056e1ee80c767ac/apps/phone-voice/README.md)
- [Galion OS README](https://github.com/galion-studio/galion-operating-system/blob/e48f3d4f6b7f2f0e0ae77ba4784ab427fb54587d/README.md) (commit 2026-11-02)
- [OS voice profiles](https://github.com/galion-studio/galion-operating-system/blob/e48f3d4f6b7f2f0e0ae77ba4784ab427fb54587d/docs/voice-profile/README.md) (2026-11-02)
- [Galion OS Voice profile](https://github.com/galion-studio/galion-operating-system/blob/e48f3d4f6b7f2f0e0ae77ba4784ab427fb54587d/docs/voice-profile/voice-first.md)
- [Hermes GUI profile](https://github.com/galion-studio/galion-operating-system/blob/e48f3d4f6b7f2f0e0ae77ba4784ab427fb54587d/docs/voice-profile/hermes-later.md)
- [Voice profile packaging](https://github.com/galion-studio/galion-operating-system/blob/e48f3d4f6b7f2f0e0ae77ba4784ab427fb54587d/docs/voice-profile/PACKAGING.md)
- [fb0 surface spec](https://github.com/galion-studio/galion-operating-system/blob/e48f3d4f6b7f2f0e0ae77ba4784ab427fb54587d/docs/voice-profile/FB0-SURFACE.md)
- [galion-probe](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/galion-probe/src/main.rs)
- [LEDGER.md](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/LEDGER.md) row A15
- [Hermes Research names](https://github.com/galion-studio/hermes-research/blob/e204789ce73306c645dcb95b4b509cc764ca2c47/docs/BRAND.md) (commit 2026-10-07)
- [Hermes Research plan](https://github.com/galion-studio/hermes-research/blob/e204789ce73306c645dcb95b4b509cc764ca2c47/docs/PLAN.md) (7 October 2026)
- [Hermes Research README](https://github.com/galion-studio/hermes-research/blob/e204789ce73306c645dcb95b4b509cc764ca2c47/README.md)
- [community README](https://github.com/galion-studio/community/blob/main/README.md)
- [galion-studio README](https://github.com/galion-studio/galion-studio/blob/main/README.md)
- [Voice nerve and cortex](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/galion-voice-v2/OPENMIND-NERVE-CORTEX.md)
- [galion-openmind crate](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/crates/galion-openmind/src/lib.rs)
- [One UI](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/galion-voice-v2/ONE-UI.md)
- [galion-render](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/galion-render/src/lib.rs)
- [Window surface stub](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/galion-render/src/backends/window.rs)
- [Galion One persona](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/galion-voice-v2/personas/GALION-ONE.md)
- [Research console plan](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/galion-voice-v2/symphony/PLAN-RESEARCH-CONSOLE.md) (2026-11-03)
- [Ultra plan](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/galion-voice-v2/symphony/PLAN-ULTRA.md) (2026-11-03)
- [Plus plan](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/galion-voice-v2/symphony/PLAN-PLUS.md)
- [Voice brand](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/brand/GALION-VOICE-BRAND.md)
- [V3 admin backlog](https://github.com/galion-studio/galion-one-voice/blob/3e1f5a12c53cd8e0dd47d111676d61d608540559/docs/galion-voice-v3/ADMIN-BACKLOG.md) (2026-11-03)
