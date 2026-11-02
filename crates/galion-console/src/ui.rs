// Galion Voice — Perpetual Open MIT License
//! ratatui renderer (terminal default). Pure function of `ConsoleState` -> frame.

use crate::heartbeat::Answer;
use crate::model::{ConsoleState, JobState};
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    symbols,
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Sparkline},
    Frame,
};

const GLYPHS: &[u8] = b"01<>=+*#:.ABCDEFGHJKLMNPQRSTUVWXYZ";

pub fn draw(f: &mut Frame, state: &mut ConsoleState) {
    let area = f.area();
    let head_h = if state.live.is_some() { 2 } else { 1 };
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(head_h), Constraint::Min(8), Constraint::Length(1)])
        .split(area);

    let head = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(head_h - 1)])
        .split(rows[0]);
    header(f, head[0], state);
    if let Some(live) = &state.live {
        f.render_widget(Paragraph::new(live.line()), head[1]);
    }

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(rows[1]);
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(cols[0]);

    bench_panel(f, left[0], state);
    jobs_panel(f, left[1], state);
    tensor_rain_panel(f, cols[1], state);

    let foot = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(0), Constraint::Length(YES_NO_WIDTH)])
        .split(rows[2]);
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(" q", Style::default().fg(Color::Yellow)),
        Span::raw(" quit  "),
        Span::styled("y/n", Style::default().fg(Color::Yellow)),
        Span::raw(" answer  "),
        Span::styled("terminal", Style::default().fg(Color::Green)),
        Span::raw(match &state.live {
            Some(l) => format!(
                " (default) | window: planned | web: planned | source: live {}",
                if l.source.is_empty() { "stub" } else { l.source }
            ),
            None => " (default) | window: planned | web: planned | source: synthetic".to_string(),
        }),
    ]));
    f.render_widget(footer, foot[0]);
    yes_no(f, foot[1], state);

    if !state.color {
        crate::live::strip_color(f.buffer_mut());
    }
}

const YES_NO_WIDTH: u16 = 22;

/// Minimal YES / NO control. Black console, no chrome:
/// - the answer committed on the last few heartbeat ticks is inverted (black on white),
/// - the previous answer rests in plain white, the other in dark gray,
/// - an answer armed for the next tick is underlined,
/// - `t<N>` is the heartbeat tick the last answer landed on.
fn yes_no(f: &mut Frame, area: Rect, state: &ConsoleState) {
    let w = &state.yes_no;
    let hot = w.highlighted(state.tick);
    let rest = w.last().map(|e| e.kind);
    let pending = w.pending();
    let cell = |a: Answer| {
        let mut st = if hot == Some(a) {
            Style::default().fg(Color::Black).bg(Color::White).add_modifier(Modifier::BOLD)
        } else if rest == Some(a) {
            Style::default().fg(Color::White)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        if pending == Some(a) {
            st = st.add_modifier(Modifier::UNDERLINED);
        }
        Span::styled(format!(" {} ", a.label()), st)
    };
    let mark = match w.last() {
        Some(e) => format!(" t{}", e.tick),
        None => " ·".to_string(),
    };
    let line = Line::from(vec![
        cell(Answer::Yes),
        Span::raw(" "),
        cell(Answer::No),
        Span::styled(mark, Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(Paragraph::new(line), area);
}

fn header(f: &mut Frame, area: Rect, state: &ConsoleState) {
    let t = Line::from(vec![
        Span::styled(" GALION VOICE ", Style::default().fg(Color::Black).bg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw(" console v0 — dual-band voice-to-voice · CPU-first · "),
        Span::styled(format!("tick {}", state.tick), Style::default().fg(Color::DarkGray)),
    ]);
    f.render_widget(Paragraph::new(t), area);
}

fn bench_panel(f: &mut Frame, area: Rect, state: &ConsoleState) {
    let block = Block::default().borders(Borders::ALL).title(" Bench history (RTF, lower=better) ");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(2), Constraint::Min(1)])
        .split(inner);

    let last = state.bench_history.back();
    let (rtf, lat) = last.map(|s| (s.rtf, s.latency_ms)).unwrap_or((0.0, 0.0));
    let ok = rtf > 0.0 && rtf < 1.0;
    let summary = vec![
        Line::from(vec![
            Span::raw("last RTF "),
            Span::styled(format!("{rtf:.3}"), Style::default().fg(if ok { Color::Green } else { Color::Red })),
            Span::raw(format!("  latency {lat:.1} ms  n={}", state.bench_history.len())),
        ]),
        Line::from(Span::styled("gate: RTF < 1.0 (real time on cheap VPS CPU)", Style::default().fg(Color::DarkGray))),
    ];
    f.render_widget(Paragraph::new(summary), parts[0]);

    // Sparkline wants u64; scale RTF x1000, show the most recent samples that fit.
    let w = parts[1].width as usize;
    let data: Vec<u64> = state
        .bench_history
        .iter()
        .rev()
        .take(w)
        .rev()
        .map(|s| (s.rtf * 1000.0) as u64)
        .collect();
    let spark = Sparkline::default()
        .data(&data)
        .max(1000)
        .bar_set(symbols::bar::NINE_LEVELS)
        .style(Style::default().fg(Color::Cyan));
    f.render_widget(spark, parts[1]);
}

fn jobs_panel(f: &mut Frame, area: Rect, state: &ConsoleState) {
    let block = Block::default().borders(Borders::ALL).title(" Jobs ");
    let inner = block.inner(area);
    f.render_widget(block, area);

    let items: Vec<ListItem> = state
        .jobs
        .iter()
        .map(|j| {
            let color = match j.state {
                JobState::Queued => Color::DarkGray,
                JobState::Running => Color::Yellow,
                JobState::Passed => Color::Green,
                JobState::Failed => Color::Red,
            };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{:<8}", j.state.label()), Style::default().fg(color)),
                Span::raw(format!("{:>4.0}%  {}", j.progress * 100.0, j.name)),
            ]))
        })
        .collect();

    // Overall progress gauge on the last line.
    let parts = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);
    f.render_widget(List::new(items), parts[0]);
    let total = if state.jobs.is_empty() {
        0.0
    } else {
        state.jobs.iter().map(|j| j.progress).sum::<f64>() / state.jobs.len() as f64
    };
    f.render_widget(
        Gauge::default().gauge_style(Style::default().fg(Color::Green)).ratio(total.clamp(0.0, 1.0)).label(format!("pipeline {:.0}%", total * 100.0)),
        parts[1],
    );
}

/// "Matrix rain" placeholder: each column is a falling trail whose brightness is
/// modulated by a sampled tensor activation. Later: real per-layer activations from galion-tensor.
fn tensor_rain_panel(f: &mut Frame, area: Rect, state: &mut ConsoleState) {
    let title = format!(
        " Tensor / matrix rain (placeholder) · {} ops · {:.2} GFLOP/s ",
        state.tensor.ops, state.tensor.gflops
    );
    let block = Block::default().borders(Borders::ALL).title(title);
    let inner = block.inner(area);
    f.render_widget(block, area);
    if inner.width == 0 || inner.height == 0 {
        return;
    }

    let w = inner.width as usize;
    let h = inner.height;
    if state.rain_heads.len() != w {
        state.rain_heads = (0..w).map(|i| ((i * 7919) % (h as usize * 2 + 1)) as u16).collect();
    }

    let acts = &state.tensor.activations;
    let trail: u16 = (h / 2).max(3);
    let cycle = h + trail;
    let mut lines = Vec::with_capacity(h as usize);
    for row in 0..h {
        let mut spans = Vec::with_capacity(w);
        for (col, head) in state.rain_heads.iter().enumerate() {
            let head = head % cycle;
            let act = if acts.is_empty() { 0.5 } else { acts[col % acts.len()] };
            let dist = head as i32 - row as i32;
            let glyph = GLYPHS[(col * 31 + row as usize * 17 + state.tick as usize / 2) % GLYPHS.len()] as char;
            let span = if dist == 0 {
                Span::styled(glyph.to_string(), Style::default().fg(Color::White).add_modifier(Modifier::BOLD))
            } else if dist > 0 && (dist as u16) < trail {
                let fade = 1.0 - dist as f32 / trail as f32;
                let g = (60.0 + 195.0 * fade * (0.4 + 0.6 * act)) as u8;
                Span::styled(glyph.to_string(), Style::default().fg(Color::Rgb(0, g, (g as f32 * 0.35) as u8)))
            } else {
                Span::raw(" ")
            };
            spans.push(span);
        }
        lines.push(Line::from(spans));
    }
    f.render_widget(Paragraph::new(lines), inner);
}
