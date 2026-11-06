#[path="../../src/pauses.rs"] mod pauses;
use std::{env,fs,path::Path};
fn read(p:&str)->(Vec<f32>,hound::WavSpec) {
 let mut r=hound::WavReader::open(p).expect("read WAV");let s=r.spec(); assert_eq!(s.channels,1,"mono required");
 let x=if s.sample_format==hound::SampleFormat::Float {r.samples::<f32>().map(Result::unwrap).collect()} else {let scale=2f32.powi(s.bits_per_sample as i32-1);r.samples::<i32>().map(|v|v.unwrap() as f32/scale).collect()}; (x,s)
}
fn main(){
 let a:Vec<String>=env::args().collect();
 assert!(a.len()==6 || a.len()==7,"usage: cbx-pauses INPUT OUTPUT REF TEXTFILE META [--punctuation-pauses]");
 assert!(!Path::new(&a[2]).exists(),"refuse overwrite");
 if a.len()==6 {fs::copy(&a[1],&a[2]).unwrap();fs::write(&a[5],"off: byte-identical copy\n").unwrap();return;}
 assert_eq!(a[6],"--punctuation-pauses");
 let (x,s)=read(&a[1]);let (r,rs)=read(&a[3]);let st=pauses::reference_stats(&r,rs.sample_rate as usize);
 let text=fs::read_to_string(&a[4]).unwrap();let (y,n)=pauses::insert(&x,s.sample_rate as usize,&text,Some(&st));
 if n==0 {fs::copy(&a[1],&a[2]).unwrap();} else {
 let mut w=hound::WavWriter::create(&a[2],s).unwrap();
 if s.sample_format==hound::SampleFormat::Float {for v in &y {w.write_sample(*v).unwrap();}} else {let scale=2f32.powi(s.bits_per_sample as i32-1);for v in &y {w.write_sample((v*scale).round() as i32).unwrap();}} w.finalize().unwrap();}
 fs::write(&a[5],format!("reference={st:?}; inserted={n}; input_samples={}; output_samples={}\n",x.len(),y.len())).unwrap();
}
