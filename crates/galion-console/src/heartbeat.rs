// Galion Voice — Perpetual Open MIT License
//! Yes / No widget state, bound to the 80 ms heartbeat.
//!
//! *A conversation is one heartbeat, not a pipeline.* A key press is never handled
//! as a free-floating UI callback: [`YesNo::press`] only *arms* an answer, and the
//! answer becomes a [`HeartbeatEvent`] when the next 80 ms tick lands
//! ([`YesNo::on_tick`], driven by [`crate::ConsoleState::advance`]).
//!
//! ## Keys (interactive TUI)
//! | key                 | answer |
//! |---------------------|--------|
//! | `y` / `Y` / `Left`  | YES    |
//! | `n` / `N` / `Right` | NO     |
//!
//! ## Coalescing rule (locked)
//! **Queue capacity 1, last press in a frame wins.** Any number of presses between
//! two ticks collapse into at most one event on the next tick, carrying the most
//! recent answer. Key auto-repeat / spam therefore can never emit more than one
//! event per 80 ms frame, and every event's `tick` is a real heartbeat tick.
//! `presses` on the event records how many raw presses were coalesced.
//!
//! ## Highlight
//! The committed answer is highlighted for [`HIGHLIGHT_TICKS`] ticks
//! (4 × 80 ms = 320 ms), then rests as a dim marker until the next answer.

use std::collections::VecDeque;

/// Heartbeat period. One tick = one 80 ms audio frame.
pub const FRAME_MS: u64 = 80;
/// How many ticks the last answer stays highlighted after it commits.
pub const HIGHLIGHT_TICKS: u64 = 4;
/// Bounded event log (safe to run for days over SSH).
pub const LOG_CAPACITY: usize = 32;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    Yes,
    No,
}

impl Answer {
    pub fn label(self) -> &'static str {
        match self {
            Answer::Yes => "YES",
            Answer::No => "NO",
        }
    }

    /// Map a key character to an answer (`y`/`n`, case-insensitive).
    pub fn from_char(c: char) -> Option<Self> {
        match c.to_ascii_lowercase() {
            'y' => Some(Answer::Yes),
            'n' => Some(Answer::No),
            _ => None,
        }
    }
}

/// One answer, committed on one heartbeat tick.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeartbeatEvent {
    /// Heartbeat tick (frame id) the event was committed on.
    pub tick: u64,
    pub kind: Answer,
    /// Frame-clock timestamp: `tick * FRAME_MS` (deterministic, no wall clock).
    pub at_ms: u64,
    /// Raw presses coalesced into this event (>= 1).
    pub presses: u32,
}

/// Yes/No widget state. Renderer-agnostic; fully testable without a terminal.
#[derive(Clone, Debug, Default)]
pub struct YesNo {
    pending: Option<Answer>,
    pending_presses: u32,
    last: Option<HeartbeatEvent>,
    log: VecDeque<HeartbeatEvent>,
}

impl YesNo {
    /// Arm an answer for the next tick. Last press in a frame wins.
    pub fn press(&mut self, a: Answer) {
        self.pending = Some(a);
        self.pending_presses = self.pending_presses.saturating_add(1);
    }

    /// Answer armed for the next tick, if any.
    pub fn pending(&self) -> Option<Answer> {
        self.pending
    }

    /// Called exactly once per heartbeat with the new tick id. Commits the armed
    /// answer (if any) as a [`HeartbeatEvent`] on that tick.
    pub fn on_tick(&mut self, tick: u64) -> Option<HeartbeatEvent> {
        let kind = self.pending.take()?;
        let ev = HeartbeatEvent { tick, kind, at_ms: tick.wrapping_mul(FRAME_MS), presses: self.pending_presses };
        self.pending_presses = 0;
        if self.log.len() == LOG_CAPACITY {
            self.log.pop_front();
        }
        self.log.push_back(ev);
        self.last = Some(ev);
        Some(ev)
    }

    /// Re-insert a persisted event (e.g. from `live::AnswerLog`) without a tick.
    /// It becomes `last()` and joins the bounded log; it is not highlighted
    /// unless its tick is within `HIGHLIGHT_TICKS` of the current tick.
    pub fn restore(&mut self, ev: HeartbeatEvent) {
        if self.log.len() == LOG_CAPACITY {
            self.log.pop_front();
        }
        self.log.push_back(ev);
        self.last = Some(ev);
    }

    pub fn last(&self) -> Option<HeartbeatEvent> {
        self.last
    }

    pub fn log(&self) -> &VecDeque<HeartbeatEvent> {
        &self.log
    }

    /// Answer to highlight at `now` (the current tick), for `HIGHLIGHT_TICKS` ticks.
    pub fn highlighted(&self, now: u64) -> Option<Answer> {
        self.last.filter(|e| now.wrapping_sub(e.tick) < HIGHLIGHT_TICKS).map(|e| e.kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ConsoleState;

    #[test]
    fn yes_press_commits_on_next_tick() {
        let mut s = ConsoleState::default();
        s.yes_no.press(Answer::Yes);
        assert!(s.yes_no.last().is_none(), "no event before the tick");
        assert_eq!(s.yes_no.pending(), Some(Answer::Yes));
        let ev = s.advance().expect("event on next tick");
        assert_eq!(ev, HeartbeatEvent { tick: 1, kind: Answer::Yes, at_ms: 80, presses: 1 });
        assert_eq!(s.yes_no.pending(), None);
        assert!(s.advance().is_none(), "one press = one event");
    }

    #[test]
    fn no_press_commits_on_next_tick() {
        let mut s = ConsoleState::default();
        for _ in 0..5 {
            s.advance();
        }
        s.yes_no.press(Answer::No);
        let ev = s.advance().expect("event on next tick");
        assert_eq!((ev.tick, ev.kind, ev.at_ms), (6, Answer::No, 480));
    }

    #[test]
    fn spam_coalesces_last_press_in_frame_wins() {
        let mut s = ConsoleState::default();
        for i in 0..10 {
            s.yes_no.press(if i % 2 == 0 { Answer::Yes } else { Answer::No });
        }
        s.yes_no.press(Answer::Yes);
        let ev = s.advance().unwrap();
        assert_eq!((ev.kind, ev.presses), (Answer::Yes, 11));
        // Next frame: a single NO is its own event on its own tick.
        s.yes_no.press(Answer::No);
        let ev2 = s.advance().unwrap();
        assert_eq!((ev2.tick, ev2.kind, ev2.presses), (ev.tick + 1, Answer::No, 1));
        // Exactly one event per frame that had presses; ticks strictly increasing.
        assert_eq!(s.yes_no.log().len(), 2);
        assert!(s.yes_no.log().iter().zip(s.yes_no.log().iter().skip(1)).all(|(a, b)| b.tick > a.tick));
        assert!(s.advance().is_none());
    }

    #[test]
    fn highlight_lasts_a_few_ticks() {
        let mut s = ConsoleState::default();
        s.yes_no.press(Answer::Yes);
        s.advance();
        for _ in 0..HIGHLIGHT_TICKS - 1 {
            assert_eq!(s.yes_no.highlighted(s.tick), Some(Answer::Yes));
            s.advance();
        }
        assert_eq!(s.yes_no.highlighted(s.tick), Some(Answer::Yes));
        s.advance();
        assert_eq!(s.yes_no.highlighted(s.tick), None);
        assert_eq!(s.yes_no.last().map(|e| e.kind), Some(Answer::Yes), "last answer kept");
    }

    #[test]
    fn log_is_bounded() {
        let mut y = YesNo::default();
        for t in 0..(LOG_CAPACITY as u64 * 2) {
            y.press(Answer::No);
            y.on_tick(t);
        }
        assert_eq!(y.log().len(), LOG_CAPACITY);
    }

    #[test]
    fn keys_map() {
        assert_eq!(Answer::from_char('Y'), Some(Answer::Yes));
        assert_eq!(Answer::from_char('n'), Some(Answer::No));
        assert_eq!(Answer::from_char('q'), None);
    }

    #[test]
    fn widget_renders_headless() {
        use ratatui::{backend::TestBackend, Terminal};
        let mut term = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let mut s = ConsoleState::default();
        s.yes_no.press(Answer::No);
        s.advance();
        term.draw(|f| crate::ui::draw(f, &mut s)).unwrap();
        let buf = term.backend().buffer();
        let last: String = (0..buf.area.width).map(|x| buf[(x, buf.area.height - 1)].symbol()).collect();
        assert!(last.contains("YES") && last.contains("NO"), "footer: {last:?}");
        assert!(last.contains("t1"), "shows commit tick: {last:?}");
    }
}
