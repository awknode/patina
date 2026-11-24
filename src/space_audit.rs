//! How loud SPACE is at low MIX (test only): `cargo test --release --lib space_audit -- --nocapture`
use super::*;
use crate::params::M_REVERB;

#[test]
fn space_audit() {
    let sr = 48000.0f32;
    // a short click-y drum hit every half second for 2 s, then 2 s of silence (the tail)
    let n = (sr * 4.0) as usize;
    let mut x = vec![0.0f32; n];
    for h in 0..4 {
        let s0 = (h as f32 * 0.5 * sr) as usize;
        for i in 0..(0.08 * sr) as usize {
            let t = i as f32 / sr;
            x[s0 + i] = (std::f32::consts::TAU * 180.0 * t).sin() * (-t * 40.0).exp() * 0.7;
        }
    }
    let rms = |v: &[f32]| (v.iter().map(|a| a * a).sum::<f32>() / v.len() as f32).sqrt();
    let db = |v: f32| 20.0 * v.max(1e-9).log10();
    let hit_rms = rms(&x[..(2.0 * sr) as usize]);
    for var in [0usize, 4] {
        for mix in [0.01f32, 0.05, 0.1, 0.2, 0.3] {
            let mut st = State::with(&[M_REVERB]);
            st.m[M_REVERB].mix = mix;
            st.m[M_REVERB].var = var;
            let mut e = Engine::new(sr, 0);
            let (mut l, mut r) = (x.clone(), x.clone());
            for (a, b) in l.chunks_mut(64).zip(r.chunks_mut(64)) {
                e.process(a, b, &st, Transport { bpm: Some(120.0), beat: None, playing: false });
            }
            let lat = 0; // dry alignment not needed for the tail
            let _ = lat;
            let tail = rms(&l[(2.3 * sr) as usize..(3.0 * sr) as usize]);
            let d: Vec<f32> = l.iter().zip(&x).map(|(a, b)| a - b).collect();
            println!("var {var} MIX {:>3.0}%  tail {:>6.1} dB vs hits   change in hits {:>6.1} dB", mix * 100.0, db(tail) - db(hit_rms), db(rms(&d[..(2.0 * sr) as usize])) - db(hit_rms));
        }
    }
}

#[test]
fn chow_ab_level() {
    use crate::params::M_MAGNETIC;
    let sr = 48000.0f32;
    let x: Vec<f32> = (0..(sr * 2.0) as usize).map(|i| { let t = i as f32 / sr; ((std::f32::consts::TAU * 110.0 * t).sin() * 0.5 + (std::f32::consts::TAU * 1760.0 * t).sin() * 0.15) * if (t * 2.0).fract() < 0.6 { 1.0 } else { 0.1 } }).collect();
    let rms = |v: &[f32]| (v.iter().map(|a| a * a).sum::<f32>() / v.len() as f32).sqrt();
    let db = |v: f32| 20.0 * v.max(1e-9).log10();
    for var in 0..6usize {
        for sat in [0.2f32, 0.6, 1.0] {
            let mut st = State::with(&[M_MAGNETIC]);
            st.m[M_MAGNETIC].var = var;
            st.m[M_MAGNETIC].k[2] = sat;
            st.m[M_MAGNETIC].k[3] = 0.0;
            let mut e = Engine::new(sr, 0);
            let (mut l, mut r) = (x.clone(), x.clone());
            for (a, b) in l.chunks_mut(64).zip(r.chunks_mut(64)) {
                e.process(a, b, &st, Transport { bpm: Some(120.0), beat: None, playing: false });
            }
            assert!(l.iter().all(|v| v.is_finite()));
            println!("mag var {var} SAT {:>3.0}%: level {:+5.1} dB", sat * 100.0, db(rms(&l[4800..])) - db(rms(&x[4800..])));
        }
    }
}
