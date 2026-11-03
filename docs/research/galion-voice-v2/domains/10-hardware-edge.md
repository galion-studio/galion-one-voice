> **Research synthesis dated 2026-11-03 (Europe/Zagreb).** This page is the domain brief, copied into the repo as docs only. It is not a measurement, not a LOCKS change, and not product code. Citations below are only those in the source brief. None were added on landing.
>
> **LOCKS.toml overrides any conflicting domain text:** no whisper.cpp, no GGML-family containers, no in-tree ORT, `paid_tiers=false`. `chrome_allowed=false`. Consent stays deny-default. Agents do not edit LOCKS.toml.
>
> **Void on this page:** any recommendation to use whisper.cpp or faster-whisper conflicts with LOCKS.toml and is rejected. CPU-class timing notes may stay as context. Hosting prices are infra notes, not product tiers (`paid_tiers=false`).

# Domain 10 — Hardware / Edge Deployment (Galion Voice V2)

Researched: 2026-11-03 (Europe/Zagreb). Scope: where V2 runs. Research only; no product changes.

## Current baseline
- Titanaxe VPS `51.38.137.193`, "CX-class" shared vCPU, CPU-first, exposed via ngrok.
- Note: 51.38.0.0/16 is normally an OVHcloud range, not Hetzner. Check the provider before using the Hetzner pricing below. If the box is on OVH, compare it with OVH VPS and OVH Eco/Rise bare metal.
- Local/Plus/Ultra memory modes exist, but `paid_tiers=false` is LOCKED. The options below are **deployment profiles, not product tiers**. Don't attach pricing or SKUs to them.

## CPU-friendly model evidence
| Component | Evidence | Source |
|---|---|---|
| Whisper.cpp tiny/base/small on 8-core ARM (RK3588) | RTF 0.10 / 0.16 / 0.47; medium 1.51 (slower than real time). RSS 0.3–0.9 GB | [Turing Pi](https://turingpi.com/whisper-cpp-piper-tts-arm64-turing-pi-rk3588/) |
| Piper TTS, medium voice, CPU | RTF ~0.10; the "high" voice is ~0.65 | same |
| Piper, per-sentence latency | Synthesising a whole sentence at once adds ~0.9 s delay, so streaming/chunked decode is needed for good UX | [RK3588 TTS port slides](https://clehaxze.tw/blobs/rk3588-tts-port-en.pdf) |
| FasterWhisper int8 CPU vs ONNX 10 s window | 1101 ms vs 226 ms for a 10 s clip. A shorter encoder window gives a big win | [edge_whisper](https://github.com/ktomanek/edge_whisper) |
| Kokoro on an NPU | ~800 ms TTFA, RTF 0.77, much slower than Piper/Matcha | [rkvoice-stream](https://github.com/suharvest/rkvoice-stream) |

**Takeaway:** on x86 VPS vCPUs (faster per core than RK3588 A76), use whisper.cpp or faster-whisper int8 base/small plus a Piper medium voice with sentence/chunk streaming. That fits a 4–8 vCPU box. Avoid medium/large ASR and Kokoro on shared vCPU if latency matters.

## Option comparison
### A. Shared-vCPU VPS (current)
- **Pros:** cheapest. Hetzner CX23–CX53 legacy list was €5.49–29.49/mo, but CX is **no longer orderable** after the 15 June 2026 price change. The current shared-AMD line is CPX: CPX32 4 vCPU/8 GB €35.49, CPX42 8 vCPU/16 GB €69.49. ([Hetzner price adjustment](https://docs.hetzner.com/general/infrastructure-and-availability/price-adjustment/), [wz-it comparison](https://wz-it.com/en/blog/hetzner-cloud-vs-dedicated-server/))
- **Cons:** noisy-neighbour jitter hits real-time voice p95. You can't upgrade within CX, and rescaling triggers the new pricing.
- **Action:** keep the existing box. Do not rescale casually, because that would lose legacy pricing.

### B. Dedicated-vCPU cloud (CCX)
- CCX23 4 dedicated vCPU/16 GB costs €85.99. Latency is more predictable, but €/vCPU is about 2× an AX bare-metal server. ([wz-it](https://wz-it.com/en/blog/hetzner-cloud-vs-dedicated-server/))

### C. Bare metal (Hetzner AX/EX or OVH equivalent)
- AX41-1-LTD (Ryzen 5 3600, 6c/12t, 64 GB): €57.30/mo, no setup fee, while stock lasts. AX42-1 (Zen4 8c/16t, 64 GB): €97.30 + €49 setup. EX44-1-LTD (i5-13500, 14c/20t, 64 GB): €57.30. ([Hetzner docs](https://docs.hetzner.com/general/infrastructure-and-availability/price-adjustment/), [wz-it](https://wz-it.com/en/blog/hetzner-cloud-vs-dedicated-server/))
- **Pros:** no steal time. AVX2/AVX-512 (Zen4) helps ggml/ONNX. 64 GB RAM holds the Ultra memory profile and several sessions.
- **Cons:** monthly commitment, you run the ops, and failover is up to you.
- **Best value step** once concurrency or jitter becomes a problem.

### D. NPU/edge boxes (RK3588/RK3576, Raspberry Pi 5 + Hailo-8L, Jetson)
- RK3588 measured voice-to-voice latency ~0.7 s. Piper hybrid TTFA ~142 ms, Matcha+Vocos ~145 ms. ([rkvoice-stream](https://github.com/suharvest/rkvoice-stream))
- Pi 5 + Hailo-8L: hybrid Whisper tiny (encoder on NPU, decoder on CPU) takes ~131 ms per 10 s clip, ~8.4× faster than FasterWhisper on CPU. A full-NPU decoder is slower because it has no KV cache. ([edge_whisper](https://github.com/ktomanek/edge_whisper))
- **Cons:** separate toolchains (RKNN/Hailo/TensorRT), model conversion and quantisation, closed SDKs ([slides](https://clehaxze.tw/blobs/rk3588-tts-port-en.pdf)), mainly tiny/base models, and fleet management.
- **Fit:** an offline/private "Local" deployment profile or a demo kiosk, not the main server. Keep the CPU path as the reference implementation and treat the NPU as an optional backend behind the same ASR/TTS interface.

## GStreamer / VAAPI
- Only relevant if the orb or UI ingests or decodes **video**, or if you transcode many streams server-side. Voice-only audio (Opus/PCM) is cheap on CPU and needs neither. Headless VPSs have no GPU, so VAAPI is unavailable there.
- AX42 (Ryzen 8700GE iGPU) or EX (Intel iGPU) bare metal could expose VAAPI if video decode is ever needed. The orb should stay client-side rendering (WebGL/canvas), not server video. **Recommendation: skip for V2.**

## ngrok
- Fine for development and demos. For production, use a stable domain with a reverse proxy (Caddy/nginx + TLS) on the VPS or bare metal, or a Cloudflare Tunnel. WebRTC/WSS voice gets less hop latency without the ngrok relay. (No citation searched; standard practice.)

## Recommendation
1. **Now:** stay on the current CPU VPS. Use whisper.cpp/faster-whisper int8 base(→small) and Piper medium with chunked streaming. Measure p50/p95 TTFA and steal time.
2. **Trigger to move:** p95 jitter caused by steal time, or more than ~2–3 concurrent sessions. Then move to bare metal (AX41-LTD/EX44-LTD ~€57/mo, or AX42), or to a CCX if hourly billing matters.
3. **Edge/NPU:** R&D spike only (RK3588 + rkvoice-stream as reference). Keep it behind the backend interface and don't productize it.
4. Nothing above implies paid tiers (`paid_tiers=false` stays locked).

## Open questions
- Confirm the provider and plan for 51.38.137.193 (OVH vs Hetzner).
- Measure real steal time and an ASR/TTS RTF benchmark on the box itself.

— Maciej Grajczyk
