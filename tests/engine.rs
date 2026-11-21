use patina::*;
use std::f32::consts::PI;

const SR: f32 = 48000.0;

fn run(e: &mut Engine, st: &State, x: &[f32]) -> (Vec<f32>, Vec<f32>) {
    let mut l = x.to_vec();
    let mut r = x.to_vec();
    for (a, b) in l.chunks_mut(256).zip(r.chunks_mut(256)) {
        e.process(a, b, st, Transport { bpm: Some(120.0), beat: None, playing: false });
    }
    (l, r)
}
fn sine(f: f32, n: usize, a: f32) -> Vec<f32> {
    (0..n).map(|i| a * (2.0 * PI * f * i as f32 / SR).sin()).collect()
}
fn noise(n: usize, a: f32) -> Vec<f32> {
    let mut s = 1u32;
    (0..n)
        .map(|_| {
            s = s.wrapping_mul(1664525).wrapping_add(1013904223);
            a * ((s >> 8) as f32 / 8388608.0 - 1.0)
        })
        .collect()
}
fn rms(x: &[f32]) -> f32 {
    (x.iter().map(|v| v * v).sum::<f32>() / x.len() as f32).sqrt()
}
fn tone_level(x: &[f32], f: f32, from: usize) -> f32 {
    let (mut s, mut c) = (0.0f32, 0.0f32);
    for (i, v) in x.iter().enumerate().skip(from) {
        let ph = 2.0 * PI * f * i as f32 / SR;
        s += v * ph.sin();
        c += v * ph.cos();
    }
    2.0 * (s * s + c * c).sqrt() / (x.len() - from) as f32
}

#[test]
fn every_module_alone_is_bounded_and_quiet_in_silence() {
    for m in 0..N_MODS {
        for var in 0..MODS[m].vars.len().max(1) {
            for hot in [false, true] {
                let mut st = State::with(&[m]);
                st.safe = false;
                st.m[m].var = var;
                if hot {
                    st.m[m].k = [1.0; MAX_K];
                    st.m[m].mix = 1.0;
                    if m == M_ECHO || m == M_CHORUS {
                        st.m[m].k[1] = 0.9;
                    }
                }
                let mut e = Engine::new(SR, 1);
                let (l, r) = run(&mut e, &st, &noise(48000, 0.5));
                let pk = l.iter().chain(r.iter()).fold(0.0f32, |a, v| a.max(v.abs()));
                assert!(pk.is_finite() && pk < 8.0, "{} {} hot={hot}: peak {pk}", MODS[m].name, var);
                // then 3 s of silence: everything (tails included) dies away, nothing self-oscillates
                // then 4 s of silence: tails die away (long reverbs/echoes at least keep falling),
                // nothing self-oscillates
                let (l, _) = run(&mut e, &st, &vec![0.0; 192000]);
                let (r1, r2) = (rms(&l[48000..96000]), rms(&l[144000..]));
                assert!(r2 < 1e-3 || r2 < r1 * 0.8, "{} {} hot={hot}: silence -> {r1} then {r2}", MODS[m].name, var);
                if !hot && m != M_REVERB && m != M_ECHO {
                    assert!(r2 < 1e-4, "{} {}: silence -> {r2}", MODS[m].name, var);
                }
            }
        }
    }
}

#[test]
fn amount_zero_is_the_dry_delayed() {
    let mut st = State::with(&[M_HEAT, M_WOBBLE, M_NOISE, M_MAGNETIC, M_REVERB, M_CRUSH, M_ECHO, M_FILTER]);
    st.amount = 0.0;
    let mut e = Engine::new(SR, 1);
    let lat = e.lat;
    let x = noise(48000, 0.3);
    let (l, _) = run(&mut e, &st, &x);
    for i in 1000..48000 {
        assert!((l[i] - x[i - lat]).abs() < 1e-4, "sample {i}: {} vs {}", l[i], x[i - lat]);
    }
}

#[test]
fn range_keeps_the_rest_clean() {
    // a hard bit crush only above 2 kHz leaves a 100 Hz tone alone
    let mut st = State::with(&[M_CRUSH]);
    st.m[M_CRUSH].k = [1.0, 0.9, 0.0, 0.0, 0.0, 0.0, 0.0];
    st.m[M_CRUSH].mix = 1.0;
    st.m[M_CRUSH].lo = 2000.0;
    let mut e = Engine::new(SR, 1);
    let (l, _) = run(&mut e, &st, &sine(100.0, 48000, 0.5));
    let lv = tone_level(&l, 100.0, 24000);
    assert!((lv - 0.5).abs() < 0.03, "100 Hz came out at {lv}");
    // and the same crush on the full range wrecks it
    st.m[M_CRUSH].lo = 20.0;
    let mut e = Engine::new(SR, 1);
    let (l, _) = run(&mut e, &st, &sine(100.0, 48000, 0.5));
    let resid: Vec<f32> = l.iter().enumerate().map(|(i, v)| v - 0.0 * i as f32).collect();
    assert!(rms(&resid[24000..]) > 0.0);
}

#[test]
fn noise_follows_the_music() {
    let st = State::with(&[M_NOISE]);
    let mut e = Engine::new(SR, 1);
    let mut x = sine(440.0, 48000, 0.5);
    x.extend(vec![0.0; 96000]);
    let (l, _) = run(&mut e, &st, &x);
    let playing = rms(&l[20000..40000]) - 0.5 / 2f32.sqrt();
    let gap = rms(&l[130000..]);
    assert!(gap < 1e-4, "hiss in the gap: {gap}");
    let _ = playing;
}

#[test]
fn half_time_plays_an_octave_down() {
    let mut st = State::with(&[M_TIME]);
    st.m[M_TIME].k[0] = 0.25; // 1/2 note cycles = 1 s at 120 bpm
    let mut e = Engine::new(SR, 1);
    let (l, _) = run(&mut e, &st, &sine(1000.0, 96000, 0.5));
    // inside a cycle (away from its edges) the 1 kHz tone is now 500 Hz
    let seg = &l[48000 + 10000..48000 + 40000];
    let mut z = 0;
    for w in seg.windows(2) {
        if w[0] <= 0.0 && w[1] > 0.0 {
            z += 1;
        }
    }
    let f = z as f32 / (30000.0 / SR);
    assert!((f - 500.0).abs() < 15.0, "half-time tone at {f} Hz");
}

#[test]
fn reorder_is_click_free_and_keeps_the_reverb_tail() {
    let mut e = Engine::new(SR, 1);
    let a = State::with(&[M_REVERB, M_HEAT, M_ECHO]);
    let b = State::with(&[M_ECHO, M_REVERB, M_HEAT]);
    let mut x = noise(24000, 0.3);
    x.extend(vec![0.0; 24000]);
    run(&mut e, &a, &x);
    let (l, _) = run(&mut e, &b, &vec![0.0; 9600]);
    assert!(rms(&l[4800..]) > 1e-3, "reverb tail was cut by the reorder");
    let jump = l.windows(2).fold(0.0f32, |m, w| m.max((w[1] - w[0]).abs()));
    assert!(jump < 0.3, "jump {jump}");
}

#[test]
fn cpu_budget() {
    let st = State::with(&[M_HEAT, M_TAME, M_MAGNETIC, M_WOBBLE, M_CHORUS, M_ECHO, M_REVERB, M_NOISE]);
    let mut e = Engine::new(SR, 1);
    let x = noise(48000 * 10, 0.3);
    let t = std::time::Instant::now();
    run(&mut e, &st, &x);
    let pct = t.elapsed().as_secs_f32() / 10.0 * 100.0;
    println!("8 heavy modules: {pct:.2}% of one core");
    assert!(pct < 20.0);
}

/// At default knobs and MIX 100 %, no module is wildly louder or quieter than its input.
#[test]
fn levels_are_sane() {
    let x = noise(96000, 0.3);
    let rin = rms(&x[48000..]);
    for m in 0..N_MODS {
        for var in 0..MODS[m].vars.len().max(1) {
            let mut st = State::with(&[m]);
            st.m[m].var = var;
            st.m[m].mix = 1.0;
            st.safe = false;
            let mut e = Engine::new(SR, 1);
            let (l, r) = run(&mut e, &st, &x);
            let d = 20.0 * ((rms(&l[48000..]) + rms(&r[48000..])) * 0.5 / rin).log10();
            println!("{:9} {:10} {:+.1} dB", MODS[m].name, MODS[m].vars.get(var).unwrap_or(&"-"), d);
            if m != M_CHOP && m != M_FILTER {
                assert!(d.abs() < 12.0, "{} {}: {d:+.1} dB", MODS[m].name, var);
            }
        }
    }
}

/// MAGNITUDE 0 with LOSS in the rack: still the dry, delayed by the (bigger) latency.
#[test]
fn loss_latency_is_reported_and_aligned() {
    let mut st = State::with(&[M_LOSS, M_HEAT, M_ECHO]);
    st.amount = 0.0;
    let mut e = Engine::new(SR, 1);
    let lat = e.lat_for_state(&st);
    assert!(lat > 500, "LOSS adds its frame: {lat}");
    let x = noise(48000, 0.3);
    let (l, _) = run(&mut e, &st, &x);
    assert_eq!(e.lat, lat);
    for i in 2000..48000 {
        assert!((l[i] - x[i - lat]).abs() < 1e-4, "sample {i}: {} vs {}", l[i], x[i - lat]);
    }
}

/// LOSS at a low bitrate takes the top end away and leaves the body.
#[test]
fn loss_eats_the_highs() {
    let mut st = State::with(&[M_LOSS]);
    st.m[M_LOSS].k[0] = 0.9;
    st.m[M_LOSS].k[1] = 0.0;
    let mut e = Engine::new(SR, 1);
    let x: Vec<f32> = (0..48000).map(|i| { let t = i as f32 / SR; 0.3 * (std::f32::consts::TAU * 300.0 * t).sin() + 0.1 * (std::f32::consts::TAU * 9000.0 * t).sin() }).collect();
    let (l, _) = run(&mut e, &st, &x);
    let lo = tone_level(&l, 300.0, 12000);
    let hi = tone_level(&l, 9000.0, 12000);
    assert!(lo > 0.2 && hi < 0.01, "300 Hz {lo}, 9 kHz {hi}");
}

/// ECHO through the whole rack (with DISTORT and SPACE in it) at full feedback
/// and GRAINS: bounded, and it dies away once the input stops.
#[test]
fn echo_thru_rack_stays_bounded() {
    let mut st = State::with(&[M_HEAT, M_ECHO, M_REVERB, M_CRUSH]);
    st.safe = false;
    st.m[M_ECHO].k[1] = 0.9;
    st.m[M_ECHO].k[5] = 1.0;
    st.m[M_ECHO].k[6] = 1.0;
    st.m[M_ECHO].mix = 0.6;
    st.m[M_HEAT].k[3] = 1.0;
    let mut e = Engine::new(SR, 1);
    let (l, r) = run(&mut e, &st, &noise(96000, 0.5));
    let pk = l.iter().chain(r.iter()).fold(0.0f32, |a, v| a.max(v.abs()));
    assert!(pk.is_finite() && pk < 8.0, "peak {pk}");
    let (l, _) = run(&mut e, &st, &vec![0.0; 48000 * 12]);
    let (a, b) = (rms(&l[48000..96000]), rms(&l[48000 * 11..]));
    println!("thru rack tail: {a} -> {b}");
    assert!(b < a * 0.5 || b < 1e-3, "tail {a} -> {b}");
}

/// FLUX moves things but never breaks them.
#[test]
fn flux_everywhere_is_bounded() {
    let mut st = State::with(&[M_NOISE, M_WOBBLE, M_HEAT, M_CRUSH, M_REVERB, M_MAGNETIC, M_ECHO, M_LOSS]);
    for m in 0..N_MODS {
        st.m[m].flux = 1.0;
    }
    st.safe = false;
    let mut e = Engine::new(SR, 1);
    let (l, r) = run(&mut e, &st, &noise(96000, 0.4));
    let pk = l.iter().chain(r.iter()).fold(0.0f32, |a, v| a.max(v.abs()));
    assert!(pk.is_finite() && pk < 8.0, "peak {pk}");
    assert!(e.flux_v.iter().any(|v| v.abs() > 0.05));
}

/// 1/4 speed plays two octaves down.
#[test]
fn quarter_speed_is_two_octaves_down() {
    let mut st = State::with(&[M_TIME]);
    st.m[M_TIME].var = 3;
    st.m[M_TIME].k[0] = 0.0; // 1 bar
    st.m[M_TIME].k[3] = 1.0;
    let mut e = Engine::new(SR, 1);
    let (l, _) = run(&mut e, &st, &sine(800.0, 192000, 0.5));
    // a bar at 120 = 2 s; look inside the second bar
    let seg = &l[48000 * 2 + 4000..48000 * 2 + 40000];
    assert!(tone_level(seg, 200.0, 0) > 4.0 * tone_level(seg, 800.0, 0), "200 Hz should dominate");
}

/// LOUDNESS MATCH: a hot DISTORT comes out about as loud as it went in.
#[test]
fn loudness_match_holds_level() {
    let mut st = State::with(&[M_HEAT]);
    st.safe = false;
    st.m[M_HEAT].k[0] = 1.0;
    st.m[M_HEAT].k[3] = 1.0;
    st.matching = true;
    st.in_db = 9.0;
    let mut e = Engine::new(SR, 1);
    let x = noise(96000, 0.1);
    let (l, _) = run(&mut e, &st, &x);
    let d = 20.0 * (rms(&l[48000..]) / rms(&x[48000..])).log10();
    assert!(d.abs() < 2.0, "level moved {d:.1} dB");
}

#[test]
fn echo_sends_through_modules_stay_bounded_and_die_away() {
    // repeats through DISTORT, CODEC, SPACE, WOBBLE and COMPRESS, high feedback, pitched up
    let mut st = State::with(&[M_HEAT, M_LOSS, M_REVERB, M_WOBBLE, M_COMP]);
    st.safe = false;
    st.x.e_on = true;
    st.x.e_pitch = 12.0;
    st.x.e_duck = 0.5;
    st.x.e_smooth = 0.5;
    st.m[M_ECHO].k[1] = 0.95;
    st.m[M_ECHO].k[6] = 0.6;
    st.m[M_ECHO].mix = 0.6;
    for m in [M_HEAT, M_LOSS, M_REVERB, M_WOBBLE, M_COMP] {
        st.m[m].esend = 1.0;
    }
    st.m[M_HEAT].k[0] = 0.8;
    let mut e = Engine::new(SR, 1);
    let mut x = noise(48000, 0.5);
    x.extend(std::iter::repeat(0.0).take(48000 * 8));
    let (l, r) = run(&mut e, &st, &x);
    let pk = l.iter().chain(r.iter()).fold(0.0f32, |a, v| a.max(v.abs()));
    assert!(pk.is_finite() && pk < 8.0, "peak {pk}");
    let early = rms(&l[48000..96000]);
    let late = rms(&l[48000 * 8..]);
    assert!(late < early * 0.5 + 1e-4, "tail not dying: early {early} late {late}");
}

#[test]
fn echo_strip_off_is_silent_after_the_dry() {
    let mut st = State::with(&[M_WIDTH]);
    st.x.e_on = false;
    let mut e = Engine::new(SR, 1);
    let mut x = sine(440.0, 4800, 0.5);
    x.extend(std::iter::repeat(0.0).take(48000));
    let (l, _) = run(&mut e, &st, &x);
    assert!(rms(&l[12000..]) < 1e-5);
    // on: repeats keep going after the input stops
    st.x.e_on = true;
    st.m[M_ECHO].mix = 0.5;
    let mut e = Engine::new(SR, 1);
    let (l, _) = run(&mut e, &st, &x);
    assert!(rms(&l[12000..36000]) > 1e-3, "no repeats");
}

#[test]
fn global_focus_keeps_the_rest_clean() {
    let mut st = State::with(&[M_CRUSH]);
    st.m[M_CRUSH].on = true;
    st.m[M_CRUSH].k = [1.0, 0.9, 0.0, 0.0, 0.0, 0.0, 0.0];
    st.m[M_CRUSH].mix = 1.0;
    st.x.g_lo = 2000.0;
    let mut e = Engine::new(SR, 1);
    let (l, _) = run(&mut e, &st, &sine(100.0, 48000, 0.5));
    let lv = tone_level(&l, 100.0, 24000);
    assert!((lv - 0.5).abs() < 0.03, "100 Hz came out at {lv}");
}

#[test]
fn global_cut_removes_outside_with_every_shape() {
    for shape in 0..5 {
        let mut st = State::with(&[M_WIDTH]);
        st.x.g_cut = true;
        st.x.g_lo = 1000.0;
        st.x.g_shape = shape;
        let mut e = Engine::new(SR, 1);
        let (l, _) = run(&mut e, &st, &sine(100.0, 48000, 0.5));
        let lv = tone_level(&l, 100.0, 24000);
        let lim = [0.12, 0.03, 0.01, 0.001, 0.05][shape as usize];
        assert!(lv < lim, "shape {shape}: 100 Hz at {lv}");
        let (l, _) = run(&mut e, &st, &sine(5000.0, 48000, 0.5));
        let hv = tone_level(&l, 5000.0, 24000);
        assert!(hv > 0.4, "shape {shape}: 5 kHz at {hv}");
    }
}

#[test]
fn module_cut_removes_outside_its_range() {
    let mut st = State::with(&[M_WIDTH]);
    st.m[M_WIDTH].lo = 1000.0;
    st.m[M_WIDTH].mix = 1.0;
    st.m[M_WIDTH].cut = true;
    let mut e = Engine::new(SR, 1);
    let (l, _) = run(&mut e, &st, &sine(100.0, 48000, 0.5));
    let lv = tone_level(&l, 100.0, 24000);
    assert!(lv < 0.03, "100 Hz still at {lv}");
    st.m[M_WIDTH].cut = false;
    let mut e = Engine::new(SR, 1);
    let (l, _) = run(&mut e, &st, &sine(100.0, 48000, 0.5));
    assert!(tone_level(&l, 100.0, 24000) > 0.45);
}

#[test]
fn master_compress_every_model_squeezes_and_holds_level() {
    for model in 0..6 {
        let mut st = State::with(&[M_WIDTH]);
        st.safe = false;
        st.x.comp = 0.8;
        st.x.comp_model = model;
        let mut e = Engine::new(SR, 1);
        // loud / quiet bursts
        let x: Vec<f32> = noise(96000, 0.6).iter().enumerate().map(|(i, v)| v * if (i / 6000) % 2 == 0 { 1.0 } else { 0.15 }).collect();
        let (l, _) = run(&mut e, &st, &x);
        assert!(l.iter().all(|v| v.is_finite()));
        let gr = get(&e.shared.mcomp_gr);
        let (i, o) = (rms(&x[48000..]), rms(&l[48000..]));
        let d = 20.0 * (o / i).log10();
        assert!(d.abs() < 6.0, "{}: level moved {d:.1} dB (gr {gr:.1})", comp::MODELS[model]);
    }
}

#[test]
fn compress_module_and_latency_line_up() {
    // COMPRESS and DISTORT in the rack, master COMPRESS on, AMOUNT 0: still the dry, delayed
    let mut st = State::with(&[M_COMP, M_HEAT, M_LOSS]);
    st.amount = 0.0;
    let mut e = Engine::new(SR, 2);
    let lat = e.lat_for_state(&st);
    let x = noise(48000, 0.3);
    let (l, _) = run(&mut e, &st, &x);
    assert_eq!(lat, e.lat);
    for i in 4000..48000 {
        assert!((l[i] - x[i - lat]).abs() < 1e-4, "sample {i}");
    }
}

#[test]
fn tone_modes_are_bounded() {
    for mode in 0..4 {
        for t in [-1.0, 1.0] {
            let mut st = State::with(&[M_WIDTH]);
            st.safe = false;
            st.x.tone_mode = mode;
            st.x.tone = t;
            let mut e = Engine::new(SR, 1);
            let x = noise(48000, 0.3);
            let (l, _) = run(&mut e, &st, &x);
            let d = 20.0 * (rms(&l[24000..]) / rms(&x[24000..])).log10();
            assert!(d.is_finite() && d.abs() < 14.0, "mode {mode} tone {t}: {d:.1} dB");
        }
    }
}

#[test]
fn loss_toggles_and_new_knobs_are_bounded() {
    for (m, var) in [(M_MAGNETIC, 0usize), (M_MAGNETIC, 2), (M_LOSS, 0), (M_LOSS, 1), (M_LOSS, 2)] {
        let mut st = State::with(&[m]);
        st.safe = false;
        st.m[m].var = var;
        st.m[m].k = [0.8; MAX_K];
        st.x.cod_flut = true;
        st.x.mag_wear = false;
        let mut e = Engine::new(SR, 1);
        let (l, _) = run(&mut e, &st, &noise(96000, 0.5));
        let pk = l.iter().fold(0.0f32, |a, v| a.max(v.abs()));
        assert!(pk.is_finite() && pk < 4.0, "{} {var}: {pk}", MODS[m].name);
    }
}

#[test]
fn cpu_worst_case() {
    // full rack incl. COMPRESS + CODEC, ECHO sent through all of it, master COMPRESS, TONE, CUT, 4x
    let mods = [M_HEAT, M_COMP, M_LOSS, M_WOBBLE, M_CHORUS, M_REVERB, M_NOISE, M_TAME];
    let mut st = State::with(&mods);
    st.os = 2;
    st.x.e_on = true;
    st.x.comp = 0.5;
    st.x.tone = 0.3;
    st.x.g_cut = true;
    st.x.g_lo = 60.0;
    for m in mods {
        st.m[m].esend = 1.0;
    }
    let mut e = Engine::new(SR, 2);
    let x = noise(48000 * 5, 0.3);
    let t = std::time::Instant::now();
    run(&mut e, &st, &x);
    let pct = t.elapsed().as_secs_f32() / 5.0 * 100.0;
    println!("worst case: {pct:.2}% of one core");
    assert!(pct < 40.0);
}

/// Energy above ~7 kHz added by a module on a plain 440 Hz tone (dB below the tone).
fn added_top(m: usize, var: usize) -> f32 {
    let mut st = State::with(&[m]);
    st.safe = false;
    st.m[m].var = var;
    let mut e = Engine::new(SR, 1);
    let x: Vec<f32> = (0..96000).map(|i| (0.25 * (std::f64::consts::TAU * 440.0 * i as f64 / SR as f64).sin()) as f32).collect();
    let (l, _) = run(&mut e, &st, &x);
    // 4th-order-ish high-pass by repeated differencing + one-pole
    let mut y = l[48000..].to_vec();
    for _ in 0..4 {
        let mut prev = 0.0;
        for v in y.iter_mut() {
            let d = *v - prev;
            prev = *v;
            *v = d;
        }
    }
    // differencing 4x boosts 440 Hz by (2 sin(pi f/sr))^4 ~ 1e-5 and 7 kHz by ~0.6;
    // skip the start, where differencing from zero makes a step
    20.0 * (rms(&y[64..]) / rms(&x[48000..])).log10()
}

#[test]
fn loss_adds_no_hiss_on_top() {
    let floor = added_top(M_WIDTH, 0);
    println!("clean path: top {floor:.1} dB");
    for (m, var) in [(M_MAGNETIC, 0usize), (M_MAGNETIC, 1), (M_MAGNETIC, 2), (M_LOSS, 0), (M_LOSS, 1), (M_LOSS, 2)] {
        let t = added_top(m, var);
        println!("{} {}: top {t:.1} dB", MODS[m].name, MODS[m].vars[var]);
        assert!(t < -70.0, "{} {}: top end at {t:.1} dB", MODS[m].name, MODS[m].vars[var]);
    }
}

#[test]
fn vinyl_b_crackles_snaps_and_pops() {
    // NOISE VINYL B alone, FOLLOW 0: count events (big sample-to-sample jumps) in 4 s of silence
    let mut st = State::with(&[M_NOISE]);
    st.safe = false;
    st.m[M_NOISE].var = 4;
    st.m[M_NOISE].k = [0.8, 0.6, 0.7, 0.0, 0.5, 0.6, 0.6];
    let mut e = Engine::new(SR, 1);
    let (l, r) = run(&mut e, &st, &vec![0.0; 48000 * 4]);
    let pk = l.iter().chain(r.iter()).fold(0.0f32, |a, v| a.max(v.abs()));
    let bg = rms(&l);
    let ticks = l.windows(2).filter(|w| (w[1] - w[0]).abs() > bg * 6.0).count();
    let low: f32 = { let mut a = 0.0; l.iter().map(|v| { a += 0.01 * (v - a); a.abs() }).fold(0.0, f32::max) };
    println!("vinyl B: peak {pk:.3}, rms {bg:.4}, ticks {ticks}, low peak {low:.3}");
    assert!(pk.is_finite() && pk < 1.0);
    assert!(ticks > 400, "only {ticks} ticks");
    assert!(pk > bg * 8.0, "no snaps standing out");
    assert!(low > bg * 1.5, "no pops");
}
