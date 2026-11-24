// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Galion Labs
//! One deterministic plan. Frame counts, then the control values.

use crate::glottis::open_quotient_from_rd;
use crate::{
    EmotionMoment, Error, Frame, Inside, Phase, Span, SpanKind, Track, VoiceBaseline, FRAME_MS,
    FRAME_S, MAX_FRAMES, QUIET_PS_MAX, QUIET_PS_MIN, SPEECH_OQ_MAX, SPEECH_OQ_MIN, SPEECH_PS_MAX,
    SPEECH_PS_MIN,
};

/// Quiet breathing, split into inspiration and expiration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct QuietCycle {
    pub period_frames: u32,
    pub inhale_frames: u32,
    pub exhale_frames: u32,
}

#[derive(Clone, Copy)]
struct Drive {
    baseline: VoiceBaseline,
    inside: Inside,
    moment: EmotionMoment,
}

struct Chunk {
    pause_frames: u32,
    talk_frames: u32,
    nasal: f32,
    mouth: f32,
}

#[derive(Clone, Copy)]
struct Sample {
    phase: Phase,
    speaking: bool,
    pause: bool,
    ps: f32,
    flow: f32,
    oq: f32,
    breathiness: f32,
    nasal: f32,
    mouth: f32,
}

impl Sample {
    fn frame(self) -> Frame {
        Frame {
            t_ms: 0,
            phase: self.phase,
            speaking: self.speaking,
            pause: self.pause,
            subglottal_cm_h2o: self.ps,
            airflow_ml_s: self.flow,
            open_quotient: self.oq,
            breathiness: self.breathiness,
            nasal_coupling: self.nasal,
            mouth_openness: self.mouth,
        }
    }
}

/// Quiet breathing for `frames` ticks. Cycles tile. A partial cycle at the end
/// keeps the same inspiration and expiration lengths.
#[must_use = "the plan is the only result"]
pub fn plan_quiet(
    baseline: VoiceBaseline,
    inside: Inside,
    moment: EmotionMoment,
    frames: u32,
) -> Result<Track, Error> {
    let cycle = quiet_cycle(baseline, inside, moment)?;
    if frames > MAX_FRAMES {
        return Err(Error::TooLong);
    }
    let drive = Drive {
        baseline,
        inside,
        moment,
    };
    let depth = quiet_depth(drive);
    let nasal = nasal_leak(baseline.nasal_rest, inside.health);
    let inhale_ps = quiet_inhale_ps(drive);
    let exhale_ps = quiet_exhale_ps(drive);
    let inhale_flow = clamp_flow(
        -(depth * 500.0) / frames_s(cycle.inhale_frames),
        -600.0,
        -80.0,
    );
    let exhale_flow = clamp_flow(
        depth * 500.0 * 0.90 / frames_s(cycle.exhale_frames),
        40.0,
        400.0,
    );
    let mut out = Vec::with_capacity(frames as usize);
    for i in 0..frames {
        let inhale = i % cycle.period_frames < cycle.inhale_frames;
        out.push(
            Sample {
                phase: if inhale { Phase::Inhale } else { Phase::Exhale },
                speaking: false,
                pause: false,
                ps: if inhale { inhale_ps } else { exhale_ps },
                flow: if inhale { inhale_flow } else { exhale_flow },
                oq: if inhale { 1.0 } else { 0.95 },
                breathiness: 1.0,
                nasal,
                mouth: if inhale {
                    mouth_for_air(depth)
                } else {
                    baseline.mouth_rest
                },
            }
            .frame(),
        );
        out[i as usize].t_ms = i * FRAME_MS;
    }
    Ok(Track { frames: out })
}

/// The quiet cycle those frames repeat. Eupnea stays at 10..=20 breaths/min.
pub fn quiet_cycle(
    baseline: VoiceBaseline,
    inside: Inside,
    moment: EmotionMoment,
) -> Result<QuietCycle, Error> {
    check_baseline(baseline)?;
    check_inside(inside)?;
    check_moment(moment)?;
    let speed =
        1.0 + moment.arousal.max(0.0) * 0.30 + moment.fear * 0.20 + (1.0 - inside.health) * 0.10;
    let period = (baseline.quiet_period_frames as f32 / speed)
        .round()
        .clamp(38.0, 75.0) as u32;
    let inhale = ((period as f32) / (1.0 + baseline.ie_ratio))
        .round()
        .clamp(8.0, (period - 12) as f32) as u32;
    let exhale = period - inhale;
    Ok(QuietCycle {
        period_frames: period,
        inhale_frames: inhale,
        exhale_frames: exhale,
    })
}

/// Speech on the exhale. A caller pause is an inhale, or an inhale plus a hold,
/// and the next talk starts on the following exhale.
#[must_use = "the plan is the only result"]
pub fn plan_speech(
    baseline: VoiceBaseline,
    inside: Inside,
    moment: EmotionMoment,
    spans: &[Span],
) -> Result<Track, Error> {
    check_baseline(baseline)?;
    check_inside(inside)?;
    check_moment(moment)?;
    for span in spans {
        check_span(*span)?;
    }
    let drive = Drive {
        baseline,
        inside,
        moment,
    };
    let chunks = expand(spans, &drive)?;
    render(&drive, &chunks)
}

fn expand(spans: &[Span], drive: &Drive) -> Result<Vec<Chunk>, Error> {
    let limit = group_limit(drive);
    let mut chunks = Vec::new();
    let mut pending = 0u32;
    for span in spans {
        match span.kind {
            SpanKind::Pause => pending = add(pending, span.frames)?,
            SpanKind::Talk if span.frames == 0 => {}
            SpanKind::Talk => {
                let mut left = span.frames;
                let mut first = true;
                while left > 0 {
                    let n = left.min(limit);
                    chunks.push(Chunk {
                        pause_frames: if first { pending } else { 0 },
                        talk_frames: n,
                        nasal: span.nasal,
                        mouth: span.mouth,
                    });
                    first = false;
                    pending = 0;
                    left -= n;
                }
            }
        }
    }
    if pending > 0 {
        chunks.push(Chunk {
            pause_frames: pending,
            talk_frames: 0,
            nasal: 0.0,
            mouth: drive.baseline.mouth_rest,
        });
    }
    Ok(chunks)
}

fn render(drive: &Drive, chunks: &[Chunk]) -> Result<Track, Error> {
    let rd = speech_rd(drive);
    let oq = speech_oq(rd)?;
    let breathiness = speech_breathiness(rd);
    let mut out = Vec::new();
    for chunk in chunks {
        push_boundary(&mut out, drive, chunk)?;
        push_talk(&mut out, drive, chunk, oq, breathiness)?;
    }
    for (i, frame) in out.iter_mut().enumerate() {
        frame.t_ms = i as u32 * FRAME_MS;
    }
    Ok(Track { frames: out })
}

fn push_boundary(out: &mut Vec<Frame>, drive: &Drive, chunk: &Chunk) -> Result<(), Error> {
    let insp = if chunk.talk_frames == 0 {
        0
    } else {
        insp_frames(chunk.talk_frames, drive)
    };
    let boundary = if chunk.talk_frames == 0 {
        chunk.pause_frames
    } else {
        chunk
            .pause_frames
            .max(insp + hold_frames(drive.moment.disgust))
    };
    let depth = if chunk.talk_frames == 0 {
        0.0
    } else {
        speech_depth(chunk.talk_frames, drive)
    };
    let mark = chunk.pause_frames > 0;
    let leak = nasal_leak(drive.baseline.nasal_rest, drive.inside.health);
    push_n(
        out,
        insp,
        Sample {
            phase: Phase::Inhale,
            speaking: false,
            pause: mark,
            ps: speech_inhale_ps(depth, drive),
            flow: inhale_flow(depth, insp),
            oq: 1.0,
            breathiness: 1.0,
            nasal: leak,
            mouth: mouth_for_air(depth),
        },
    )?;
    push_n(
        out,
        boundary - insp,
        Sample {
            phase: Phase::Hold,
            speaking: false,
            pause: mark,
            ps: 0.12,
            flow: 0.0,
            oq: 0.92,
            breathiness: 1.0,
            nasal: leak,
            mouth: drive.baseline.mouth_rest,
        },
    )
}

fn push_talk(
    out: &mut Vec<Frame>,
    drive: &Drive,
    chunk: &Chunk,
    oq: f32,
    breathiness: f32,
) -> Result<(), Error> {
    if chunk.talk_frames == 0 {
        return Ok(());
    }
    let depth = speech_depth(chunk.talk_frames, drive);
    let nasal = nasal_target(drive.baseline.nasal_rest, chunk.nasal, drive.inside.health);
    let mouth = (chunk.mouth * (1.0 - 0.20 * drive.inside.fatigue)).clamp(0.10, 0.90);
    let last = chunk.talk_frames - 1;
    for i in 0..chunk.talk_frames {
        let frac = if last == 0 {
            0.0
        } else {
            i as f32 / last as f32
        };
        let ps = speech_ps(drive, frac);
        push_one(
            out,
            Sample {
                phase: Phase::Exhale,
                speaking: true,
                pause: false,
                ps,
                flow: speech_flow(
                    depth,
                    chunk.talk_frames,
                    ps,
                    drive.baseline.speech_ps_cm_h2o,
                ),
                oq,
                breathiness,
                nasal,
                mouth,
            },
        )?;
    }
    Ok(())
}

fn push_n(out: &mut Vec<Frame>, n: u32, sample: Sample) -> Result<(), Error> {
    for _ in 0..n {
        push_one(out, sample)?;
    }
    Ok(())
}

fn push_one(out: &mut Vec<Frame>, sample: Sample) -> Result<(), Error> {
    if out.len() as u32 >= MAX_FRAMES {
        return Err(Error::TooLong);
    }
    out.push(sample.frame());
    Ok(())
}

fn speech_ps(drive: &Drive, frac: f32) -> f32 {
    let ps = drive.baseline.speech_ps_cm_h2o
        * (1.0 + 0.35 * drive.inside.energy)
        * (1.0 - 0.35 * drive.inside.fatigue)
        * (0.75 + 0.25 * drive.inside.health)
        * (1.0 + drive.moment.arousal.max(0.0) * 0.15)
        * (1.0 - 0.10 * frac);
    ps.clamp(SPEECH_PS_MIN, SPEECH_PS_MAX)
}

fn speech_rd(drive: &Drive) -> f32 {
    let rd = drive.baseline.modal_rd + drive.inside.fatigue * 0.55 - drive.inside.energy * 0.15
        + (1.0 - drive.inside.health) * 0.45;
    rd.clamp(0.5, 2.7)
}

fn speech_oq(rd: f32) -> Result<f32, Error> {
    let oq = open_quotient_from_rd(f64::from(rd))? as f32;
    Ok(oq.clamp(SPEECH_OQ_MIN, SPEECH_OQ_MAX))
}

fn speech_breathiness(rd: f32) -> f32 {
    let t = ((rd - 0.5) / 2.2).clamp(0.0, 1.0);
    0.05 + 0.65 * t
}

fn speech_inhale_ps(depth: f32, drive: &Drive) -> f32 {
    let mag = (0.50 + 0.40 * depth)
        * (1.0 + 0.10 * drive.inside.energy)
        * (1.0 - 0.25 * drive.inside.fatigue);
    -mag.clamp(0.30, 2.0)
}

fn quiet_exhale_ps(drive: Drive) -> f32 {
    let ps = 0.80
        * (1.0 + 0.20 * drive.inside.energy)
        * (1.0 - 0.20 * drive.inside.fatigue)
        * (0.85 + 0.15 * drive.inside.health);
    ps.clamp(QUIET_PS_MIN, QUIET_PS_MAX)
}

fn quiet_inhale_ps(drive: Drive) -> f32 {
    let mag = 0.45
        * (1.0 + 0.15 * drive.inside.energy)
        * (1.0 - 0.20 * drive.inside.fatigue)
        * (0.85 + 0.15 * drive.inside.health);
    -mag.clamp(QUIET_PS_MIN, QUIET_PS_MAX)
}

fn quiet_depth(drive: Drive) -> f32 {
    let depth = 0.80
        * (1.0 - 0.25 * drive.inside.fatigue)
        * (0.75 + 0.25 * drive.inside.health)
        * (1.0 - 0.15 * drive.moment.amusement);
    depth.clamp(0.35, 0.95)
}

fn speech_depth(talk_frames: u32, drive: &Drive) -> f32 {
    let speech_s = frames_s(talk_frames);
    let depth = (0.42 + 0.05 * speech_s)
        * (1.0 - 0.20 * drive.moment.amusement)
        * (1.0 - 0.30 * drive.inside.fatigue)
        * (0.70 + 0.30 * drive.inside.health);
    depth.clamp(0.20, 0.90)
}

fn speech_flow(depth: f32, talk_frames: u32, ps: f32, anchor: f32) -> f32 {
    let mean = depth * 500.0 * 0.70 / frames_s(talk_frames);
    let scaled = mean * (0.90 + 0.10 * (ps / anchor).max(0.0));
    scaled.clamp(80.0, 350.0)
}

fn inhale_flow(depth: f32, insp: u32) -> f32 {
    if insp == 0 {
        return 0.0;
    }
    clamp_flow(-(depth * 500.0) / frames_s(insp), -900.0, -80.0)
}

fn nasal_leak(rest: f32, health: f32) -> f32 {
    nasal_target(rest, 0.0, health)
}

fn nasal_target(rest: f32, target: f32, health: f32) -> f32 {
    let mixed = rest + (1.0 - rest) * target;
    (mixed * (0.40 + 0.60 * health)).clamp(0.0, 1.0)
}

fn mouth_for_air(depth: f32) -> f32 {
    (0.28 + 0.22 * depth).clamp(0.20, 0.55)
}

fn group_limit(drive: &Drive) -> u32 {
    let mut n = 50.0_f32;
    n *= 1.0 - 0.35 * drive.moment.fear;
    if drive.moment.arousal > 0.0 {
        n *= 1.0 - 0.25 * drive.moment.arousal;
    }
    n *= 1.0 - 0.20 * drive.inside.fatigue;
    n.round().clamp(20.0, 60.0) as u32
}

fn insp_frames(talk_frames: u32, drive: &Drive) -> u32 {
    let mut n = 4.0 + 0.10 * talk_frames as f32;
    n *= 1.0 - 0.25 * drive.moment.amusement;
    n *= 1.0 - 0.15 * drive.inside.fatigue;
    n.round().clamp(4.0, 12.0) as u32
}

fn hold_frames(disgust: f32) -> u32 {
    if disgust <= 0.0 {
        return 0;
    }
    (disgust * 6.0).round().clamp(1.0, 8.0) as u32
}

fn frames_s(frames: u32) -> f32 {
    (frames.max(1) as f32) * FRAME_S
}

fn clamp_flow(flow: f32, lo: f32, hi: f32) -> f32 {
    flow.clamp(lo, hi)
}

fn add(acc: u32, n: u32) -> Result<u32, Error> {
    acc.checked_add(n).ok_or(Error::TooLong)
}

fn check_baseline(b: VoiceBaseline) -> Result<(), Error> {
    if !(38..=75).contains(&b.quiet_period_frames) {
        return Err(Error::OutOfRange("quiet_period_frames"));
    }
    band("ie_ratio", b.ie_ratio, 1.5, 3.0)?;
    band("speech_ps_cm_h2o", b.speech_ps_cm_h2o, 5.0, 10.0)?;
    band("modal_rd", b.modal_rd, 0.8, 1.4)?;
    band("nasal_rest", b.nasal_rest, 0.0, 0.08)?;
    band("mouth_rest", b.mouth_rest, 0.05, 0.30)?;
    Ok(())
}

fn check_inside(inside: Inside) -> Result<(), Error> {
    unit("energy", inside.energy)?;
    unit("fatigue", inside.fatigue)?;
    unit("health", inside.health)?;
    Ok(())
}

fn check_moment(moment: EmotionMoment) -> Result<(), Error> {
    band("arousal", moment.arousal, -1.0, 1.0)?;
    unit("amusement", moment.amusement)?;
    unit("disgust", moment.disgust)?;
    unit("fear", moment.fear)?;
    Ok(())
}

fn check_span(span: Span) -> Result<(), Error> {
    if span.frames > MAX_FRAMES {
        return Err(Error::TooLong);
    }
    if span.kind == SpanKind::Talk {
        unit("mouth", span.mouth)?;
        unit("nasal", span.nasal)?;
    }
    Ok(())
}

fn unit(name: &'static str, v: f32) -> Result<(), Error> {
    band(name, v, 0.0, 1.0)
}

fn band(name: &'static str, v: f32, lo: f32, hi: f32) -> Result<(), Error> {
    if !v.is_finite() {
        return Err(Error::NonFinite(name));
    }
    if v < lo || v > hi {
        return Err(Error::OutOfRange(name));
    }
    Ok(())
}
