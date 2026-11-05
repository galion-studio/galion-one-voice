// Galion Voice — Perpetual Open MIT License. (c) 2026 Maciej Grajczyk.
//! In-house persona reply floor. No model, no weights, no network.
//!
//! Owner ruling 2026-11-04: the reply path is in-house only (no Qwen, no
//! Groq, no cloud LLM). Until the S0 student `.gq` exists on galion-tensor,
//! content turns on the persona floor can be answered by this rule engine
//! when the host opts in with `GALION_LLM=canned` (or `inhouse`).
//!
//! It does what the Hermes system prompt asks of a model, by rule: say the
//! fact the caller named back in second person, then one short persona line
//! that stays on that fact. It never speaks an empathy script; every reply
//! still goes through [`crate::hermes::contains_forbidden`].
//!
//! This is not a language model. The hard harness reports it as the
//! instruction floor (`real-llm: stub`), never as Hermes or a student.

use crate::hermes::{contains_forbidden, REFUSED};

/// Label on `/v2/say` `"llm"` when this engine wrote the reply.
pub const LLM_NAME: &str = "galion-reflect";

/// Longest reflected fact, in characters, before the persona line.
pub const MAX_FACT_CHARS: usize = 180;

/// `GALION_LLM=canned|inhouse|reflect` selects this engine for the persona floor.
pub fn selected_from_env() -> bool {
    matches!(
        std::env::var("GALION_LLM")
            .ok()
            .map(|s| s.trim().to_ascii_lowercase())
            .as_deref(),
        Some("canned") | Some("inhouse") | Some("in-house") | Some("reflect")
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Matter {
    Loss,
    Joy,
    SelfDoubt,
    Other,
}

fn matter_of(low: &str) -> Matter {
    let has = |w: &[&str]| w.iter().any(|k| low.contains(k));
    if has(&[
        "died",
        "passed away",
        "funeral",
        "lost my",
        "is dead",
        "was killed",
    ]) {
        Matter::Loss
    } else if has(&[
        "likes me",
        "like me",
        "hates me",
        "hate me",
        "worthless",
        "nobody",
        "no one",
        "not good enough",
    ]) {
        Matter::SelfDoubt
    } else if has(&[
        "got the job",
        "got in",
        "landed",
        "i won",
        "we won",
        "passed my",
        "can't stop smiling",
        "cant stop smiling",
        "engaged",
        "promoted",
    ]) {
        Matter::Joy
    } else {
        Matter::Other
    }
}

/// `her` / `him` / `them` for the person a loss sentence names.
fn pronoun_for(low: &str) -> &'static str {
    const HER: &[&str] = &[
        "mother",
        "mom",
        "mum",
        "mama",
        "sister",
        "wife",
        "daughter",
        "grandmother",
        "grandma",
        "aunt",
        "girlfriend",
    ];
    const HIM: &[&str] = &[
        "father",
        "dad",
        "papa",
        "brother",
        "husband",
        "son",
        "grandfather",
        "grandpa",
        "uncle",
        "boyfriend",
    ];
    let words: Vec<&str> = low
        .split(|c: char| !c.is_ascii_alphabetic())
        .filter(|w| !w.is_empty())
        .collect();
    if words.iter().any(|w| HER.contains(w)) {
        "her"
    } else if words.iter().any(|w| HIM.contains(w)) {
        "him"
    } else {
        "them"
    }
}

/// Swap first and second person, word by word. Punctuation stays put.
pub fn second_person(text: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut prev = String::new();
    for raw in text.split_whitespace() {
        let start = raw.find(|c: char| c.is_alphanumeric()).unwrap_or(raw.len());
        let end = raw
            .rfind(|c: char| c.is_alphanumeric())
            .map(|i| i + raw[i..].chars().next().map(char::len_utf8).unwrap_or(1))
            .unwrap_or(start);
        let (lead, core, tail) = if start < end {
            (&raw[..start], &raw[start..end], &raw[end..])
        } else {
            (raw, "", "")
        };
        let low = core.to_ascii_lowercase().replace('\u{2019}', "'");
        let subj_slot = out.is_empty()
            || matches!(
                prev.as_str(),
                "and" | "but" | "because" | "so" | "that" | "if" | "when" | "think" | "said"
            );
        let swapped: Option<&str> = match low.as_str() {
            "i" => Some("you"),
            "i'm" => Some("you're"),
            "i've" => Some("you've"),
            "i'll" => Some("you'll"),
            "i'd" => Some("you'd"),
            "me" => Some("you"),
            "my" => Some("your"),
            "mine" => Some("yours"),
            "myself" => Some("yourself"),
            "am" if prev == "i" => Some("are"),
            "was" if prev == "i" => Some("were"),
            "you" if subj_slot => Some("I"),
            "you" => Some("me"),
            "your" => Some("my"),
            "yours" => Some("mine"),
            "yourself" => Some("myself"),
            "you're" => Some("I'm"),
            "you've" => Some("I've"),
            "you'll" => Some("I'll"),
            "are" if prev == "you" => Some("am"),
            "were" if prev == "you" => Some("was"),
            _ => None,
        };
        let sentence_start = out
            .last()
            .is_none_or(|w: &String| w.ends_with(['.', '!', '?']));
        let word = match swapped {
            Some(w) if sentence_start => capitalize(w),
            Some(w) => w.to_string(),
            None => core.to_string(),
        };
        prev = low;
        out.push(format!("{lead}{word}{tail}"));
    }
    let mut s = out.join(" ");
    if let Some(first) = s.chars().next() {
        let up: String = first.to_uppercase().collect();
        s.replace_range(..first.len_utf8(), &up);
    }
    s
}

fn capitalize(w: &str) -> String {
    let mut c = w.chars();
    match c.next() {
        Some(f) => f.to_uppercase().chain(c).collect(),
        None => String::new(),
    }
}

/// First one or two sentences of what the caller said, clipped on a word.
fn fact_span(user: &str) -> String {
    let t = user.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut cut = t.len();
    let mut seen = 0;
    for (i, c) in t.char_indices() {
        if matches!(c, '.' | '!' | '?') {
            seen += 1;
            if seen == 2 {
                cut = i + 1;
                break;
            }
        }
    }
    let mut s: String = t[..cut].to_string();
    if s.chars().count() > MAX_FACT_CHARS {
        let clipped: String = s.chars().take(MAX_FACT_CHARS).collect();
        s = match clipped.rfind(' ') {
            Some(i) if i > 20 => clipped[..i].to_string(),
            _ => clipped,
        };
    }
    let s = s
        .trim_end_matches(|c: char| c == ',' || c == ';' || c == ':' || c.is_whitespace())
        .to_string();
    if s.ends_with(['.', '!', '?']) {
        s
    } else {
        format!("{s}.")
    }
}

fn persona_line(persona: &str, matter: Matter, low: &str) -> String {
    let jarvis = persona == "jarvis";
    match (jarvis, matter) {
        (false, Matter::Loss) => format!("Do you want to tell me about {}?", pronoun_for(low)),
        (true, Matter::Loss) => "Understood. Say what needs handling.".into(),
        (false, Matter::Joy) => "How did you find out?".into(),
        (true, Matter::Joy) => "Noted. Well earned.".into(),
        (false, Matter::SelfDoubt) => "Who were you picturing when you said that?".into(),
        (true, Matter::SelfDoubt) => {
            "That is a reading, not a fact. Name whose opinion you mean.".into()
        }
        (false, Matter::Other) => "Go on.".into(),
        (true, Matter::Other) => "Go on.".into(),
    }
}

/// One in-house reply: the caller's fact in second person, then the persona line.
/// The result is guarded: a forbidden phrase is [`REFUSED`], never spoken.
pub fn reply(persona: &str, user: &str) -> String {
    let user = user.trim();
    if user.is_empty() {
        return String::new();
    }
    let low = user.to_ascii_lowercase();
    let fact = second_person(&fact_span(user));
    let line = persona_line(persona, matter_of(&low), &low);
    let out = format!("{fact} {line}");
    if contains_forbidden(&out) {
        return REFUSED.to_string();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grief_names_the_fact_in_both_registers() {
        let s = reply("samantha", "My mother died this morning.");
        assert_eq!(
            s,
            "Your mother died this morning. Do you want to tell me about her?"
        );
        let j = reply("jarvis", "My mother died this morning.");
        assert!(j.starts_with("Your mother died this morning."), "{j}");
        assert!(j.len() > s.len() - 40);
        assert!(!contains_forbidden(&s) && !contains_forbidden(&j));
    }

    #[test]
    fn joy_and_self_doubt_stay_on_the_matter() {
        let joy = reply("samantha", "I got the job. I can't stop smiling.");
        assert_eq!(
            joy,
            "You got the job. You can't stop smiling. How did you find out?"
        );
        let vul = reply("samantha", "I don't think anyone actually likes me.");
        assert_eq!(
            vul,
            "You don't think anyone actually likes you. Who were you picturing when you said that?"
        );
        let jv = reply("jarvis", "I don't think anyone actually likes me.");
        assert!(jv.ends_with("Name whose opinion you mean."), "{jv}");
        // Jarvis states; it does not question.
        for t in [
            "My mother died this morning.",
            "I got the job.",
            "I don't think anyone actually likes me.",
        ] {
            assert!(!reply("jarvis", t).contains('?'), "{t}");
        }
    }

    #[test]
    fn person_swap_both_directions() {
        assert_eq!(
            second_person("I was late and you were right."),
            "You were late and I was right."
        );
        assert_eq!(
            second_person("I'm tired of my job."),
            "You're tired of your job."
        );
        assert_eq!(second_person("I told you."), "You told me.");
        assert_eq!(
            second_person("my brother\u{2019}s car"),
            "Your brother\u{2019}s car"
        );
    }

    #[test]
    fn a_caller_sorry_is_reflected_not_scripted() {
        // "I'm sorry" from the caller becomes "You're sorry", which is not the script.
        let r = reply("samantha", "I'm sorry I was late.");
        assert_eq!(r, "You're sorry you were late. Go on.");
        assert!(!contains_forbidden(&r));
    }

    #[test]
    fn long_input_is_clipped_on_a_word() {
        let long = "My ".to_string() + &"very ".repeat(80) + "long day.";
        let r = reply("jarvis", &long);
        assert!(r.chars().count() < MAX_FACT_CHARS + 40, "{r}");
        assert!(r.ends_with("Go on."));
    }

    #[test]
    fn empty_is_empty() {
        assert_eq!(reply("samantha", "   "), "");
    }
}
