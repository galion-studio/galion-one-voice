# Shared punctuation pauses (experimental, default off)

Built pipeline: `target/release/cbx`. Enable on `speak` with
`--punctuation-pauses --pause-reference /absolute/path/to/original/reference.wav`.
Omit the flag for the unchanged original synthesis/audio-writing path.

Companion paired-audio tool (same Rust module):

```sh
pause-tool/target/release/cbx-pauses INPUT.wav NEW_OUTPUT.wav REF.wav TEXT.txt META.txt [--punctuation-pauses]
```

Without the flag this tool byte-copies input, refuses an existing output, and does not load models. Both reference and input must be mono. Pipeline speak reference is converted by the existing audio loader. Companion references can use their native sample rate.

Internal >=200ms reference quiet runs determine the median; sentence target is median and clause target 60%, bounded 80–800ms. Approximate word-fraction punctuation times snap to low-energy boundaries within 250ms. Existing silence is topped up, not doubled. Missing safe boundaries are skipped. No pitch/rate change or new voice profile. No forced alignment claim.

Three unit tests pass (`cargo test --manifest-path pause-tool/Cargo.toml --offline -j 2`). The full pipeline compiles; local build needed explicit `OPENSSL_INCLUDE_DIR=/usr/include`, `OPENSSL_LIB_DIR=/usr/lib/x86_64-linux-gnu`, and `LIBRARY_PATH=../lib` (absolute path recommended).

Full t01–t12 paired tests for both personas are in `../../emotion-fix/validation12/`. Default-off output was byte-identical for all 24 WAVs. Extra pauses did not improve Rust emotion distance for either voice, so this flag is **not recommended on by default**. See `../../emotion-fix/REPORT-12LINE.md` for measured results and limitations.
