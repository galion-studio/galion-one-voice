<!--
SPDX-License-Identifier: MIT
Copyright (c) 2026 Galion Labs
-->

# galion-research-gateway

Layer 11 of HORBM. Titan is gone. This process is the local Rust gateway. It
binds `127.0.0.1` only. It does not dial a remote host and it does not call a
paid API.

Galion Research today is the Node desk, `hermes-research`, on
`127.0.0.1:8787`. That port clashes with the HORBM preview, so this binary
does not bind 8787. The desk's model client dials Hermes at
`http://127.0.0.1:8642/v1/chat/completions` and refuses every other host.
This process makes that same dial and no other. It does not read a key and
it does not send `Authorization`.

`NoRemoteLookup` is the test double. It opens no socket. The binary uses
`HermesLookup`. A Hermes miss, a timeout, or a closed port is no answer, and
the layer 5 router turns that into `I don't know.`

## Port

Listen: `127.0.0.1:7911`

Hermes, the only outbound socket: `127.0.0.1:8642`

`GALION_RESEARCH_BIND` may set another listen port. The host must stay
`127.0.0.1`. `0.0.0.0`, `::1`, a hostname, and any other address are refused
before listen.

7911 is in 7900–7999. It is not 7881 (Samantha), 7878 (one-dot), 8787 (the
Node desk), or 11434 (Ollama), and it is not 8642.

The hard cap on one lookup is 3 seconds (`RESEARCH_CAP`), including the
Hermes read. A later body is dropped. The client says timeout, and the
router says `I don't know.`

`POST /v1/lookup` with `{"question":"..."}`. The reply is JSON:
`not_found`, `timeout`, or `answer` plus a `text` and a `source` label.
A URL is not a source label.

The Samantha turn calls this only after HORBM cards miss, and only for a
deep question. Call `galion_sam_horbm::ReasoningRouter`, not this client, for
a finished ASR line.

## Linux

```bash
cd voice-stack/research-gateway
cargo test
cargo run --bin galion-research-gateway
```

From the voice-stack workspace:

```bash
cargo test --manifest-path voice-stack/Cargo.toml -p galion-research-gateway -p galion-sam-horbm -p galion-sam-orch
```

## Windows

The owner's tree is on `G:`. Do not use a `C:` path for this repo, the
knowledge cards, or the cargo target.

```bat
cd /d G:\galion\galion-one-voice\voice-stack\research-gateway
cargo test
cargo run --bin galion-research-gateway
```

Knowledge cards, read only, stay under `G:\galion\knowledge` or
`G:\galion\horbm\knowledge`. This binary does not read them and does not
create that folder.
