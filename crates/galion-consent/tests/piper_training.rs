// Galion Voice — Perpetual Open MIT License
//! Piper fine-tune addendum gate (owner t662s3211).
mod common;

use galion_consent::{piper_training_allowed, PIPER_TRAINING_ADDENDUM, SCOPE_PIPER_TRAINING};

#[test]
fn piper_training_needs_the_owner_addendum() {
    let t = common::Tree::new("piper-train");
    t.place_with("samantha", &common::wav_bytes(16_000, 1, 2, 10.0), |f| {
        for (k, v) in f.iter_mut() {
            if k == "instrument" {
                *v = "Public-domain dedication (LibriVox) plus CC0 1.0".into();
            }
        }
    });
    let env = t.env();
    // No addendum yet.
    let e = piper_training_allowed("samantha", &env).unwrap_err();
    assert_eq!(e.reason, "addendum");
    // Addendum present: allowed for this CC0 persona only.
    let body = format!(
        "persona: samantha\nscope: {SCOPE_PIPER_TRAINING}\ndecision: t662s3211\nperformer: Amy Koenig\n"
    );
    common::write(&t.folder("samantha").join(PIPER_TRAINING_ADDENDUM), &body);
    assert!(piper_training_allowed("samantha", &env).is_ok());
    // Wrong decision card: refused.
    common::write(
        &t.folder("samantha").join(PIPER_TRAINING_ADDENDUM),
        "persona: samantha\nscope: piper-voice-training\ndecision: other\nperformer: Amy Koenig\n",
    );
    assert_eq!(piper_training_allowed("samantha", &env).unwrap_err().reason, "addendum");
    // Celebrity still blocked before anything else.
    assert_eq!(piper_training_allowed("armstrong", &env).unwrap_err().reason, "celebrity");
}
