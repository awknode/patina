//! WAV_OUT=dir: renders 10 s of each record flavor over a quiet chord (listening check).
use patina::*;
fn wav(path: &str, l: &[f32], r: &[f32]) {
    let n = l.len() as u32;
    let mut b = Vec::new();
    b.extend(b"RIFF"); b.extend((36 + n * 4).to_le_bytes()); b.extend(b"WAVEfmt ");
    b.extend(16u32.to_le_bytes()); b.extend(1u16.to_le_bytes()); b.extend(2u16.to_le_bytes());
    b.extend(48000u32.to_le_bytes()); b.extend((48000u32 * 4).to_le_bytes()); b.extend(4u16.to_le_bytes()); b.extend(16u16.to_le_bytes());
    b.extend(b"data"); b.extend((n * 4).to_le_bytes());
    for i in 0..l.len() { for v in [l[i], r[i]] { b.extend(((v.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes()); } }
    std::fs::write(path, b).unwrap();
}
#[test]
fn render() {
    let Ok(dir) = std::env::var("WAV_OUT") else { return };
    std::fs::create_dir_all(&dir).unwrap();
    for (var, name) in [(0usize, "vinyl_a"), (4, "vinyl_b"), (5, "shellac"), (6, "dust"), (1, "tape")] {
        let mut st = State::with(&[M_NOISE]);
        st.m[M_NOISE].var = var;
        st.m[M_NOISE].k = [0.6, 0.5, 0.6, 0.0, 0.5, 0.45, 0.35];
        let mut e = Engine::new(48000.0, 1);
        let n = 480000;
        // a soft chord for the first half, silence after (FOLLOW is 0: the record keeps going)
        let x: Vec<f32> = (0..n).map(|i| { let t = i as f64 / 48000.0; if i < n / 2 { (0.06 * ((220.0 * std::f64::consts::TAU * t).sin() + (277.2 * std::f64::consts::TAU * t).sin() + (329.6 * std::f64::consts::TAU * t).sin())) as f32 } else { 0.0 } }).collect();
        let mut l = x.clone(); let mut r = x.clone();
        for (a, b) in l.chunks_mut(512).zip(r.chunks_mut(512)) { e.process(a, b, &st, Transport { bpm: Some(90.0), beat: None, playing: false }); }
        wav(&format!("{dir}/patina_noise_{name}.wav"), &l, &r);
    }
}
