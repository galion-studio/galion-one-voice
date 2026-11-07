//! Conservative punctuation pauses. No phoneme alignment claim: punctuation times
//! are word-fraction estimates, snapped to existing low-energy boundaries.
//! Reference pauses require >=200 ms to avoid counting stop closures.
//! No suitable boundary means no insertion; never deliberately split voiced audio.
#[derive(Debug, Clone)]
pub struct PauseStats { pub median_seconds: f32, pub pause_ratio: f32, pub n: usize }
fn energy(x: &[f32], frame: usize) -> Vec<f32> {
    x.chunks(frame).map(|c| (c.iter().map(|v| v*v).sum::<f32>() / c.len() as f32).sqrt()).collect()
}
pub fn reference_stats(x: &[f32], sr: usize) -> PauseStats {
    let frame=(sr/100).max(1); let e=energy(x,frame);
    let peak=e.iter().copied().fold(0.0f32,f32::max);
    let threshold=(peak*0.10).max(0.0001);
    let first=e.iter().position(|v| *v>threshold).unwrap_or(0);
    let last=e.iter().rposition(|v| *v>threshold).unwrap_or(0);
    let mut pauses=Vec::new(); let mut run=0;
    for (i,v) in e.iter().enumerate() {
        if i<first || i>last {continue;}
        if *v<=threshold {run+=1;} else { if run>=20 {pauses.push(run as f32*frame as f32/sr as f32);} run=0; }
    }
    pauses.sort_by(|a,b| a.total_cmp(b));
    let n=pauses.len();
    let median=if n==0 {0.0} else if n%2==0 {(pauses[n/2-1]+pauses[n/2])*0.5} else {pauses[n/2]};
    PauseStats{median_seconds:median,pause_ratio:pauses.iter().sum::<f32>()/(x.len().max(1) as f32/sr as f32),n}
}
pub fn insert(x: &[f32], sr: usize, text: &str, stats: Option<&PauseStats>) -> (Vec<f32>,usize) {
    let Some(stats)=stats else {return (x.to_vec(),0)};
    if stats.n==0 || x.is_empty() {return (x.to_vec(),0)}
    let words:Vec<&str>=text.split_whitespace().collect();
    if words.len()<2 {return (x.to_vec(),0)}
    let frame=(sr/100).max(1); let e=energy(x,frame);
    let peak=e.iter().copied().fold(0.0f32,f32::max);
    let threshold=(peak*0.10).max(0.0001);
    let mut edits=Vec::new(); let mut last=0usize;
    for (i,w) in words.iter().enumerate().take(words.len()-1) {
        let w=w.trim_end_matches(['"','\'','”',')',']']);
        let sentence=w.ends_with(['.','!','?']);
        if !sentence && !w.ends_with([',',';',':','—']) {continue;}
        let estimated=(i+1)*e.len()/words.len();
        let start=estimated.saturating_sub(25).max(last+1).max(2);
        let end=(estimated+26).min(e.len().saturating_sub(2));
        if start>=end {continue;}
        // Require 30 ms quiet; then snap to its minimum-energy frame.
        let candidate=(start..end).filter(|j| e[j-1]<=threshold && e[*j]<=threshold && e[j+1]<=threshold)
            .min_by(|a,b| e[*a].total_cmp(&e[*b]).then(a.abs_diff(estimated).cmp(&b.abs_diff(estimated))));
        let Some(j)=candidate else {continue;};
        let mut lo=j; let mut hi=j;
        while lo>0 && e[lo-1]<=threshold {lo-=1;}
        while hi+1<e.len() && e[hi+1]<=threshold {hi+=1;}
        let existing=(hi-lo+1) as f32*frame as f32/sr as f32;
        let target=(stats.median_seconds*if sentence {1.0} else {0.6}).clamp(0.08,0.8);
        let add=((target-existing).max(0.0)*sr as f32).round() as usize;
        if add>0 {edits.push((j*frame,add));last=hi;}
    }
    let mut out=Vec::with_capacity(x.len()+edits.iter().map(|(_,n)| *n).sum::<usize>());
    let mut prev=0;
    for (at,n) in &edits {out.extend_from_slice(&x[prev..*at]);out.resize(out.len()+n,0.0);prev=*at;}
    out.extend_from_slice(&x[prev..]); (out,edits.len())
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn off_is_identical() {let x=vec![0.3,0.0,-0.4]; assert_eq!(insert(&x,24000,"Hello, world.",None).0,x);}
    #[test] fn no_boundary_no_cut() {let x=vec![0.2;24000];let s=PauseStats{n:1,median_seconds:0.3,pause_ratio:0.2};assert_eq!(insert(&x,24000,"Hello, world.",Some(&s)).0,x);}
    #[test] fn adds_only_at_quiet_boundary() {let mut x=vec![0.2;24000];x[11040..12960].fill(0.0);let s=PauseStats{n:1,median_seconds:0.4,pause_ratio:0.2};let (y,n)=insert(&x,24000,"Hello, world.",Some(&s));assert_eq!(n,1);assert!(y.len()>x.len()); assert_eq!(y.iter().filter(|v| **v==0.2).count(),x.iter().filter(|v| **v==0.2).count());}
}
