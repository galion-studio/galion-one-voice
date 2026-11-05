// Galion Voice — Perpetual Open MIT License
// (c) 2026 Maciej Grajczyk / G.A.L.I.O.N.
//! Opt-in coherent, distribution-preserving prefix selection. No synthesis code.
mod audio;
use anyhow::{bail,Context,Result};
use serde_json::{json,Value};
use std::{path::{Path,PathBuf},process::Command,fs};
fn wav(path:&Path,x:&[f32],sr:u32)->Result<()> {
 let file=fs::OpenOptions::new().create_new(true).write(true).open(path)?;
 let mut w=hound::WavWriter::new(file,hound::WavSpec{channels:1,sample_rate:sr,bits_per_sample:32,sample_format:hound::SampleFormat::Float})?;
 for &v in x {w.write_sample(v)?;}w.finalize()?;Ok(())
}
fn stats(rows:&[(f64,f64,Vec<f64>)],start:f64,len:f64)->(Vec<f64>,Vec<f64>){
 let width=rows[0].2.len();let mut mean=vec![0.;width];let mut second=vec![0.;width];let mut total=0.;
 // Source circular interval; no scalar speech-token interpolation occurs.
 let duration=rows.last().unwrap().1;
 for (a,b,z) in rows {
  let mut w=0.;for k in -1..=1 {let l=start+k as f64*duration;w+=(b.min(l+len)-a.max(l)).max(0.);}
  total+=w;for j in 0..width {mean[j]+=w*z[j];second[j]+=w*z[j]*z[j];}
 }
 for j in 0..width {mean[j]/=total;second[j]=(second[j]/total-mean[j]*mean[j]).max(0.).sqrt();}
 (mean,second)
}
fn cost(rows:&[(f64,f64,Vec<f64>)],start:f64,target:&(Vec<f64>,Vec<f64>))->f64 {
 let (m,s)=stats(rows,start,6.0);let scales=[0.25,0.25,0.25,0.25,0.1,0.1,0.1,3.0,6.0,0.12,1.0,2.0];
 m.iter().enumerate().map(|(j,x)|((x-target.0[j])/scales[j]).powi(2)+0.5*((s[j]-target.1[j])/scales[j]).powi(2)).sum::<f64>()/m.len() as f64
}
fn main()->Result<()> {
 let a:Vec<String>=std::env::args().skip(1).collect();
 if a.iter().any(|s|s=="--help") {println!("galion-blend-ref --input REF.wav --output NEW.wav --meta NEW.json [--blend-preserve --emotion-scorer PATH]");return Ok(());}
 let get=|key:&str|->Result<PathBuf>{let i=a.iter().position(|s|s==key).with_context(||format!("missing {key}"))?;Ok(PathBuf::from(a.get(i+1).context("missing value")?))};
 let input=get("--input")?;let out=get("--output")?;let meta=get("--meta")?;
 if out.exists()||meta.exists(){bail!("refusing existing output or metadata");}
 let enabled=a.iter().any(|s|s=="--blend-preserve");
 if !enabled {let mut src=fs::File::open(&input)?;let mut dst=fs::OpenOptions::new().create_new(true).write(true).open(&out)?;std::io::copy(&mut src,&mut dst)?;fs::write(meta,serde_json::to_vec_pretty(&json!({"enabled":false,"mode":"byte-copy","input":input,"output":out}))?)?;return Ok(());}
 let aud=audio::load_wav(input.to_str().context("input not utf8")?)?;
 if aud.samples.iter().any(|v|!v.is_finite()){bail!("nonfinite reference");}
 let duration=aud.samples.len() as f64/aud.sr as f64;
 if duration<=6.0 {let mut src=fs::File::open(&input)?;let mut dst=fs::OpenOptions::new().create_new(true).write(true).open(&out)?;std::io::copy(&mut src,&mut dst)?;fs::write(meta,serde_json::to_vec_pretty(&json!({"enabled":true,"mode":"byte-copy-short-reference","duration_s":duration}))?)?;return Ok(());}
 let work=meta.with_extension("work");fs::create_dir(&work).context("work directory must be new")?;
 let scorer=get("--emotion-scorer")?;let mut paths=Vec::new();let mut ranges=Vec::new();let mut t=0.;
 while t<duration {let mut end=(t+2.5_f64).min(duration);if duration-end<1.0 {end=duration;}
  let p=work.join(format!("window-{:03}.wav",paths.len()));let i=(t*aud.sr as f64) as usize;let e=(end*aud.sr as f64).round() as usize;wav(&p,&aud.samples[i..e.min(aud.samples.len())],aud.sr)?;
  paths.push(p);ranges.push((t,end));t=end;
 }
 let scored=work.join("emotion.json");let status=Command::new(scorer).arg("score").args(["--threads","2","--out"]).arg(&scored).args(&paths).status()?;
 if !status.success(){bail!("fp32 emotion scorer failed; no prepared reference produced");}
 let scores:Value=serde_json::from_slice(&fs::read(scored)?)?;let files=scores["files"].as_array().context("missing files")?;
 let mut rows=Vec::new();
 for ((s,e),p) in ranges.iter().zip(paths.iter()) {
  let r=files.iter().find(|r|r["file"].as_str()==p.to_str()).context("missing scored window")?;
  let mut z=Vec::new();for k in ["neu","hap","ang","sad"]{z.push(r["ser"]["probs"][k].as_f64().context("SER missing")?);}
  for k in ["arousal","valence","dominance"] {z.push(r["avd"][k].as_f64().context("AVD missing")?);}
  let q=&r["proxies"];for k in ["rms_dbfs","energy_dr_db","pause_ratio","speech_rate_syl_s","spectral_tilt_db_oct"]{z.push(q[k].as_f64().context("proxy missing")?);}
  rows.push((*s,*e,z));
 }
 let target=stats(&rows,0.,duration);let baseline_cost=cost(&rows,0.,&target);
 // Quiet 20ms candidate cuts: keep complete contiguous prompt; no sentence medley.
 let frame=(aud.sr as f64*0.02) as usize;let db:Vec<f64>=aud.samples.chunks(frame).map(|c|10.*(c.iter().map(|v|(*v as f64).powi(2)).sum::<f64>()/c.len() as f64+1e-12).log10()).collect();
 let peak=db.iter().copied().fold(-120.,f64::max);let mut candidates=vec![(0.,baseline_cost)];
 for i in 1..db.len()-1 {if db[i]<=db[i-1] && db[i]<db[i+1] && db[i]<peak-25. {let s=i as f64*0.02;candidates.push((s,cost(&rows,s,&target)));}}
 candidates.sort_by(|a,b|a.1.total_cmp(&b.1).then(a.0.total_cmp(&b.0)));let mut selected=candidates[0].0;
 // Require meaningful improvement; neutral/short references often no-op.
 if candidates[0].1>baseline_cost*0.95 {selected=0.;}
 let cut=((selected*aud.sr as f64).round() as usize).min(aud.samples.len());
 if cut==0 {wav(&out,&aud.samples,aud.sr)?;}else{
  let mut y=aud.samples[cut..].to_vec();let join=y.len();y.extend_from_slice(&aud.samples[..cut]);
  // Fade only around the old end/start boundary, never normalize loudness or pitch.
  let fade=(aud.sr as f64*0.005) as usize;
  for i in 0..fade.min(join).min(y.len()-join) {let g=i as f32/fade as f32;y[join-1-i]*=g;y[join+i]*=g;}
  wav(&out,&y,aud.sr)?;
 }
 let report=json!({"schema":"galion-blend-ref/1","enabled":true,"mode":"coherent-prefix-rotation","input":input,"output":out,"duration_s":duration,"prefix_seconds":6.0,"selected_start_s":selected,"baseline_cost":baseline_cost,"selected_cost":cost(&rows,selected,&target),"target_mean":target.0,"target_population_sd":target.1,"selected_prefix_stats":stats(&rows,selected,6.0),"feature_order":["neu","hap","ang","sad","arousal","valence","dominance","rms_dbfs","energy_dr_db","pause_ratio","speech_rate_syl_s","spectral_tilt_db_oct"],"candidates":candidates,"source_windows":ranges,"scores":work.join("emotion.json"),"caveat":"window overlap approximates distributions; no guaranteed learned-model affect preservation"});
 fs::write(meta,serde_json::to_vec_pretty(&report)?)?;println!("{}",serde_json::to_string_pretty(&report)?);Ok(())
}
#[cfg(test)]mod tests {
 use super::*;
 #[test]fn circular_stats_coverage(){let r=vec![(0.,2.,vec![0.;12]),(2.,4.,vec![1.;12])];let (m,s)=stats(&r,0.,4.);assert!((m[0]-0.5).abs()<1e-8);assert!((s[0]-0.5).abs()<1e-8);let (m,_)=stats(&r,3.,2.);assert!((m[0]-0.5).abs()<1e-8);}
 #[test]fn identical_distribution_zero_cost(){let r=vec![(0.,3.,vec![0.5;12]),(3.,8.,vec![0.5;12])];let t=stats(&r,0.,8.);assert!(cost(&r,2.,&t)<1e-9);}
}
