# Research notes for Nebula Galion Voice V2

The copied design document is
[`docs/emotion-v2/UNIVERSAL-EMOTION-GAS-TABLE-20261107.md`](../../docs/emotion-v2/UNIVERSAL-EMOTION-GAS-TABLE-20261107.md).
It is the 7 November 2026 universal emotion–gas vocabulary and Demo 13 rows
v2. The original path on the design machine was
`/workspace/Galion/Galion-OS-Design-Package/15-voice/`, and the shared copy
used here is `UNIVERSAL-EMOTION-GAS-TABLE-20261107.md`. “Universal” in that
note is a coverage goal, not a claim that one emotion inventory or one
emotion-to-gas link holds in every culture. Associations are many-to-many
tendencies.

## Colour evidence

- Jonauskaite, D., Abu-Akel, A., Dael, N., et al. (2020). Universal patterns in color–emotion associations are further shaped by linguistic and geographic proximity. *Psychological Science*, 31(10), 1245–1260. 4,598 participants, 30 nations. Red–anger/love, yellow–joy, orange–amusement, pink–love/pleasure, grey–sadness/disappointment, black–fear. Not an exclusive ranking.
- Jonauskaite, D., Parraga, C. A., Quiblier, M., & Mohr, C. (2020). Feeling blue or seeing red? *i-Perception*. Narrower French-speaking sample, not the 30-nation study.
- Jonauskaite, D., et al. (2019). Brown–disgust differences across countries. *Royal Society Open Science* / PMC6774957.
- Hupka, R. B., Zaleski, Z., Otto, J., Reidl, L., & Tarabrina, N. V. (1997). The colors of anger, envy, fear, and jealousy. *Journal of Cross-Cultural Psychology*, 28(2), 156–171.
- Valdez, P., & Mehrabian, A. (1994). Effects of color on emotions. *Journal of Experimental Psychology: General*, 123(4), 394–409.
- Ou, L.-C., Luo, M. R., Woodcock, A., & Wright, A. (2004). A study of colour emotion and colour preference. Part I. *Color Research & Application*, 29(3), 232–240.

Display targets are hand-chosen semantic swatches. Spectral hex values are
the CIE 1931 gamut-clipped swatches already computed in the design document
(CVRL 1 nm colour-matching functions, D65 linear sRGB matrix, negative
channels clamped, peak channel scaled to 1, sRGB transfer). An independent check with the same CVRL 1 nm table reproduced the stored
spectral hex for the demo wavelengths (410.2 `#7000FF`, 486.1 `#00C0FF`,
541.2 `#00FF00`, 575.5 `#FFD300`, 587.6 `#FF8700`, 594.5 `#FF6300`, 656.3
`#FF0000`). This repository
does not invent a second set of spectral hex values. CIEDE2000 between the
13 display targets has a minimum of about 12.7 (Amused `#FF9C64` versus
Excited `#FF6300`). No display target was moved.

Ne I 594.5 is a laboratory neon line, kept as the excited fallback. It is
not a typical H II emission line. He II 541.2, [N II] 575.5, [Cl III], and
[S III] 631.2 are real but weak; the display grade is an explicit lift so
the semantic colour can be seen without pretending the line is intrinsically
dominant.

## Nebula physics used by the simulator

- Elmegreen, B. G., & Scalo, J. (2004). Interstellar turbulence I. *ARA&A*, 42, 211–273. Supersonic turbulence, filaments, Kolmogorov-like motions.
- Kolmogorov, A. N. (1941). The local structure of turbulence. *Dokl. Akad. Nauk SSSR*, 30, 301–305. Velocity amplitude per octave scales near `f^(-1/3)`.
- Hennebelle, P., & Falgarone, E. (2012). Turbulent molecular clouds. *A&A Rev*, 20, 55. arXiv:1211.0637.
- Klessen, R. S., & Glover, S. C. O. (2016). Physical processes in the interstellar medium. Turbulence both supports a cloud and builds clumps that can collapse.
- Bridson, R., Houriham, J., & Nordenstam, M. (2007). Curl-noise for procedural fluid flow. SIGGRAPH 2007. The velocity field is the curl of a noise potential, so it stays nearly divergence-free.
- André, Ph., et al. (2010) and Arzoumanian, D., et al. (2019). Herschel filaments, characteristic width about 0.1 pc. Particles are seeded on open curves, not a sphere.
- NASA, *Decoding Nebulae* and Hubble nebula notes. Emission lines versus dust-scattered starlight. Reflection nebulae are blue-weighted because small grains scatter short wavelengths more efficiently.
- Henyey, L. G., & Greenstein, J. L. (1941). The phase function used for the dust scatter lobe.
- Rayleigh scattering (`λ^-4`) and Beer–Lambert extinction `exp(-τ)` with `τ ∝ density × (550/λ)^4`.

Samantha’s gold and Jarvis’s deep blue are the illuminating-star colours and
a small grade on the scattered veil. Dense gas keeps the emotion’s display
colour.

Emotion V2 coordinates (valence, arousal) come from `emotion-v2/emotions.toml`.
Dominance is the third PAD axis (Russell & Mehrabian, 1977) and is not stored
in that file. Warmth and love are not emotion ids there. Tender is the
warmth-adjacent demo layer. Seductive is Samantha-only and is not one of the
twelve dataset emotions.

## Wiring

Element ids are labels. They do not enter prosody, pitch, or sample synthesis.

- Rust table: `crates/nebula-elements/src/lib.rs` (`DEMO`, `UNIVERSAL`, `label_weight`).
- Python mirror: `emotion-v2/elements.py`.
- Sync test: `tools/emotion-v2/harness/tests/test_elements_sync.py`.
- Harness distribution: `Analysis.distribution` in `tools/emotion-v2/harness/backends.py`.
- Dataset manifest field `element`: `tools/emotion-v2/dataset/emotion_v2_dataset/manifest.py` (`as_dict`). The render and augment functions do not read it.
