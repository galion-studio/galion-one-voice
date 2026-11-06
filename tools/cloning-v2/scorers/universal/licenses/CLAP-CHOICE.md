# CLAP model choice

- **Intended:** microsoft/msclap 2023 (HF card `ms-pl`, code MIT). Commercial OK.
- **Runtime blocker on this box:** Python 3.13 has no `numpy<2` binary wheels; `msclap 1.3.4` requires `numpy>=1.23,<2` and `transformers<5`. Building numpy from source needs a C++ toolchain not present.
- **Fallback used (explicitly allowed by brief):** `laion/clap-htsat-fused` via Hugging Face `transformers.ClapModel` — **Apache-2.0** (commercial OK).
  - URL: https://huggingface.co/laion/clap-htsat-fused
  - Weights file: `model.safetensors` (~586 MB)

## 2026-11-05 update
Yelling / crying / laughter are now PRIMARY on EfficientAT (source `efficientat`); CLAP keeps love / seductive / logic / reason only.
