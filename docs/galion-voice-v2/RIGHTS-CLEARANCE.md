# V2-S14 · Rights clearance (Samantha, Jarvis)

Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
Europe/Zagreb. A checklist. Record only. **Not legal advice.** Counsel
writes the opinion. This file does not grant a right, and it does not
check a box.

Master slogan: "The seamless connection with the human being, on the voice only."

The legal hold lives in `VOICE-CLONING.md`. This file is the gate under
that hold: what has to be true, per persona, before a film voice may be
fetched, stored, sent to the clone worker, or heard in a public demo.
The matching record is a line in `LEDGER.md`, pasted from the templates
at the bottom. Until that line exists, the gate is closed.

No film audio is fetched here. No model is downloaded. No clone is run.

## Two uses of the same names

| Name | What it is today | What this checklist gates |
|---|---|---|
| `samantha` | A session register. Warm, conversational. Tags later. Opens the session. `SAMANTHA-V2.md`, `BusBody::Persona`. Piper or flite speaks. | The 2014 film performance of Samantha in *Her*, as a conditioning voice. |
| `jarvis` | A session register. Calm, short command lines. Tags off. The current `en_US-ljspeech-medium` checkpoint is not a British accent. | The Iron Man film performance of that voice, as a conditioning voice. |

A register id is allowed. Describing the Piper voice as either performer
is not. The canned line stays "I'm Galion". `LOCKS.toml` `[ui]`
`chrome_allowed = false`: no nameplate, no caption of the id on the mark.

## The hold, restated

Cloning a performer's voice without rights is high risk. The comparison
the owner requires is public: in May 2024 OpenAI withdrew the Sky voice
after Scarlett Johansson said it resembled her performance in *Her*.
OpenAI said Sky was not an imitation of her. The dispute is the warning,
not a licence. A claim that an output "is not an imitation" does not
clear this gate.

Jarvis, as an Iron Man film voice, and Samantha, as the voice of *Her*
(2014), are **owner-directed research targets**. One reference of about
ten seconds is the intended conditioning clip for each, once rights
exist. Chatterbox-Turbo's own code asserts a reference longer than five
seconds. About ten seconds satisfies that assert and the owner's target.
Zero-shot. No fine-tune. No cloud TTS API.

Before any of the following, rights, consent, and licence have to be
cleared and written in `LEDGER.md`:

- fetching or storing a film clip
- calling a clone worker with that clip
- a public demo that speaks in either voice

`LOCKS.toml` `[formats.exceptions.v2s14_chatterbox]` sets
`reference_clips = false`. The worker treats a non-empty `reference`,
`audio_prompt`, `audio_prompt_path`, or `clip` as exit 3 and writes no
PCM (`CLONE-WORKER.md`). `scripts/clone/fetch-clone-weights.sh` fetches
pinned `.safetensors` only when `GALION_CLONE_FETCH=1`, and it refuses
audio suffixes. A LEDGER clearance does not flip the lock and does not
set that flag. An owner edits the lock later, in its own commit, after
the clearance line is real.

Chatterbox code is MIT, copyright 2025 Resemble AI, read 2026-11-03.
That licence covers the code. It does not cover a film performance, a
publicity right, or a clip. Perth's licence was not read. Do not vendor
Perth as part of a clearance.

Seat 8 in `VOICE-CLONING.md` is this gate. The checklist can be written
while the gate stays closed. That is the state of this file.

## How a box is allowed to be ticked

Tick a box on a copy that stays with the instrument (counsel's file, not
this repo), or in a later pull request that also pastes the matching
LEDGER template with every field filled. A box ticked in git without
that line is not clearance.

Leave every box in this file unchecked. Do not paste a template while a
`[FIELD]` is still in it. A remaining field means the fact is unknown.
Do not invent a counsel name, a rightsholder, a date, or a sha256.

One persona can clear without the other. A private clearance does not
open a public demo. A refusal on either persona closes only that persona.

Caller consent is a different gate. `LOCKS.toml` `[consent]` (`sense`,
`adapt`, `remember`, `share`, default deny) is about the person on the
phone. It is not the performer's consent. Withdrawing `sense` does not
withdraw a film licence, and a film licence does not grant `sense`.

## Shared rules

These apply to both personas. Each checklist repeats them so one persona
can be cleared on its own page.

### Consent

The performer, or a representative the instrument shows is authorised,
signs a written consent. A press quote, a lack of objection, a public
credit, or the Sky dispute is not consent.

The consent has to tell the performer that listeners may recognise the
film performance, and that the machine will speak new lines in that
voice. It names Galion Voice, the grantee counsel identifies (the open
A21 question is whether that is Maciej Grajczyk, FRXDEL STUDIOS, or the
Galion trade name), and the acts below.

Permitted, and no wider:

- one clip of about ten seconds
- local, self-offline, zero-shot conditioning (`audio_prompt` on the
  Chatterbox path), CPU, out of process
- no fine-tune, no training set, no cloud upload, no sold or sublicensed
  voice
- withdrawal deletes the clip and any local derivative (speaker
  embedding, prompt-token cache). This scaffold writes none. A later
  worker must not keep one after withdrawal.

Counsel covers personality, publicity, and moral rights for the
performer's domicile and for Croatia / the EU, including the moral-rights
question already open in `docs/agent-wave20/A21-COUNSEL-MEMO.md`. This
checklist does not answer it.

Both performers are adults. This list is not a path for a minor's voice.

### Film-clip rights

Copyright in the picture and soundtrack is a second signature, separate
from the performer. The licence names the work (title and year) and the
rightsholder **as printed on the instrument**. Copy that name. Do not
guess it from a credit roll or a database.

The clip grant is the same width as the consent: one extract of about
ten seconds, stored off git, used only as the local conditioning
reference, not redistributed, not posted, not made a training set.
Quotation, "research", or a fair-use label is not the clearance the
owner required.

After the file exists on the owner's machine, record duration and sha256
in the LEDGER line. Compute the hash from the file. Do not invent one.
The wav does not enter git, and it does not enter `weights/clone/`
(that directory is the pinned safetensors tree). No scene list, no
timestamp, and no source URL belong in this repo.

### Licence

The instrument is a signed writing, or a written refusal. A conversation
is not an instrument. Counsel reads it before the owner treats it as
clearance. One writing may hold both the consent and the clip grant if
the signatories are the right ones. Two thin letters that leave a gap
are not one licence.

The writing states: grantor, grantee, persona, work, permitted acts,
forbidden acts, territory, term, revocation, the credit line (or a
written agreement to use none), and the disclosure line the public demo
must speak. Galion code stays the Perpetual Open MIT licence. The voice
is not MIT. Weights stay out of git (`weights_vendored = false`).

### Public-demo gates

A public demo is any sharing with a person who did not build the tree,
including one teammate and including a tunnel URL. Loopback on the
owner's machine is the private case. It still needs consent and the clip
licence before a clip exists. It does not need the login gate, because
nothing is shared.

Staging and any public demo follow `SECURITY-EDGE.md` and the owner
decision of 2026-11-03 (Plan V5 H3):

- login before the URL is shared: ngrok OAuth or basic auth
- an unauthenticated request fails at the edge (401 or an OAuth
  redirect) and does not reach loopback
- CORS is the exact `https://` origin, no wildcard; the unset-variable
  `*` is the local demo only
- no password, OAuth secret, authtoken, or allow-listed email in the repo
- the app stays on loopback; `GALION_ALLOW_PUBLIC_BIND` is not set here
- AI Act Art. 50 stays on. The page shows and the first tap speaks:
  - HR: `Razgovarate s umjetnom inteligencijom. Galion je AI asistent, a ne osoba.`
  - EN: `You are talking to an AI. Galion is an AI assistant, not a person.`
- `GALION_AI_DISCLOSURE=0` is for benches. A public demo leaves it on.
- Beside those two sentences, the demo shows and speaks the imitation
  line for that persona (wording in the persona sections). Art. 50 says
  the speaker is an AI. The extra line says whose performance it imitates.
- When `generate()` is actually on, the Perth watermark stays beside the
  Art. 50 disclosure. Perth is not vendored by this clearance.
- No PCM on disk. No transcript in the demo log. No film wav in git.
- Piper (`en_US-ljspeech-medium`) speaks until the clone slot is present
  **and** this persona's public-demo LEDGER line is pasted. Flite remains
  the pack-missing fallback under Piper.

Private clearance and public-demo clearance are separate LEDGER lines.
Do not combine them.

## Samantha

Research target: the voice of Samantha in *Her* (2014). Register today:
conversational and emotional; paralinguistic tags are for this id when a
model is allowed to keep them. The slogan's "seamless connection" is that
register: warmth, timing, pauses. The film voice is not that register.

The performer the consent has to name is Scarlett Johansson, or a
representative the instrument shows is authorised to sign for her. The
Sky withdrawal is why her name is on this list. It is not her consent.

### Consent

- [ ] Written consent from Scarlett Johansson, or from a representative the instrument shows is authorised.
- [ ] The consent says listeners may recognise the Samantha performance in *Her* (2014), and that Galion will speak new lines in that voice.
- [ ] Grantee is the name counsel uses, copied from the instrument.
- [ ] Scope is one clip of about ten seconds, local zero-shot conditioning, no fine-tune, no cloud upload, no sale or sublicense of the voice.
- [ ] Withdrawal is written: who may revoke, how notice arrives, and that the clip and any local derivative are deleted.
- [ ] Counsel has written that personality, publicity, and moral rights for this performer and this use are covered. The opinion is kept with the instrument, not committed if it holds personal data.

### Film-clip rights

- [ ] A separate written grant, or the same instrument if the signatory holds the film right, covers the picture and soundtrack of *Her* (2014).
- [ ] The rightsholder is copied from the instrument. No name was guessed in this file.
- [ ] The grant is one extract of about ten seconds, off git, conditioning only, not redistributed, not posted, not a training set.
- [ ] Duration and sha256 are computed from the local file after it exists, then written in the LEDGER line. The wav stays out of git.

### Licence

- [ ] Counsel has read the signed writing before it is treated as clearance.
- [ ] Territory, term, credit line, and revocation are filled in. Empty means not cleared.
- [ ] The writing does not depend on the Chatterbox MIT licence, on a dropped cloud API, or on a "not an imitation" claim.

### Public-demo gates

Leave this group unchecked for a private clearance.

- [ ] The private LEDGER line for Samantha is already pasted, with no `[FIELD]` left.
- [ ] The URL is behind ngrok OAuth or basic auth. An unauthenticated request fails at the edge.
- [ ] CORS is one exact `https://` origin. No wildcard on this demo.
- [ ] Art. 50 HR and EN stay on, in the words `SECURITY-EDGE.md` already fixes.
- [ ] The demo also shows and speaks: `This voice is a synthetic imitation of the Samantha performance in Her (2014). It is not Scarlett Johansson.`
- [ ] The Croatian line beside it is the one the owner confirmed for this demo: `Ovaj je glas sintetička imitacija izvedbe Samanthe u filmu Her (2014.). Nije to Scarlett Johansson.` Confirm the sentence before the URL is shared. Do not swap in a paraphrase at demo time.
- [ ] No caption on the mark calls the voice Samantha or names the performer (`chrome_allowed = false`).
- [ ] The public-demo LEDGER line is pasted. The URL stays unshared until that line exists.

## Jarvis

Research target: the Iron Man film voice. Register today: calm, precise,
command and interface. Short lines. Tags off. The film voice is not that
register.

The performance the owner named is the one publicly credited to Paul
Bettany as J.A.R.V.I.S. Counsel confirms that credit against the specific
film the clip comes from, before anyone is asked to sign. A credit is not
consent. The instrument names the picture (title and year). "Iron Man
films" as a set is not a specific grant.

The character and the name are a third question. A performer's voice
grant does not license the J.A.R.V.I.S. character or the name Jarvis.
The public demo does not present Galion as that character. Counsel says
whether the internal register id `jarvis` may stay. The locked surface
still does not draw the name.

### Consent

- [ ] Written consent from Paul Bettany, or from a representative the instrument shows is authorised, after counsel confirms he is the performer of the named film.
- [ ] The consent says listeners may recognise that Iron Man film performance, and that Galion will speak new lines in that voice.
- [ ] Grantee is the name counsel uses, copied from the instrument.
- [ ] Scope is one clip of about ten seconds, local zero-shot conditioning, no fine-tune, no cloud upload, no sale or sublicense of the voice.
- [ ] Withdrawal is written: who may revoke, how notice arrives, and that the clip and any local derivative are deleted.
- [ ] Counsel has written that personality, publicity, and moral rights for this performer and this use are covered.

### Film-clip rights

- [ ] The instrument names one picture, title and year, not the series as a whole.
- [ ] A written grant covers that picture and its soundtrack. The rightsholder is copied from the instrument.
- [ ] Counsel has separated the voice grant from the character and the name, and has written whether the register id `jarvis` may remain.
- [ ] The grant is one extract of about ten seconds, off git, conditioning only, not redistributed, not posted, not a training set.
- [ ] Duration and sha256 are computed from the local file after it exists. The wav stays out of git.

### Licence

- [ ] Counsel has read the signed writing before it is treated as clearance.
- [ ] Territory, term, credit line, and revocation are filled in.
- [ ] The writing does not depend on the Chatterbox MIT licence or on a dropped cloud API.
- [ ] The writing does not authorise Galion to speak as Marvel's assistant or to use the character as a product name.

### Public-demo gates

Leave this group unchecked for a private clearance.

- [ ] The private LEDGER line for Jarvis is already pasted, with no `[FIELD]` left.
- [ ] The URL is behind ngrok OAuth or basic auth. An unauthenticated request fails at the edge.
- [ ] CORS is one exact `https://` origin. No wildcard on this demo.
- [ ] Art. 50 HR and EN stay on, in the words `SECURITY-EDGE.md` already fixes.
- [ ] The demo also shows and speaks: `This voice is a synthetic imitation of the J.A.R.V.I.S. performance in [FILM TITLE, YEAR]. It is not [PERFORMER]. Galion is not that character.` The two brackets are filled from the instrument, then the brackets are removed.
- [ ] The Croatian line beside it is the one the owner confirmed, in the same shape: `Ovaj je glas sintetička imitacija izvedbe J.A.R.V.I.S.-a u filmu [NASLOV, GODINA]. Nije to [IZVOĐAČ]. Galion nije taj lik.`
- [ ] No caption on the mark says Jarvis or names the performer.
- [ ] The public-demo LEDGER line is pasted. The URL stays unshared until that line exists.

## Order, when a human is ready

1. Counsel opinion on the signed consent and the signed clip grant.
2. Paste the private LEDGER template for that persona. Fill every field.
3. Only then may a human place the one clip on the owner's machine, off git.
4. Compute duration and sha256. Amend the LEDGER line with those two facts. Do not invent them in advance.
5. A later owner commit may set `reference_clips` true for that persona. This file does not edit `LOCKS.toml`. The worker keeps refusing a clip until that commit exists.
6. A public demo waits for its own LEDGER line, after the private line, with login, exact CORS, Art. 50, and the imitation sentence.
7. Weight fetch (`GALION_CLONE_FETCH=1`) is a different decision. Clearance of a voice does not download Nano or Turbo. This file does not set the flag.

A refusal or a withdrawal uses the refused template. Delete any local
clip and derivative. Piper speaks. The register id may remain, because
it is not the film voice.

## LEDGER paste templates

Paste one block at the end of `LEDGER.md`. Keep the bold date line.
Europe/Zagreb. Replace every `[FIELD]`. If a field is unknown, do not
paste the block.

The private blocks do not open a demo. The public blocks are invalid
unless the private block for the same persona is already in the file.
The refused blocks close that persona. They do not flip
`reference_clips`, and they do not authorise a download or a run.

### Samantha, private

```text
**[YYYY-MM-DD HH:MM] CET · V2-S14 rights gate · Samantha · PRIVATE CLEARED.** Maciej Grajczyk. Counsel opinion: [COUNSEL NAME], [OPINION DATE], kept with the instrument (not in git if it holds personal data). Performer consent: [GRANTOR], [INSTRUMENT DATE], [SCOPE IN ONE LINE]. Grantee: [GRANTEE AS NAMED]. Film licence: Her (2014), rightsholder [NAME AS PRINTED ON THE INSTRUMENT], [INSTRUMENT DATE]. One extract of about ten seconds. Local zero-shot conditioning only. No redistribution. No fine-tune. No cloud upload. Clip off git. sha256 [COMPUTED FROM THE LOCAL FILE]. Duration [SECONDS]. Withdrawal: [WHO MAY REVOKE, AND HOW]. Public demo still closed. reference_clips stays false until a later owner edit. No generate in this entry. No weight download in this entry. Piper speaks.
```

### Samantha, public demo

```text
**[YYYY-MM-DD HH:MM] CET · V2-S14 rights gate · Samantha · PUBLIC DEMO OPEN.** Maciej Grajczyk. Private line already in this ledger. Login: ngrok [OAuth or basic auth]. Unauthenticated request fails at the edge ([401 or OAuth redirect], checked [DATE]). CORS exact origin: [https://ORIGIN]. No wildcard. Art. 50 on, HR and EN, words from SECURITY-EDGE.md. Imitation line shown and spoken: "This voice is a synthetic imitation of the Samantha performance in Her (2014). It is not Scarlett Johansson." Croatian line confirmed by the owner: "Ovaj je glas sintetička imitacija izvedbe Samanthe u filmu Her (2014.). Nije to Scarlett Johansson." No credential in the repo. No nameplate on the mark. URL unshared before this line. Perth watermark stays beside Art. 50 when generate() is on. Perth is not vendored here.
```

### Samantha, refused or withdrawn

```text
**[YYYY-MM-DD HH:MM] CET · V2-S14 rights gate · Samantha · REFUSED.** Maciej Grajczyk. [WHO REFUSED OR WITHDREW], [DATE]. No clip kept. Any local file and any local derivative deleted. Public demo closed. reference_clips false. No generate. No weight download. Piper speaks. The register id samantha remains a session register only (SAMANTHA-V2.md). It is not the film voice.
```

### Jarvis, private

```text
**[YYYY-MM-DD HH:MM] CET · V2-S14 rights gate · Jarvis · PRIVATE CLEARED.** Maciej Grajczyk. Counsel opinion: [COUNSEL NAME], [OPINION DATE], kept with the instrument (not in git if it holds personal data). Performer consent: [GRANTOR], [INSTRUMENT DATE], credit confirmed against [FILM TITLE, YEAR]. Grantee: [GRANTEE AS NAMED]. Scope: [ONE LINE]. Film licence: [FILM TITLE, YEAR], rightsholder [NAME AS PRINTED ON THE INSTRUMENT], [INSTRUMENT DATE]. Character and name: [COUNSEL'S SENTENCE ON WHETHER THE REGISTER ID jarvis MAY STAY]. Galion is not presented as that character. One extract of about ten seconds. Local zero-shot conditioning only. No redistribution. No fine-tune. No cloud upload. Clip off git. sha256 [COMPUTED FROM THE LOCAL FILE]. Duration [SECONDS]. Withdrawal: [WHO MAY REVOKE, AND HOW]. Public demo still closed. reference_clips stays false until a later owner edit. No generate in this entry. No weight download in this entry. Piper speaks.
```

### Jarvis, public demo

```text
**[YYYY-MM-DD HH:MM] CET · V2-S14 rights gate · Jarvis · PUBLIC DEMO OPEN.** Maciej Grajczyk. Private line already in this ledger. Film named in that line: [FILM TITLE, YEAR]. Performer named in that line: [PERFORMER]. Login: ngrok [OAuth or basic auth]. Unauthenticated request fails at the edge ([401 or OAuth redirect], checked [DATE]). CORS exact origin: [https://ORIGIN]. No wildcard. Art. 50 on, HR and EN, words from SECURITY-EDGE.md. Imitation line shown and spoken: "This voice is a synthetic imitation of the J.A.R.V.I.S. performance in [FILM TITLE, YEAR]. It is not [PERFORMER]. Galion is not that character." Croatian line confirmed by the owner: "Ovaj je glas sintetička imitacija izvedbe J.A.R.V.I.S.-a u filmu [NASLOV, GODINA]. Nije to [IZVOĐAČ]. Galion nije taj lik." No credential in the repo. No nameplate on the mark. URL unshared before this line. Perth watermark stays beside Art. 50 when generate() is on. Perth is not vendored here.
```

### Jarvis, refused or withdrawn

```text
**[YYYY-MM-DD HH:MM] CET · V2-S14 rights gate · Jarvis · REFUSED.** Maciej Grajczyk. [WHO REFUSED OR WITHDREW], [DATE]. No clip kept. Any local file and any local derivative deleted. Public demo closed. reference_clips false. No generate. No weight download. Piper speaks. The register id jarvis remains a session register only. It is not the film voice. A voice refusal does not, by itself, decide the character-name question; counsel's last written sentence on the id still stands.
```

## State of this writing

2026-11-03, Europe/Zagreb. Every box above is unchecked. Both private
gates are closed. Both public-demo gates are closed. No clip is in the
tree. No sha256 of a clip is recorded, because no clip was fetched.
`reference_clips` remains false. Seat 8 is the checklist, not a
clearance.

The same day, the owner directed the Chatterbox Nano worker to be built.
That direction is not an instrument, and it does not fill a field above.
`scripts/clone/consent.py` reads a `consent.txt` the operator places
outside git. The required keys are the ones in this checklist (date,
persona, work, performer, grantor, instrument, grantee, scope,
public_demo, expiry, operator, withdrawal, counsel, sha256, duration_s).
An empty key, a bracket, or a placeholder word is a refusal. The worker
still refuses `op: condition` while `reference_clips` is false. No box
in this file was ticked. No LEDGER template was pasted.

— Maciej Grajczyk
