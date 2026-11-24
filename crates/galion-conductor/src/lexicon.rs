// Galion Voice — Perpetual Open MIT License. (c) 2026 Galion Labs.
//! Listed backchannels. A hit does not cancel the speaking turn.
//!
//! The list is the union of the domain 03 lexicon
//! (`uh-huh`, `mm`, `yeah`, `okay`, `right`, `da`, `aha`) and the short
//! tokens `call.rs` already treats as `only_bc`
//! (`mm`, `hm`, `yeah`, `right`, `quite`, `oh`, `uh`, `huh`, `okay`),
//! plus the cached backchannel spellings `mm-hm` and `mhm`.

const PHRASES: &[&str] = &[
    "uh-huh", "uh huh", "mm", "mmm", "mm-hm", "mm hm", "mhm", "hm", "hmm", "yeah", "yea", "yep",
    "okay", "ok", "right", "da", "aha", "oh", "uh", "huh", "quite",
];

/// Two words or fewer, every word on the list, or the whole phrase on the list.
pub fn is_listed_backchannel(text: &str) -> bool {
    let norm = normalize(text);
    if norm.is_empty() {
        return false;
    }
    let words: Vec<&str> = norm.split(' ').filter(|word| !word.is_empty()).collect();
    if words.is_empty() || words.len() > 2 {
        return false;
    }
    if PHRASES.contains(&norm.as_str()) {
        return true;
    }
    words.iter().all(|word| PHRASES.contains(word))
}

fn normalize(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' {
            out.push(ch.to_ascii_lowercase());
        } else if ch.is_whitespace() && !out.is_empty() && !out.ends_with(' ') {
            out.push(' ');
        }
    }
    if out.ends_with(' ') {
        out.pop();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listed_forms_pass_and_a_real_interruption_does_not() {
        for sample in [
            "yeah", "Yeah.", "Mm-hm.", "okay", "uh-huh", "right", "mhm", "oh yeah",
        ] {
            assert!(is_listed_backchannel(sample), "{sample}");
        }
        assert!(!is_listed_backchannel(""));
        assert!(!is_listed_backchannel("stop talking now"));
        assert!(!is_listed_backchannel("yeah stop"));
        assert!(!is_listed_backchannel("hello"));
    }
}
