# Galion OS base — 2026-11-08

Specialist 02 of the 2026-11-08 owner swarm. This note is the OS-base
record for `galion-one-voice`. It is documentation only. It does not
build an image, add an init, change `LOCKS.toml`, or turn a price into
a charge.

Read against:

- `galion-one-voice` `main` at `3e1f5a12c53cd8e0dd47d111676d61d608540559`
  (merge of pull request #102).
- [`galion-studio/galion-operating-system`](https://github.com/galion-studio/galion-operating-system)
  `main` at [`e48f3d4f6b7f2f0e0ae77ba4784ab427fb54587d`](https://github.com/galion-studio/galion-operating-system/commit/e48f3d4f6b7f2f0e0ae77ba4784ab427fb54587d)
  (2026-11-02). Remote tree listing and file reads. Nothing in that
  repository was cloned into this one.

Boot-sequence detail, including how a boot sound is ordered against
PID 1 and ALSA, is colleague 10's note in this same dated folder. CI
inventory is colleague 08. This file does not repeat either of those.

## 1. What exists, what is specified, and what is absent

The operating-system repository is a document set. Its tree on `e48f3d4`
is `LICENSE`, `README.md`, and `docs/voice-profile/` (`README.md`,
`voice-first.md`, `hermes-later.md`, `PACKAGING.md`, `FB0-SURFACE.md`).
There is no `Cargo.toml`, no kernel, no rootfs, no initramfs, no GitHub
release, and no published download. The root README says downloads will
be listed once the first image is ready
([README](https://github.com/galion-studio/galion-operating-system/blob/e48f3d4f6b7f2f0e0ae77ba4784ab427fb54587d/README.md)).
`PACKAGING.md` §8 says the repository holds the README, the licence, and
that note, and that it has no Cargo workspace and no release workflow
([PACKAGING.md](https://github.com/galion-studio/galion-operating-system/blob/e48f3d4f6b7f2f0e0ae77ba4784ab427fb54587d/docs/voice-profile/PACKAGING.md)).

`galion-init` and `galion-voiced` are names in that packaging note.
Neither string appears in `galion-one-voice`. A search of this tree
found no init binary, no PID 1, and no image builder. The planned boot
in `voice-first.md` is "Linux, then a static musl `galion-init` as
PID 1, then ALSA, then `galion-voiced`", and the same paragraph says no
image is built in that repository
([voice-first.md](https://github.com/galion-studio/galion-operating-system/blob/e48f3d4f6b7f2f0e0ae77ba4784ab427fb54587d/docs/voice-profile/voice-first.md)).

| Piece | Where it is written | What is in a tree today |
|---|---|---|
| Galion OS Voice image | OS `voice-first.md`, `PACKAGING.md` §2 and §6 | Specification. No image, no kernel config, no rootfs. |
| `galion-init` (static musl PID 1, no shell) | OS `PACKAGING.md` §3 and §6 | Name only. Not in either repository. |
| `galion-voiced` (24 kHz, 80 ms, you / me / feeling, ALSA, Opus; T-Thin then maybe T-Edge) | OS `PACKAGING.md` §3 | Name only. The 80 ms clock exists here as `galion-heartbeat`, not as that binary. |
| `galion-probe` | OS `PACKAGING.md` §3 calls the remote probe pending and the local label v0.1.0 | **Code, different job.** `galion-probe/` is a std-only CPU readiness probe. It reads `/proc` and `/sys`, runs in-memory benchmarks, and prints JSON. The banner is `galion-probe 0.1.0` (`galion-probe/src/main.rs`). It is not PID 1 and it is not an image probe of a Galion OS board. `LEDGER.md` row A15: probe v0.1.0 landed locally; R0 pending, no SSH. |
| `/dev/fb0` one-dot writer | OS `FB0-SURFACE.md`, status **SPEC** | Specification. This tree has an offscreen raster and a mailbox, not an fb0 opener. |
| Hermes GUI profile | OS `hermes-later.md` | Later specification. No window program in the OS repo. `WindowSurface::open` in this tree returns `Unsupported`. |
| Plain Linux Galion Voice | OS voice-profile README: neither image is required | **This repository.** Heartbeat, console, render, side band. It is a voice program on ordinary Linux, not a Galion OS image. |
| Hebrew/Israel voice profile | OS pull request #5, then #7 | Removed. `e48f3d4` deletes `docs/voice-profile/he-IL/`. It is not part of the base. |

`PLAN-ULTRA.md` says the same thing from this repository: Galion OS is
the tenant operating environment, and this repository does not ship the
image (`docs/galion-voice-v2/symphony/PLAN-ULTRA.md` §2). The V3 admin
backlog says the nerve group `galion-os` is not, by itself, a desktop
install, and question 15 is still "which image"
(`docs/galion-voice-v3/ADMIN-BACKLOG.md` §4).

## 2. Dependency order

Voice and One come first. An image, a desktop, a Research host, and a
bill are later, and each one is blocked on something that is not in
tree.

1. **Galion Voice, the program.** The 80 ms heartbeat is implemented.
   `FRAME_MS` is 80 (`crates/galion-heartbeat/src/lib.rs`). A late frame
   pauses. Hermes is off that step: `galion-sideband` is a types-only
   contract, `try_send` only, retrieval cap 3 seconds, and the handoff
   is not called from the heartbeat (`crates/galion-sideband/src/lib.rs`).
   The voice program already runs without a Galion OS image. The OS
   notes say that on purpose
   ([voice-profile README](https://github.com/galion-studio/galion-operating-system/blob/e48f3d4f6b7f2f0e0ae77ba4784ab427fb54587d/docs/voice-profile/README.md)).

2. **Galion One, the product voice.** The persona file calls Galion One
   the main product voice for Galion Voice / Galion One, and the
   interface sentence is voice only, one mark, no menu
   (`docs/galion-voice-v2/personas/GALION-ONE.md`). Samantha and Jarvis
   are internal test voices in their own persona files. They are not
   the product a caller is sold. One does not depend on an OS image.
   The OS notes assign the name Galion One to a separate product and
   tell the Voice profile not to use it
   ([voice-first.md](https://github.com/galion-studio/galion-operating-system/blob/e48f3d4f6b7f2f0e0ae77ba4784ab427fb54587d/docs/voice-profile/voice-first.md),
   [voice-profile README](https://github.com/galion-studio/galion-operating-system/blob/e48f3d4f6b7f2f0e0ae77ba4784ab427fb54587d/docs/voice-profile/README.md)).

3. **Voice quality, and a real boot sound beside it.** Owner priority
   for this swarm: a real boot sound sits with voice quality, not after
   the later GUI. The sound is not in tree. Colleague 10 owns the boot
   sequence. See §6.

4. **Galion OS Voice image, still unbuilt.** It depends on the three
   named binaries, a chosen image builder (buildroot or a custom
   initramfs; `PACKAGING.md` §6 says the choice is unmade), a read-only
   root, and an owner ask for an image after the plain-Linux drop-in
   exists. The drop-in archive `galion-os-voice-<version>-<triple>.tar.xz`
   is also unbuilt. The first shape in the packaging note is that
   archive, not the image.

5. **Hermes on the side band, then the Hermes GUI.** The side-band
   crate exists and does no I/O. Which Hermes binary serves it, Rust
   gateway or published Python, is still an owner pick
   ([voice-profile README](https://github.com/galion-studio/galion-operating-system/blob/e48f3d4f6b7f2f0e0ae77ba4784ab427fb54587d/docs/voice-profile/README.md)).
   The GUI profile ships after Voice and is not a dependency of One or
   of the voice program.

6. **Galion Research on a desktop.** V3 backlog item 4 aims Research
   and `galion-console` at a Galion OS Linux desktop. The console is a
   developer tool (`console_is_user_ui = false` in `LOCKS.toml` `[ui]`,
   line 173). The backlog does not install a package, a unit, or a
   framebuffer, and it does not name the image
   (`docs/galion-voice-v3/ADMIN-BACKLOG.md` §4).

7. **Hosted Research Ultra and hosted One.** Plans and prices only.
   See §5. They do not come before the voice program, and they are not
   a reason to build the image first.

`docs/galion-voice-v2/sprint-v2/research/SYMPHONY-STACK-V2-PLAN.md`
lists a later P8 (nerve, Hermes gateway) and P9 (encrypted store,
Galion OS Voice profile packaging). Those rows are a sprint plan. They
are not a booted stack. P9's acceptance line, "Galion OS Voice profile
boots one dot with Hermes off the 80 ms path", is unmet: there is no
image boot to witness.

## 3. Product interfaces

The interfaces below are the ones a person or a neighbouring program
can actually meet, or is told they will meet. The second column is the
evidence class.

| Interface | Class | Contract that is actually written |
|---|---|---|
| One mark on the phone | **Code and page** | `docs/galion-voice-v2/ONE-UI.md`. Page `crates/galion-console/assets/v1-screen.html`. One ellipse, fill `#F5F5F7`. Amber `#F5A524` to violet `#7C3AED` is background only. Hint text and the Art. 50 line are the phone path, not a second graphic. Bound to loopback in the secure-edge note. |
| Heartbeat dot states | **Code** | Idle, listening, speaking. `LOCKS.toml` `[ui]` `dot_states`, `dot_fill = "solid"`, `dot_varies = ["size", "brightness"]`. `galion-render` terminal dot, null surface, PNG sequence, and `rasterize` (`galion-render/src/dot.rs`). G4 status text treats null and terminal as the blessed path; the framebuffer writer is the open piece in the OS spec. |
| Null, terminal, offscreen | **Code** | `NullSurface` records `DotParams` and draws nothing. `TerminalDotSurface` is the terminal one-dot. `WindowSurface` stores a frame and `open()` returns `Unsupported` (`galion-render/src/backends/window.rs`). `FrameMailbox` is the handoff for a future window or fb0 thread. It does not open `/dev/fb0`. |
| `/dev/fb0` one dot | **Specification** | [FB0-SURFACE.md](https://github.com/galion-studio/galion-operating-system/blob/e48f3d4f6b7f2f0e0ae77ba4784ab427fb54587d/docs/voice-profile/FB0-SURFACE.md). Device path exactly `/dev/fb0`. No X11, Wayland, or compositor. Disc `#F5F5F7`. No glyphs. Missing framebuffer is a normal boot, same as `NullSurface`. Writer is not implemented. `docs/agent-wave20/W20-05-STATUS.md` says the fb0 mapping is spec only. |
| Developer console | **Code, not the product UI** | `galion-console`. `console_is_user_ui = false`. The OS voice-first note says the developer console is not the Voice-image interface. |
| Galion One speaking | **Persona instruction** | `docs/galion-voice-v2/personas/GALION-ONE.md`. Product voice. Not an OS service. |
| Galion Research speaking | **Persona instruction** | `docs/galion-voice-v2/personas/GALION-RESEARCH.md`. Lookup voice. Not Galion One. Not a desktop. |
| Hermes side band | **Types, no transport** | `crates/galion-sideband`. No I/O. Gateway choice open. Money-class actions are outside the voice's own grant in the OS voice-first note ("Money is denied"). |
| Nerve group `galion-os` | **Voice event label** | `docs/galion-voice-v2/OPENMIND-NERVE-CORTEX.md` §4. `voice.frame_miss` and `voice.backend_up` use group `galion-os` and unit `galion-os-voice`. That is an inventory group for a nerve process Galion Voice does not start. It is not `galion-init`. |
| Hermes GUI | **Later specification** | One window, one gradient, one prompt field ([hermes-later.md](https://github.com/galion-studio/galion-operating-system/blob/e48f3d4f6b7f2f0e0ae77ba4784ab427fb54587d/docs/voice-profile/hermes-later.md)). Shared base named as `galion-init` plus the Obsidian Open Mind contract. Neither of those is a crate in this repository. `crates/galion-openmind` is the voice-side nerve/handoff crate, not an OS init. |
| Plain-Linux install of three binaries | **Specification** | Unpack, check `SHA256SUMS`, place `galion-init`, `galion-voiced`, and `galion-probe` on the `PATH` of user `galion` ([PACKAGING.md](https://github.com/galion-studio/galion-operating-system/blob/e48f3d4f6b7f2f0e0ae77ba4784ab427fb54587d/docs/voice-profile/PACKAGING.md) §7). The archive does not exist, so this install cannot be performed. |
| Hosted Ultra computer | **Plan** | One CX43-class machine, not booted (`PLAN-ULTRA.md`). Not an interface yet. |
| Billing | **Absent** | See §5. No product interface takes a payment. |

Palette tokens that the OS notes and this tree already share, and that
a later image must not re-invent: obsidian `#0A0A0C`, amber `#F5A524`,
violet `#7C3AED`, dot `#F5F5F7`. They are in `LOCKS.toml` `[ui]` and in
the OS voice-first gradient table. The gradient does not enter the dot.

## 4. Security, storage, and boot gaps

These are gaps against the written contracts. They are not a finding
that the voice program on loopback is unsafe in every path. The phone
path has its own retention note, and that note is about a demo process,
not about an OS image.

### Security

- **No image means no measured secure boot.** There is no signed
  rootfs, no verified boot chain, and no production image without a
  shell, because there is no image. The no-shell rule is
  `PACKAGING.md` §3 and §6. It is untested.
- **Remote gates R0–R3 are pending.** `PACKAGING.md` §7. R0 is the
  probe from the prebuilt archive on a machine with no Rust toolchain.
  The archive is not published. `LEDGER.md` A15 agrees: R0 pending, no
  SSH. This swarm does not SSH.
- **Device identity is unspecified material.** `PACKAGING.md` §3:
  T-Thin includes device identity and owner approval before a device
  joins, and §6 says identity material is absent from the public
  archive. There is no key ceremony and no join protocol in either
  tree.
- **Update path is a hold.** A/B image swap is named and marked
  unimplemented (`PACKAGING.md` §6). `PLAN-V5.md` §6 rollback is a
  symlink of a voice package on a VPS (`/opt/galion/<ver>/`), which is
  a different mechanism and is itself not executed by this note.
- **The phone path is loopback-first, and that is code-level policy,
  not an OS.** `docs/galion-voice-v2/SECURITY-EDGE.md`: mic and turn
  bytes are RAM-bounded; `WebSurface` refuses `0.0.0.0` and `::`
  unless public bind, session, and an exact CORS list are all set;
  this repository does not set the public-bind flag. Staging auth is
  an owner decision dated 2026-11-03 (login before a shared URL). That
  does not harden a Galion OS root.
- **Money stays denied on the Voice profile** in the OS note, and
  `paid_tiers = false` in `LOCKS.toml` `[licence]` (line 182). There
  is no payment component to lock down because none is linked. See §5.
- **Weights stay out of the OS archive.** `PACKAGING.md` §3 sets the
  manifest slot `weights` to `none` and refuses model containers,
  including GGML-family containers, until written counsel sign-off.
  That matches the voice-repo rule that weights are not the MIT grant
  (`docs/brand/GALION-VOICE-BRAND.md`).
- **`harden-dryrun.sh`, `release-dryrun.sh`, and `remote-r0-dryrun.sh`**
  are described as print-only posture *outside* the OS repository,
  with `--execute` exiting 2 (`PACKAGING.md` §7). They are not in the
  OS tree. This note does not go looking for them on a host.

### Storage

- **Image root:** read-only, no package manager, no shell, specified
  and not built (`PACKAGING.md` §6).
- **Ultra disk quota:** unnamed. `PLAN-ULTRA.md` §5 records t284's
  "Ultra $220 / 2GB SSD" and refuses to adopt 2 GB as the workspace,
  because the cited CX43 class disk is 160 GB and the same owner note's
  Plus line says 100 GB SSD. The page does not pick a quota. No host
  was booted (`PLAN-ULTRA.md` §7).
- **Phone demo:** no account store and no raw-audio folder
  (`SECURITY-EDGE.md` "Where bytes live"). That is not the user's
  filesystem on a Galion OS image, which the packaging note does not
  specify beyond a read-only root and "the user's files" as a reason
  2 GB cannot be a full computer (`PLAN-ULTRA.md` §5).
- **Encrypted store** is a P9 row in the symphony stack plan, not a
  crate and not an image feature.
- **VPS unit draft** prefers `NoNewPrivileges` and a read-only root for
  the voice service (`docs/ops/PLAN-V5.md` §7). That unit is a systemd
  service on someone else's Linux. It is not `galion-init`. The plan
  says the service is not deployed until the human steps and a LEDGER
  line say so (`docs/brand/GALION-VOICE-BRAND.md`).

### Boot

- **Specified order:** kernel, `galion-init`, ALSA, `galion-voiced`
  (`PACKAGING.md` §6). Hermes is on another machine. No display server.
- **Not present:** kernel config, initramfs, `galion-init` source,
  ALSA state, a unit that supervises `galion-voiced`, a witness that a
  missing `/dev/fb0` still brings voice up. The fb0 spec requires that
  outcome ([FB0-SURFACE.md](https://github.com/galion-studio/galion-operating-system/blob/e48f3d4f6b7f2f0e0ae77ba4784ab427fb54587d/docs/voice-profile/FB0-SURFACE.md) §1). Nothing boots it.
- **Clock that does exist** is the voice heartbeat, 80 ms, inside this
  repository. An image that eventually runs voice has to keep that
  clock. The image does not create it.
- **Boot sounds:** specified only as a feeling line on the public OS
  page and on the later GUI profile ("Gentle sounds for starting,
  finishing, notices and questions"
  [README](https://github.com/galion-studio/galion-operating-system/blob/e48f3d4f6b7f2f0e0ae77ba4784ab427fb54587d/README.md),
  [hermes-later.md](https://github.com/galion-studio/galion-operating-system/blob/e48f3d4f6b7f2f0e0ae77ba4784ab427fb54587d/docs/voice-profile/hermes-later.md)).
  The first-profile boot order does not name a sound file. This
  repository has no boot-chime asset. Eval listening-set wavs under
  `docs/galion-voice-v2/evals/` are speech samples for voice tests.
  They are not a boot sound, and this note does not publish them.

## 5. Planned prices, not working billing

Owner instruction for this note, 2026-11-08: **Research Ultra is $220**
and **One is $20**. Both figures are **planned**. Billing does not work.
Nothing in this file edits `LOCKS.toml`, adds a checkout, or treats a
plan page as a live SKU.

What the tree already says, kept distinct from that owner sentence:

| Record | Figure | In force as billing? |
|---|---|---|
| This owner note | Research Ultra $220. One $20. | No. Planned names and figures only. |
| `PLAN-ULTRA.md` §1 | Ultra **$220 per month**. Plus **$20**, mentioned so the disk sentence can be read, and "not specified here". | No. The page says it does not boot a machine and does not edit the lock. |
| `ADMIN-BACKLOG.md` §5 | Plus and Ultra would include voice-to-voice under Galion One. Local, free forever, would not. **No price** on that page. | No. The same section says Plus and Ultra are "Not a product" while `paid_tiers = false`. |
| `LOCKS.toml` `[licence]` | `free_forever = true` (line 181), `paid_tiers = false` (line 182). | This is the lock. A plan does not override it. |
| `docs/brand/GALION-VOICE-BRAND.md` | One-dot page shows no price, no trial, no checkout. Changing `[licence]` needs the owner, counsel, and a LEDGER line. | Public text matches the lock, not the plan prices. |
| OS voice-profile README | "There is no paid tier, account, or licence key for the voice program." | The free download of the voice program. It is not a statement that a future hosted Research Ultra is free. It is a statement that today's program is not sold. |
| Code search | No payment integration in this tree. `stripe` does not name a client. `paid_tiers` occurs as the lock and as docs that repeat the lock. | There is no working bill, invoice, entitlement, or account store. |

Research Ultra in this note is the hosted research computer the Ultra
plan describes (Galion OS + agents + voice, CX43-class, CPU-only,
Hermes off the 80 ms tick), priced by the owner at $220. It is not a
second price beside `PLAN-ULTRA.md`. One at $20 is the owner's planned
price for the One product. This note does not rename Plus to One, and
it does not copy Plus's 100 GB disk onto One. Those storage numbers
stay the conflict recorded in `PLAN-ULTRA.md` §5 until the owner
resolves them.

`ADMIN-BACKLOG.md` §5 already lists what would have to happen before a
paid tier is true: the owner edits `[licence]`, counsel signs, LEDGER
records it, the brand text is updated afterwards, and the owner says
what Local still includes. None of those steps have been done. A $220
or $20 figure in this file does not do them.

## 6. Real boot sounds, next to voice quality

Owner priority added for this swarm: **real boot sounds**, at the same
rank as voice quality.

What that means for the base, without taking colleague 10's boot
detail:

- The sound is part of bringing **Galion Voice and Galion One** up. It
  is not a decoration for the later Hermes GUI, which is the only OS
  note that already mentions start and finish sounds.
- It has to be a real asset with a licence the owner accepts. The
  listening-set wavs already in this repository are not that asset.
  This note does not add audio.
- It has to survive the Voice-image rules already written: no shell on
  the production image, ALSA comes up before `galion-voiced`, a missing
  framebuffer still boots voice, and the 80 ms speech path is not
  blocked by the chime. Those constraints are for colleague 10 to
  schedule. They are recorded here so a boot-sound design cannot move
  the chime onto `Heartbeat::step` or into the Hermes GUI-only profile
  and call the priority done.
- Acceptance is not "the README says gentle sounds". Acceptance is a
  file, a trigger after ALSA and before or as voice becomes ready, a
  mute and a night silence that the GUI note already promises, and a
  witness on a machine. None of those exist today.

Colleague 10 covers the boot detail. This section is the priority and
the gap.

## 7. Acceptance TODOs

Each item is open. "Done" would require the evidence in the last
column. Writing this note does not check any row.

| ID | Todo | Evidence that would close it | Evidence today |
|---|---|---|---|
| OS-1 | Publish the plain-Linux drop-in before any image. Archive layout in `PACKAGING.md` §4, both musl triples, `weights=none`, checksums, keyless cosign after a green release run. | A release asset, `SHA256SUMS`, and a manifest whose `git` is a commit that contains the three binaries. | OS repo has no workflow and no release. `galion-init` and `galion-voiced` are absent here. `galion-probe` exists and is not that archive. |
| OS-2 | Owner picks the image builder. `PACKAGING.md` §6 leaves buildroot vs custom initramfs unchosen. | A one-line owner choice, recorded in the OS repo, with no agent inventing the builder. | Unchosen. |
| OS-3 | Build the Voice image only after OS-1 and OS-2, and only when the owner asks. Read-only root, no shell, `galion-init` as PID 1, no display server. | An image file the owner names, plus a boot log showing PID 1 and voice up. | `voice-first.md` and `PLAN-ULTRA.md` both say the image is not shipped. V3 question 15 is unanswered (`ADMIN-BACKLOG.md` §4). |
| OS-4 | Boot voice with no framebuffer. | A boot where `/dev/fb0` is missing and `galion-voiced` still runs, matching `FB0-SURFACE.md` §1. | No image boot. `NullSurface` in this tree is the analogous code path, not an image test. |
| OS-5 | Implement `Fb0DotSurface` against the closed ioctl set in `FB0-SURFACE.md`, or explicitly keep the image on `NullSurface`. | A writer that opens only `/dev/fb0`, takes the non-blocking lock, does not call `FBIOPUT_VSCREENINFO`, and draws one disc. Or an owner line that the first image has no pixels. | `FrameMailbox` plus `rasterize` exist. `WindowSurface::open` is `Unsupported`. W20-05 calls fb0 spec only. |
| OS-6 | Keep Hermes off the 80 ms path on the image. | A boot trace with no Hermes process on the Voice image, and side-band calls only via `try_send` off `Heartbeat::step`. | True of the crates in this repo (`galion-sideband`). Unwitnessed on an image, because there is no image. |
| OS-7 | Real boot sound at voice-quality priority. | An owned asset, ALSA playback that does not stall the 80 ms clock, mute, and night silence. Sequence written by colleague 10. | No boot-chime file. GUI prose mentions gentle sounds. First-profile boot order does not. |
| OS-8 | Voice quality bar for Galion One stays ahead of the image and ahead of billing. | The voice program's own gates, recorded in LEDGER, for the One persona. Not a Galion OS download. | One is a persona instruction. This note does not score it. |
| OS-9 | Resolve storage before calling Ultra or One a computer. | Owner choice among the three readings in `PLAN-ULTRA.md` §5. Do not silently replace 2 GB with 160 GB. | Quota unnamed. |
| OS-10 | Prices stay planned until the lock changes. Research Ultra $220 and One $20 are not offered. | Owner edit of `[licence]`, counsel, LEDGER line, then brand text, as `ADMIN-BACKLOG.md` §5 and `GALION-VOICE-BRAND.md` already require. A payment flow that can take a charge and can refuse one. | `paid_tiers = false`. `free_forever = true`. No payment integration. One-dot page has no checkout. |
| OS-11 | Do not treat the nerve group `galion-os` as the operating system. | Any OS status page that distinguishes the event group from an image. | This note. The group remains a label in `OPENMIND-NERVE-CORTEX.md`. |
| OS-12 | Remote probe R0, only after OS-1, from the archive, by the owner. | An R0 result the owner attaches. Agents do not SSH. | `PACKAGING.md` §7 and `LEDGER.md` A15: pending. |

## 8. What this note does not do

- It does not implement init, a framebuffer writer, a boot chime, or a
  payment.
- It does not merge, deploy, SSH, or edit `LOCKS.toml`, `SOUL.md`, or
  `config.yaml`.
- It does not change the MIT licence text or the visibility of either
  repository. Galion OS stays the public MIT document set it already is.
  Galion Voice stays under the perpetual open MIT grant the lock names.
- It does not publish weights, credentials, or the eval wavs.
- It does not inventory CI (colleague 08) and it does not write the
  boot timeline (colleague 10).

— Specialist 02, 2026-11-08. Sources are the commits and paths cited
above. No host was booted for this note.
