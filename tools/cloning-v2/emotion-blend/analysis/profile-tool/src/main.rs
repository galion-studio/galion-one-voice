#[path="../../../../scorers/emotion/src/audio.rs"] mod audio;
#[path="../../../../scorers/emotion/src/proxies.rs"] mod proxies;
use anyhow::Result;
use serde_json::json;
fn main()->Result<()> {
 let mut rows=Vec::new();
 for path in std::env::args().skip(1) {
  let a=audio::load_wav(&path)?; let x=audio::resample(&a.samples,a.sr,16000);
  let db=proxies::frame_db(&x); let max=db.iter().copied().fold(-120.0_f32,f32::max);
  let (f,_)=proxies::yin_f0(&x,&db,(max-35.0).max(-55.0));
  let z:Vec<(f64,f64)>=f.iter().enumerate().filter(|(_,v)| **v>0.0).map(|(i,v)|(i as f64*0.01,12.0*(*v as f64).log2())).collect();
  let n=z.len() as f64; let mt=z.iter().map(|v|v.0).sum::<f64>()/n;let my=z.iter().map(|v|v.1).sum::<f64>()/n;
  let den=z.iter().map(|v|(v.0-mt).powi(2)).sum::<f64>();
  let slope=if den>0.0 {Some(z.iter().map(|v|(v.0-mt)*(v.1-my)).sum::<f64>()/den)}else{None};
  let hz:Vec<f64>=f.iter().copied().filter(|v|*v>0.0).map(|v|v as f64).collect();let mh=hz.iter().sum::<f64>()/n;
  rows.push(json!({"file":path,"f0_contour_slope_st_s":slope,"f0_sd_hz":(hz.iter().map(|v|(v-mh).powi(2)).sum::<f64>()/(n-1.0)).sqrt(),"f0_min_hz":hz.iter().copied().reduce(f64::min),"f0_max_hz":hz.iter().copied().reduce(f64::max)}));
 }
 println!("{}",serde_json::to_string_pretty(&rows)?); Ok(())
}
