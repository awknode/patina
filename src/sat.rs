//! The saturation engine: six types x three variants, run oversampled.
//! Everything that makes noise (DEPTH: hiss, hum) rides the signal's own envelope,
//! so silence in is always silence out.

use crate::oversample::{Oversampler, MAX_FACTOR};
use crate::params::*;
use std::f32::consts::PI;

/// Per-block settings, worked out once from the params.
#[derive(Clone, Copy, Debug)]
pub struct Blk {
    pub ty: i32,
    pub var: i32,
    pub color: f32,
    pub depth: f32,
    /// variant gain trim (some tubes have more headroom)
    pub vgain: f32,
    pub bias: f32,
    pub p: f32,
    pub asym: f32,
    pub sag: f32,
    /// iron split frequency (Hz)
    pub iron_f: f32,
    pub iron_k: f32,
    /// tape: pre-emphasis amount (linear), corner, head bump (dB, Hz), roll-off (Hz)
    pub emph: f32,
    pub emph_f: f32,
    pub bump_db: f32,
    pub bump_f: f32,
    pub tape_lp: f32,
    pub wow: f32,
    pub bits: f32,
}

impl Blk {
    pub fn new(ty: i32, var: i32, color: f32, depth: f32) -> Self {
        let mut b = Blk { ty, var, color, depth, vgain: 1.0, bias: 0.0, p: 2.0, asym: 0.0, sag: 0.0, iron_f: 0.0, iron_k: 1.0, emph: 1.0, emph_f: 3000.0, bump_db: 0.0, bump_f: 80.0, tape_lp: 20000.0, wow: 0.0, bits: 16.0 };
        let c = color;
        match ty {
            T_PRE => match var {
                0 => b.bias = c * 0.9,
                1 => {
                    b.vgain = 0.55;
                    b.bias = c * 0.55;
                }
                _ => {
                    // CLASS A: transformer lows + class-A transistor, a touch asymmetric.
                    b.vgain = 0.8;
                    b.iron_f = 110.0;
                    b.iron_k = 1.6;
                    b.p = 2.4;
                    b.asym = 0.08 + c * 0.25;
                }
            },
            T_POWER => {
                b.sag = c;
                let (p, g, a) = match var {
                    0 => (1.6, 1.0, 0.10),
                    1 => (2.6, 0.85, 0.04),
                    _ => (3.6, 0.65, 0.02),
                };
                b.p = p;
                b.vgain = g;
                b.asym = a;
            }
            T_TAPE => {
                let (e, bd, bf, lp, w) = match var {
                    0 => (2.0, 2.0, 70.0, 20000.0, 1.0),
                    1 => (2.8, 3.0, 95.0, 11000.0, 1.6),
                    _ => (2.4, 1.5, 85.0, 7000.0, 3.0),
                };
                // COLOR = tape speed: brighter and less emphasis drive as it goes up.
                b.emph = e;
                b.bump_db = bd;
                b.bump_f = bf * (1.3 - 0.6 * c);
                b.tape_lp = (lp * (0.6 + 0.8 * c)).min(20000.0);
                b.wow = w;
            }
            T_IRON => {
                let (f, k) = match var {
                    0 => (80.0, 1.3),
                    1 => (150.0, 2.0),
                    _ => (110.0, 1.6),
                };
                b.iron_f = f;
                b.iron_k = k * (1.0 + 1.5 * c);
                b.asym = 0.04 + if var == 2 { 0.06 } else { 0.0 };
            }
            T_SOLID => {
                let (p, a) = match var {
                    0 => (2.2, 0.06),
                    1 => (2.6, 0.30),
                    _ => (5.0, 0.02),
                };
                // COLOR = knee: soft round (0) to hard (1).
                b.p = p * (0.55 + 1.1 * c);
                b.asym = a;
            }
            _ => {
                b.bits = 12.0 - c * 10.0;
            }
        }
        b
    }
}

#[derive(Clone, Copy, Default)]
struct OnePole {
    z: f32,
}
impl OnePole {
    #[inline]
    fn lp(&mut self, x: f32, a: f32) -> f32 {
        self.z += a * (x - self.z);
        self.z
    }
}
fn op_coef(f: f32, sr: f32) -> f32 {
    1.0 - (-2.0 * PI * f / sr).exp()
}

/// RBJ peaking / lowpass biquad, direct form I-ish (TDF2).
#[derive(Clone, Copy)]
struct Biq {
    b: [f32; 3],
    a: [f32; 2],
    z: [f32; 2],
}
impl Default for Biq {
    fn default() -> Self {
        Self { b: [1.0, 0.0, 0.0], a: [0.0, 0.0], z: [0.0; 2] }
    }
}
impl Biq {
    fn peak(&mut self, f: f32, q: f32, db: f32, sr: f32) {
        let a = 10f32.powf(db / 40.0);
        let w = 2.0 * PI * (f / sr).min(0.49);
        let (s, c) = w.sin_cos();
        let al = s / (2.0 * q);
        let a0 = 1.0 + al / a;
        self.b = [(1.0 + al * a) / a0, -2.0 * c / a0, (1.0 - al * a) / a0];
        self.a = [-2.0 * c / a0, (1.0 - al / a) / a0];
    }
    fn lowpass(&mut self, f: f32, q: f32, sr: f32) {
        let w = 2.0 * PI * (f / sr).min(0.49);
        let (s, c) = w.sin_cos();
        let al = s / (2.0 * q);
        let a0 = 1.0 + al;
        self.b = [(1.0 - c) / 2.0 / a0, (1.0 - c) / a0, (1.0 - c) / 2.0 / a0];
        self.a = [-2.0 * c / a0, (1.0 - al) / a0];
    }
    fn highpass(&mut self, f: f32, q: f32, sr: f32) {
        let w = 2.0 * PI * (f / sr).min(0.49);
        let (s, c) = w.sin_cos();
        let al = s / (2.0 * q);
        let a0 = 1.0 + al;
        self.b = [(1.0 + c) / 2.0 / a0, -(1.0 + c) / a0, (1.0 + c) / 2.0 / a0];
        self.a = [-2.0 * c / a0, (1.0 - al) / a0];
    }
    #[inline]
    fn tick(&mut self, x: f32) -> f32 {
        let y = self.b[0] * x + self.z[0];
        self.z[0] = self.b[1] * x - self.a[0] * y + self.z[1];
        self.z[1] = self.b[2] * x - self.a[1] * y;
        y
    }
}

/// x / (1 + |x|^p)^(1/p): a soft clipper whose knee gets harder as p grows.
#[inline]
fn knee(x: f32, p: f32) -> f32 {
    x / (1.0 + x.abs().powf(p)).powf(1.0 / p)
}

#[derive(Clone)]
struct Chan {
    os: Oversampler,
    iron_lp: OnePole,
    sag_env: f32,
    pre_hp: OnePole,
    post_hp: OnePole,
    bump: Biq,
    tape_lp: Biq,
    dc_x: f32,
    dc_y: f32,
    tone_lp: OnePole,
    wow: Vec<f32>,
    wpos: usize,
    hum_ph: f32,
    hold: f32,
}
impl Chan {
    fn new(factor: usize) -> Self {
        Self {
            os: Oversampler::new(factor),
            iron_lp: OnePole::default(),
            sag_env: 0.0,
            pre_hp: OnePole::default(),
            post_hp: OnePole::default(),
            bump: Biq::default(),
            tape_lp: Biq::default(),
            dc_x: 0.0,
            dc_y: 0.0,
            tone_lp: OnePole::default(),
            wow: vec![0.0; 2048],
            wpos: 0,
            hum_ph: 0.0,
            hold: 0.0,
        }
    }
}

pub struct Sat {
    sr: f32,
    factor: usize,
    ch: [Chan; 2],
    /// noise
    seed: u32,
    /// signal envelope (for noise riding the signal)
    env: f32,
    env_rel: f32,
    /// slow random drift for tube bias (DEPTH)
    drift: f32,
    drift_t: f32,
    wow_ph: f32,
    flut_ph: f32,
    // auto gain
    ms_in: f32,
    ms_out: f32,
    ag: f32,
    ag_c: f32,
    /// cached coefficients
    last: Option<(i32, i32, u32)>,
    pub ag_db: f32,
}

impl Sat {
    pub fn new(sr: f32, factor: usize) -> Self {
        let mut s = Self { sr, factor, ch: [Chan::new(factor), Chan::new(factor)], seed: 12345, env: 0.0, env_rel: 0.0, drift: 0.0, drift_t: 0.0, wow_ph: 0.0, flut_ph: 0.0, ms_in: 0.0, ms_out: 0.0, ag: 1.0, ag_c: 0.0, last: None, ag_db: 0.0 };
        s.env_rel = (-1.0 / (0.08 * sr)).exp();
        s.ag_c = 1.0 - (-1.0 / (0.35 * sr)).exp();
        s
    }
    pub fn factor(&self) -> usize {
        self.factor
    }
    pub fn clear(&mut self) {
        let f = self.factor;
        self.ch = [Chan::new(f), Chan::new(f)];
        self.env = 0.0;
        self.ms_in = 0.0;
        self.ms_out = 0.0;
        self.ag = 1.0;
        self.last = None;
    }

    fn setup(&mut self, b: &Blk) {
        let key = (b.ty, b.var, (b.color * 200.0) as u32);
        if self.last == Some(key) {
            return;
        }
        self.last = Some(key);
        for c in self.ch.iter_mut() {
            c.bump.peak(b.bump_f, 0.9, b.bump_db, self.sr);
            c.tape_lp.lowpass(b.tape_lp.min(self.sr * 0.45), 0.6, self.sr);
        }
    }

    #[inline]
    fn noise(&mut self) -> f32 {
        self.seed = self.seed.wrapping_mul(1664525).wrapping_add(1013904223);
        (self.seed >> 8) as f32 / 8388608.0 - 1.0
    }

    /// The core curve at the oversampled rate. `u` already has the drive applied.
    #[inline]
    fn shape(c: &mut Chan, u: f32, b: &Blk, bias: f32, iron_a: f32) -> f32 {
        match b.ty {
            T_PRE if b.var < 2 => {
                let tb = bias.tanh();
                (u + bias).tanh() - tb
            }
            T_POWER => {
                let k = u / (1.0 + 1.5 * b.sag * c.sag_env);
                knee(k, b.p) + b.asym * k.tanh().powi(2)
            }
            T_TAPE => u.atan(),
            T_PRE | T_IRON => {
                // Transformer: the lows saturate first and hardest.
                let lo = c.iron_lp.lp(u, iron_a);
                let hi = u - lo;
                let l = (lo * b.iron_k).tanh() / b.iron_k;
                let h = if b.ty == T_PRE { knee(hi + b.asym * hi * hi, b.p) } else { (hi * 0.7).tanh() / 0.7 };
                l + h + b.asym * u.tanh().powi(2)
            }
            T_SOLID => knee(u + b.asym * u * u, b.p),
            _ => match b.var {
                0 => (u * (1.0 + b.color * 3.0)).clamp(-1.0, 1.0),
                1 => {
                    let k = u * (1.0 + b.color * 3.0);
                    // triangle fold: -1..1 back and forth
                    let t = (k + 1.0).rem_euclid(4.0);
                    if t < 2.0 {
                        t - 1.0
                    } else {
                        3.0 - t
                    }
                }
                _ => {
                    let q = 2f32.powf(b.bits) * 0.5;
                    (u.clamp(-1.0, 1.0) * q).round() / q
                }
            },
        }
    }

    /// One stereo sample. `drive` linear gain, `tone_db` tilt, `auto` gain on.
    #[inline]
    pub fn tick(&mut self, l: f32, r: f32, b: &Blk, drive: f32, tone_db: f32, auto: bool) -> (f32, f32) {
        self.setup(b);
        let sr = self.sr;
        let fs_os = sr * self.factor as f32;
        // envelope of the input (noise and hum ride it)
        let pk = l.abs().max(r.abs());
        self.env = if pk > self.env { pk } else { pk + self.env_rel * (self.env - pk) };
        let depth = b.depth;
        // tube bias drift
        let mut bias = b.bias;
        if depth > 0.0 && (b.ty == T_PRE || b.ty == T_POWER) {
            self.drift_t += 1.0 / sr;
            if self.drift_t > 0.25 {
                self.drift_t = 0.0;
                self.drift = 0.8 * self.drift + 0.2 * self.noise();
            }
            bias += depth * 0.25 * self.drift;
        }
        // tape wow + flutter: modulated delay (0 .. a couple of ms), before the drive
        let wow_on = b.ty == T_TAPE && depth > 0.0;
        let (mut wd, mut fd) = (0.0, 0.0);
        if wow_on {
            self.wow_ph = (self.wow_ph + 0.7 / sr).fract();
            self.flut_ph = (self.flut_ph + 6.5 / sr).fract();
            wd = depth * b.wow * 0.0006 * sr * (1.0 - (2.0 * PI * self.wow_ph).cos());
            fd = depth * 0.00006 * sr * (1.0 - (2.0 * PI * self.flut_ph).cos());
        }
        let iron_a = op_coef(b.iron_f.max(10.0), fs_os);
        let emph_a = op_coef(b.emph_f, sr);
        let deemph_a = op_coef(b.emph_f / b.emph, sr);
        let tone_a = op_coef(1000.0, sr);
        let (tl, th) = (10f32.powf(-tone_db / 40.0), 10f32.powf(tone_db / 40.0));
        let sag_a = 1.0 - (-1.0 / (0.03 * sr)).exp();
        let mut out = [0.0f32; 2];
        let ins = [l, r];
        let factor = self.factor;
        for k in 0..2 {
            let mut x = ins[k];
            let c = &mut self.ch[k];
            if wow_on {
                let n = c.wow.len();
                c.wow[c.wpos] = x;
                let d = wd + fd;
                let di = d.floor() as usize;
                let fr = d - di as f32;
                let i0 = (c.wpos + n - di) % n;
                let i1 = (c.wpos + n - di - 1) % n;
                x = c.wow[i0] * (1.0 - fr) + c.wow[i1] * fr;
                c.wpos = (c.wpos + 1) % n;
            }
            if b.ty == T_TAPE {
                // pre-emphasis: highs hit the tape harder
                let hp = x - c.pre_hp.lp(x, emph_a);
                x += (b.emph - 1.0) * hp;
            }
            let u_in = x * drive * b.vgain;
            c.sag_env += sag_a * (u_in.abs() - c.sag_env);
            let mut buf = [0.0f32; MAX_FACTOR];
            c.os.up(u_in, &mut buf);
            for v in buf.iter_mut().take(factor) {
                *v = Self::shape(c, *v, b, bias, iron_a);
            }
            let mut y = c.os.down(&mut buf);
            if b.ty == T_TAPE {
                let hp = y - c.post_hp.lp(y, deemph_a);
                y += (1.0 / b.emph - 1.0) * hp;
                y = c.bump.tick(y);
                y = c.tape_lp.tick(y);
            }
            // DC blocker (asymmetric curves make DC)
            let dc = y - c.dc_x + 0.9995 * c.dc_y;
            c.dc_x = y;
            c.dc_y = dc;
            y = dc;
            // TONE: tilt around 1 kHz
            let lo = c.tone_lp.lp(y, tone_a);
            y = lo * tl + (y - lo) * th;
            out[k] = y;
        }
        // DEPTH noise / hum, riding the input envelope (never on its own)
        if depth > 0.0 {
            let e = self.env.min(1.0);
            let hiss = depth * e * match b.ty {
                T_TAPE => 10f32.powf(-46.0 / 20.0),
                T_DIGITAL => 10f32.powf(-50.0 / 20.0),
                _ => 10f32.powf(-58.0 / 20.0),
            };
            for k in 0..2 {
                let n = self.noise();
                let c = &mut self.ch[k];
                // hiss is bright: first difference of white noise
                let h = n - c.hold;
                c.hold = n;
                out[k] += hiss * h * 0.5;
                if b.ty == T_PRE || b.ty == T_POWER {
                    c.hum_ph = (c.hum_ph + 50.0 / sr).fract();
                    let ph = 2.0 * PI * c.hum_ph;
                    let hum = ph.sin() + 0.5 * (2.0 * ph).sin() + 0.25 * (3.0 * ph).sin();
                    out[k] += depth * e * 10f32.powf(-50.0 / 20.0) * hum;
                }
            }
        }
        // AUTO GAIN: match the wet's loudness to the input's
        self.ms_in += self.ag_c * (0.5 * (l * l + r * r) - self.ms_in);
        self.ms_out += self.ag_c * (0.5 * (out[0] * out[0] + out[1] * out[1]) - self.ms_out);
        let target = if auto {
            if self.ms_in > 1e-8 && self.ms_out > 1e-10 {
                (self.ms_in / self.ms_out).sqrt().clamp(0.03, 4.0)
            } else {
                self.ag
            }
        } else {
            1.0
        };
        self.ag += 0.0005 * (target - self.ag);
        self.ag_db = 20.0 * self.ag.log10();
        (out[0] * self.ag, out[1] * self.ag)
    }
}

/// The transfer curve for the screen: x in -1..1 at the current settings,
/// normalised so the largest output is 1.
pub fn curve_points(ty: i32, var: i32, color: f32, drive_db: f32, n: usize) -> Vec<(f32, f32)> {
    let b = Blk::new(ty, var, color, 0.0);
    let g = 10f32.powf(drive_db / 20.0) * b.vgain;
    let mut c = Chan::new(1);
    let iron_a = 1.0; // static curve: no lows split
    let mut pts: Vec<(f32, f32)> = (0..=n)
        .map(|i| {
            let x = -1.0 + 2.0 * i as f32 / n as f32;
            c.iron_lp.z = 0.0;
            let u = x * g;
            let y = if b.ty == T_TAPE { u.atan() } else { Sat::shape(&mut c, u, &b, b.bias, if b.iron_f > 0.0 { iron_a } else { 0.0 }) };
            (x, y)
        })
        .collect();
    // remove the bias offset so the curve passes through the middle
    let y0 = {
        c.iron_lp.z = 0.0;
        Sat::shape(&mut c, 0.0, &b, b.bias, if b.iron_f > 0.0 { iron_a } else { 0.0 })
    };
    let m = pts.iter().map(|p| (p.1 - y0).abs()).fold(1e-6, f32::max);
    for p in pts.iter_mut() {
        p.1 = (p.1 - y0) / m;
    }
    pts
}

/// Harmonics 1..=8 of a sine through the curve (relative amplitude, H1 = 1).
pub fn harmonics(ty: i32, var: i32, color: f32, drive_db: f32) -> [f32; 8] {
    let pts = curve_points(ty, var, color, drive_db, 512);
    let f = |x: f32| {
        let t = (x + 1.0) * 0.5 * 512.0;
        let i = (t.floor() as usize).min(511);
        let fr = t - i as f32;
        pts[i].1 * (1.0 - fr) + pts[i + 1].1 * fr
    };
    let n = 256;
    let ys: Vec<f32> = (0..n).map(|k| f(0.85 * (2.0 * PI * k as f32 / n as f32).sin())).collect();
    let mut h = [0.0f32; 8];
    for (k, hk) in h.iter_mut().enumerate() {
        let (mut re, mut im) = (0.0, 0.0);
        for (i, y) in ys.iter().enumerate() {
            let a = 2.0 * PI * (k + 1) as f32 * i as f32 / n as f32;
            re += y * a.cos();
            im += y * a.sin();
        }
        *hk = (re * re + im * im).sqrt() * 2.0 / n as f32;
    }
    let h1 = h[0].max(1e-9);
    h.map(|v| v / h1)
}

/// CLEAN LOWS / CLEAN HIGHS: a Linkwitz-Riley crossover (4th order) splits off the
/// lows and highs so only the middle gets driven. low + mid + high is flat in level
/// (an all-pass), so with no drive the sound is unchanged apart from phase; the dry
/// goes through a matching crossover so MIX still lines up.
#[derive(Clone, Copy, Default)]
struct Lr4 {
    a: Biq,
    b: Biq,
}
impl Lr4 {
    fn lp(&mut self, f: f32, sr: f32) {
        self.a.lowpass(f, std::f32::consts::FRAC_1_SQRT_2, sr);
        self.b.lowpass(f, std::f32::consts::FRAC_1_SQRT_2, sr);
    }
    fn hp(&mut self, f: f32, sr: f32) {
        self.a.highpass(f, std::f32::consts::FRAC_1_SQRT_2, sr);
        self.b.highpass(f, std::f32::consts::FRAC_1_SQRT_2, sr);
    }
    #[inline]
    fn tick(&mut self, x: f32) -> f32 {
        let y = self.a.tick(x);
        self.b.tick(y)
    }
    fn clear(&mut self) {
        self.a.z = [0.0; 2];
        self.b.z = [0.0; 2];
    }
}

#[derive(Clone, Copy, Default)]
struct SplitCh {
    lo_lp: Lr4,
    lo_hp: Lr4,
    hi_lp: Lr4,
    hi_hp: Lr4,
    ap_lp: Lr4,
    ap_hp: Lr4,
}

pub struct Split {
    sr: f32,
    lo_f: f32,
    hi_f: f32,
    ch: [SplitCh; 2],
}
impl Split {
    pub fn new(sr: f32) -> Self {
        Self { sr, lo_f: -1.0, hi_f: -1.0, ch: [SplitCh::default(); 2] }
    }
    pub fn lo_on(f: f32) -> bool {
        f > 20.5
    }
    pub fn hi_on(f: f32) -> bool {
        f < 19900.0
    }
    pub fn active(&self) -> bool {
        Self::lo_on(self.lo_f) || Self::hi_on(self.hi_f)
    }
    pub fn set(&mut self, lo: f32, hi: f32) {
        let sr = self.sr;
        if (lo - self.lo_f).abs() > 0.01 {
            self.lo_f = lo;
            for c in self.ch.iter_mut() {
                c.lo_lp.lp(lo, sr);
                c.lo_hp.hp(lo, sr);
            }
        }
        if (hi - self.hi_f).abs() > 0.01 {
            self.hi_f = hi;
            let h = hi.min(sr * 0.45);
            for c in self.ch.iter_mut() {
                c.hi_lp.lp(h, sr);
                c.hi_hp.hp(h, sr);
                c.ap_lp.lp(h, sr);
                c.ap_hp.hp(h, sr);
            }
        }
    }
    pub fn clear(&mut self) {
        for c in self.ch.iter_mut() {
            for f in [&mut c.lo_lp, &mut c.lo_hp, &mut c.hi_lp, &mut c.hi_hp, &mut c.ap_lp, &mut c.ap_hp] {
                f.clear();
            }
        }
    }
    /// Returns (mid, clean) for one channel. mid + clean = the input, all-passed.
    #[inline]
    pub fn tick(&mut self, ch: usize, x: f32) -> (f32, f32) {
        let (lo_on, hi_on) = (Self::lo_on(self.lo_f), Self::hi_on(self.hi_f));
        let c = &mut self.ch[ch];
        let (mut low, rest) = if lo_on { (c.lo_lp.tick(x), c.lo_hp.tick(x)) } else { (0.0, x) };
        let (mid, high) = if hi_on { (c.hi_lp.tick(rest), c.hi_hp.tick(rest)) } else { (rest, 0.0) };
        if lo_on && hi_on {
            // give the lows the same phase turn the high split put on the rest
            low = c.ap_lp.tick(low) + c.ap_hp.tick(low);
        }
        (mid, low + high)
    }
}
