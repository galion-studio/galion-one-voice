//! YIN and pYIN (probabilistic YIN with Viterbi decoding) F0 tracking.
//! Difference function computed via FFT cross-correlation (O(N log N) per frame).
use realfft::RealFftPlanner;

pub struct PitchTrack {
    /// pYIN F0 per frame, 0 = unvoiced
    pub f0: Vec<f64>,
    /// pYIN voiced probability per frame
    pub vprob: Vec<f64>,
    /// classic YIN F0 (absolute threshold 0.15), 0 = unvoiced
    pub f0_yin: Vec<f64>,
}

const NTHR: usize = 100;

fn beta_prior(mean: f64) -> Vec<f64> {
    // Beta(2,b) over thresholds 0.01..1.00 with b = 2/mean - 2 (pYIN: means 0.1 / 0.15 / 0.2)
    let b = 2.0 / mean - 2.0;
    let mut p: Vec<f64> = (1..=NTHR).map(|i| { let s = i as f64 / NTHR as f64; s * (1.0 - s).powf(b - 1.0) }).collect();
    let z: f64 = p.iter().sum();
    for v in &mut p { *v /= z; }
    p
}

/// `x` at `fs`; frames centred at `centers` (sample indices). `gate[i]` = frame loud enough to be speech.
pub fn track(x: &[f32], fs: u32, centers: &[usize], gate: &[bool], fmin: f64, fmax: f64, beta_mean: f64) -> PitchTrack {
    let fsf = fs as f64;
    let tmin = (fsf / fmax).floor().max(2.0) as usize;
    let tmax = (fsf / fmin).ceil() as usize;
    let w = ((0.025 * fsf) as usize).max(tmax + 2); // integration window
    let seg = w + tmax + 2;
    let mut n = 1;
    while n < seg + w { n <<= 1; }
    let mut planner = RealFftPlanner::<f64>::new();
    let fwd = planner.plan_fft_forward(n);
    let inv = planner.plan_fft_inverse(n);
    let prior = beta_prior(beta_mean);
    let mut a = fwd.make_input_vec();
    let mut b = fwd.make_input_vec();
    let mut sa = fwd.make_output_vec();
    let mut sb = fwd.make_output_vec();
    let mut r = inv.make_output_vec();
    let mut scratch = fwd.make_scratch_vec();
    let mut iscratch = inv.make_scratch_vec();
    let mut segbuf = vec![0f64; seg];
    let mut cum = vec![0f64; seg + 1];
    let mut d = vec![0f64; tmax + 2];
    let mut dn = vec![0f64; tmax + 2];

    let nf = centers.len();
    let mut cands: Vec<Vec<(f64, f64)>> = Vec::with_capacity(nf); // (f0, prob)
    let mut dstore: Vec<Vec<f32>> = Vec::with_capacity(nf); // CMNDF per frame for octave correction
    let mut f0_yin = vec![0f64; nf];
    for (fi, &c) in centers.iter().enumerate() {
        let start = c as isize - (seg / 2) as isize;
        let mut mean = 0.0;
        for k in 0..seg {
            let idx = start + k as isize;
            segbuf[k] = if idx >= 0 && (idx as usize) < x.len() { x[idx as usize] as f64 } else { 0.0 };
            mean += segbuf[k];
        }
        mean /= seg as f64;
        for v in segbuf.iter_mut() { *v -= mean; }
        cum[0] = 0.0;
        for k in 0..seg { cum[k + 1] = cum[k] + segbuf[k] * segbuf[k]; }
        if cum[seg] < 1e-10 { cands.push(vec![]); dstore.push(vec![]); continue; }
        for k in 0..n { a[k] = if k < seg { segbuf[k] } else { 0.0 }; b[k] = if k < w { segbuf[k] } else { 0.0 }; }
        fwd.process_with_scratch(&mut a, &mut sa, &mut scratch).unwrap();
        fwd.process_with_scratch(&mut b, &mut sb, &mut scratch).unwrap();
        for k in 0..sa.len() { sa[k] = sa[k] * sb[k].conj(); }
        // imaginary parts of DC / Nyquist must be zero for realfft
        sa[0].im = 0.0; let l = sa.len() - 1; sa[l].im = 0.0;
        inv.process_with_scratch(&mut sa, &mut r, &mut iscratch).unwrap();
        let e0 = cum[w];
        for t in 0..=tmax {
            let rt = r[t] / n as f64; // cross-corr sum_{j<w} x_j x_{j+t}
            let et = cum[t + w] - cum[t];
            d[t] = (e0 + et - 2.0 * rt).max(0.0);
        }
        dn[0] = 1.0;
        let mut run = 0.0;
        for t in 1..=tmax { run += d[t]; dn[t] = if run > 0.0 { d[t] * t as f64 / run } else { 1.0 }; }
        // local minima candidates
        let interp = |t: usize| -> (f64, f64) {
            if t <= 1 || t >= tmax { return (t as f64, dn[t]); }
            let (y0, y1, y2) = (dn[t - 1], dn[t], dn[t + 1]);
            let den = y0 - 2.0 * y1 + y2;
            if den.abs() < 1e-12 { return (t as f64, y1); }
            let dt = 0.5 * (y0 - y2) / den;
            let dt = dt.clamp(-1.0, 1.0);
            (t as f64 + dt, y1 - 0.25 * (y0 - y2) * dt)
        };
        // YIN absolute threshold
        let mut yt = 0usize;
        let mut t = tmin;
        while t < tmax {
            if dn[t] < 0.15 { while t + 1 < tmax && dn[t + 1] < dn[t] { t += 1; } yt = t; break; }
            t += 1;
        }
        if yt > 0 && gate[fi] { f0_yin[fi] = fsf / interp(yt).0; }
        // pYIN multi-threshold
        let mut probs: Vec<(usize, f64)> = Vec::new();
        let mut t = tmin;
        let mut thr_i = 0usize;
        // thresholds increasing: first crossing tau is non-increasing in tau as threshold grows; scan once per threshold (cheap: 100 x tmax)
        while thr_i < NTHR {
            let s = (thr_i + 1) as f64 / NTHR as f64;
            t = tmin;
            let mut found = 0usize;
            while t < tmax {
                if dn[t] < s { let mut u = t; while u + 1 < tmax && dn[u + 1] < dn[u] { u += 1; } found = u; break; }
                t += 1;
            }
            if found > 0 {
                if let Some(p) = probs.iter_mut().find(|p| p.0 == found) { p.1 += prior[thr_i]; } else { probs.push((found, prior[thr_i])); }
            }
            thr_i += 1;
        }
        let _ = t;
        let g = if gate[fi] { 1.0 } else { 0.01 };
        let mut cv: Vec<(f64, f64)> = probs.iter().map(|&(tau, p)| (fsf / interp(tau).0, p * g)).filter(|c| c.0 >= fmin * 0.95 && c.0 <= fmax * 1.05).collect();
        cv.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        cv.truncate(6);
        cands.push(cv);
        dstore.push(dn[..=tmax].iter().map(|&v| v as f32).collect());
    }
    // Viterbi: states = candidates + unvoiced
    let ln_stay = (0.99f64).ln();
    let ln_sw = (0.01f64).ln();
    let sigma = 2.0; // semitones per 10 ms
    let mut prev: Vec<(f64, f64)> = vec![]; // (f0, score) voiced states
    let mut prev_uv = 0.0f64;
    let mut back: Vec<Vec<usize>> = Vec::with_capacity(nf); // for each state of frame, index of prev state (len = prev cands; last = uv)
    let mut vprob = vec![0f64; nf];
    for fi in 0..nf {
        let cv = &cands[fi];
        let vs: f64 = cv.iter().map(|c| c.1).sum::<f64>().min(1.0);
        vprob[fi] = vs;
        let e_uv = (1.0 - vs).max(1e-3).ln();
        let mut cur: Vec<(f64, f64)> = Vec::with_capacity(cv.len());
        let mut bk: Vec<usize> = Vec::with_capacity(cv.len() + 1);
        let puv_idx = prev.len();
        for c in cv {
            let e = c.1.max(1e-6).ln();
            let mut best = prev_uv + ln_sw; let mut bi = puv_idx;
            for (pi, p) in prev.iter().enumerate() {
                let st = 12.0 * (c.0 / p.0).log2();
                let s = p.1 + ln_stay - 0.5 * (st / sigma).powi(2);
                if s > best { best = s; bi = pi; }
            }
            cur.push((c.0, best + e)); bk.push(bi);
        }
        let mut best = prev_uv + ln_stay; let mut bi = puv_idx;
        for (pi, p) in prev.iter().enumerate() { let s = p.1 + ln_sw; if s > best { best = s; bi = pi; } }
        let uv = best + e_uv; bk.push(bi);
        // normalise to avoid drift
        let m = cur.iter().map(|c| c.1).fold(uv, f64::max);
        for c in &mut cur { c.1 -= m; }
        prev_uv = uv - m;
        prev = cur;
        back.push(bk);
    }
    let mut f0 = vec![0f64; nf];
    if nf > 0 {
        let mut state = prev.len(); let mut bs = prev_uv;
        for (i, p) in prev.iter().enumerate() { if p.1 > bs { bs = p.1; state = i; } }
        for fi in (0..nf).rev() {
            let ncand = cands[fi].len();
            if state < ncand { f0[fi] = cands[fi][state].0; }
            state = back[fi][state.min(ncand)];
        }
    }
    // octave-up correction: frames > 1.75x the utterance median whose CMNDF also dips at twice the lag
    let fix = |trk: &mut Vec<f64>| {
        let mut v: Vec<f64> = trk.iter().cloned().filter(|&f| f > 0.0).collect();
        if v.len() < 10 { return; }
        v.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let med = v[v.len() / 2];
        for fi in 0..nf {
            let f = trk[fi];
            if f <= 1.75 * med || dstore[fi].is_empty() { continue; }
            let t2 = 2.0 * fsf / f;
            let (lo, hi) = (((t2 * 0.92) as usize).max(tmin), ((t2 * 1.08).ceil() as usize).min(tmax - 1));
            if lo >= hi { continue; }
            let dd = &dstore[fi];
            let mut bt = lo; for t in lo..=hi { if dd[t] < dd[bt] { bt = t; } }
            let tc = ((fsf / f).round() as usize).clamp(1, tmax);
            if dd[bt] < 0.6 && dd[bt] < dd[tc] && bt > lo && bt < hi {
                let (y0, y1, y2) = (dd[bt - 1] as f64, dd[bt] as f64, dd[bt + 1] as f64);
                let den = y0 - 2.0 * y1 + y2;
                let dt = if den.abs() > 1e-12 { (0.5 * (y0 - y2) / den).clamp(-1.0, 1.0) } else { 0.0 };
                trk[fi] = fsf / (bt as f64 + dt);
            }
        }
    };
    let mut f0 = f0; let mut f0_yin = f0_yin;
    fix(&mut f0); fix(&mut f0_yin);
    PitchTrack { f0, vprob, f0_yin }
}
