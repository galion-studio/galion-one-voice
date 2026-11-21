// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs

//! Names the state tracker already holds. This layer does not discover a
//! person and does not write them to disk.

/// Which pronoun class a tracked person answers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PronounClass {
    He,
    She,
    They,
}

impl PronounClass {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::He => "he",
            Self::She => "she",
            Self::They => "they",
        }
    }
}

/// The state tracker, or any stand-in that can answer "who is him?".
pub trait NameSource {
    /// The name to write in place of this pronoun class. `None` leaves the
    /// pronoun in the query. More than one match and no focus is `None`.
    fn name_for(&self, class: PronounClass) -> Option<&str>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Person {
    name: String,
    class: PronounClass,
}

/// Ordered book. The last [`NameBook::note`] is the focus, which is the
/// person a bare "him" / "her" / "them" means when the class matches.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NameBook {
    people: Vec<Person>,
    focus: Option<usize>,
}

impl NameBook {
    pub const MAX: usize = 32;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.people.len()
    }

    pub fn is_empty(&self) -> bool {
        self.people.is_empty()
    }

    pub fn clear(&mut self) {
        self.people.clear();
        self.focus = None;
    }

    /// Remember `name` and make them the focus. Empty, over-long, and
    /// pronoun-shaped names are ignored. The book keeps at most [`Self::MAX`].
    pub fn note(&mut self, name: &str, class: PronounClass) {
        let name = name.trim();
        if !valid_name(name) {
            return;
        }
        if let Some(i) = self
            .people
            .iter()
            .position(|p| p.name.eq_ignore_ascii_case(name))
        {
            self.people[i].class = class;
            self.focus = Some(i);
            return;
        }
        if self.people.len() >= Self::MAX {
            self.people.remove(0);
            self.focus = match self.focus {
                Some(0) | None => None,
                Some(i) => Some(i - 1),
            };
        }
        self.people.push(Person {
            name: name.to_string(),
            class,
        });
        self.focus = Some(self.people.len() - 1);
    }

    /// Replace the book. The last item becomes the focus.
    pub fn replace<I, S>(&mut self, people: I)
    where
        I: IntoIterator<Item = (S, PronounClass)>,
        S: AsRef<str>,
    {
        self.clear();
        for (name, class) in people {
            self.note(name.as_ref(), class);
        }
    }

    pub fn focus_name(&self) -> Option<&str> {
        self.focus
            .and_then(|i| self.people.get(i))
            .map(|p| p.name.as_str())
    }

    pub fn people(&self) -> impl Iterator<Item = (&str, PronounClass)> + '_ {
        self.people.iter().map(|p| (p.name.as_str(), p.class))
    }
}

impl NameSource for NameBook {
    fn name_for(&self, class: PronounClass) -> Option<&str> {
        if let Some(i) = self.focus {
            if self.people.get(i).is_some_and(|p| p.class == class) {
                return Some(self.people[i].name.as_str());
            }
        }
        let mut found: Option<&str> = None;
        for person in &self.people {
            if person.class != class {
                continue;
            }
            if found.is_some() {
                return None;
            }
            found = Some(person.name.as_str());
        }
        found
    }
}

fn valid_name(name: &str) -> bool {
    if name.is_empty() || name.len() > 64 {
        return false;
    }
    if name.bytes().any(|b| b == b'\n' || b == b'\r' || b == b'\t') {
        return false;
    }
    !is_pronoun_name(name)
}

fn is_pronoun_name(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "he" | "him"
            | "his"
            | "she"
            | "her"
            | "hers"
            | "they"
            | "them"
            | "their"
            | "theirs"
            | "himself"
            | "herself"
            | "themselves"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_wins_and_a_tie_without_focus_does_not_guess() {
        let mut book = NameBook::new();
        book.note("Maciej", PronounClass::He);
        book.note("Ada", PronounClass::She);
        assert_eq!(book.name_for(PronounClass::He), Some("Maciej"));
        assert_eq!(book.name_for(PronounClass::She), Some("Ada"));
        book.note("Jan", PronounClass::He);
        assert_eq!(book.focus_name(), Some("Jan"));
        assert_eq!(book.name_for(PronounClass::He), Some("Jan"));
        book.note("Ada", PronounClass::She);
        assert_eq!(book.name_for(PronounClass::He), None);
        assert_eq!(book.name_for(PronounClass::She), Some("Ada"));
    }

    #[test]
    fn a_pronoun_is_not_stored_as_a_name() {
        let mut book = NameBook::new();
        book.note("him", PronounClass::He);
        book.note("  ", PronounClass::She);
        assert!(book.is_empty());
    }
}
