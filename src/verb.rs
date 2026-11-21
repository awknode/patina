//! SPACE: one 8-line feedback delay network voiced as plate / room / chamber /
//! hall / cathedral, plus a separate dispersive spring tank.

use crate::dsp::*;
use crate::params::*;

const BASE: [[f32; 8]; 4] = [
    // plate: short, dense
    [7.1, 8.9, 10.7, 12.3, 14.9, 16.7, 18.1, 20.3],
    // room
    [5.3, 6.7, 7.9, 9.7, 11.3, 12.9, 14.3, 16.1],
    // hall
    [23.1, 27.7, 31.3, 36.7, 41.9, 47.3, 53.1, 59.9],
    // early reflections (ms) for rooms
    [3.1, 5.3, 7.7, 10.1, 13.9, 17.3, 21.1, 26.3],
];
const DIFF_MS: [f32; 4] = [1.42, 2.13, 3.31, 4.99];
const MOD_HZ: [f32; 8] = [0.13, 0.21, 0.29, 0.37, 0.47, 0.59, 0.71, 0.89];

pub struct Reverb {
    sr: f32,
    var: usize,
    lines: Vec<Line>,
    len: [f32; 8],
    len_t: [f32; 8],
    g: [f32; 8],
    damp: [Lp1; 8],
    damp_a: f32,
    diff: [[Allpass; 4]; 2],
    diff_g: f32,
    pre: [Line; 2],
    pre_n: f32,
    pre_sm: f32,
    er: bool,
    er_n: [usize; 8],
    mod_ph: [f32; 8],
    mod_d: f32,
    hp: [Biq; 2],
    duck: Env,
    duck_k: f32,
    spring: Spring,
}

impl Reverb {
    pub fn new(sr: f32) -> Self {
        let max_line = (0.06 * 1.6 * 2.0 * 1.7 * sr) as usize + 64;
        let mut s = Self {
            sr,
            var: 99,
            lines: (0..8).map(|_| Line::new(max_line)).collect(),
            len: [1.0; 8],
            len_t: [1.0; 8],
            g: [0.0; 8],
            damp: [Lp1::default(); 8],
            damp_a: 1.0,
            diff: std::array::from_fn(|_| std::array::from_fn(|_| Allpass::new((0.006 * 2.0 * sr) as usize + 8))),
            diff_g: 0.6,
            pre: [Line::new((0.26 * sr) as usize + (0.03 * 2.5 * sr) as usize), Line::new((0.26 * sr) as usize + (0.03 * 2.5 * sr) as usize)],
            pre_n: 0.0,
            pre_sm: 0.0,
            er: false,
            er_n: [1; 8],
            mod_ph: [0.0, 0.3, 0.6, 0.1, 0.8, 0.45, 0.25, 0.7],
            mod_d: 0.0,
            hp: [Biq::default(); 2],
            duck: Env::new(0.01, 0.25, sr),
            duck_k: 0.0,
            spring: Spring::new(sr),
        };
        for h in s.hp.iter_mut() {
            h.highpass(60.0, 0.7, sr);
        }
        s
    }
    pub fn reset(&mut self) {
        self.lines.iter_mut().for_each(|l| l.clear());
        self.damp = [Lp1::default(); 8];
        self.diff.iter_mut().flatten().for_each(|a| a.clear());
        self.pre.iter_mut().for_each(|l| l.clear());
        self.hp.iter_mut().for_each(|h| h.clear());
        self.duck.e = 0.0;
        self.spring.reset();
        self.var = 99;
    }

    /// knobs: DECAY, SIZE, PRE-DELAY, DAMP, MOD, DUCK (SPRING: SIZE = TENSION, MOD = DRIP)
    pub fn control(&mut self, k: &[f32; MAX_K], var: usize) {
        let sr = self.sr;
        let first = self.var != var;
        self.var = var;
        let rt = rt60(var, k[0]).max(0.05);
        self.pre_n = pre_ms(k[2]) * 0.001 * sr;
        if first {
            self.pre_sm = self.pre_n;
        }
        self.duck_k = k[5] * 10.0;
        let damp_f = 20000.0 * 0.05f32.powf(k[3]);
        if var == V_SPRING {
            self.spring.control(k, rt, damp_f);
            return;
        }
        let (set, scale, dg, er, mod_ms, bright) = match var {
            V_PLATE => (0, 0.6 + 1.4 * k[1], 0.72, false, 0.35, 1.0),
            V_GOLD => (0, 0.7 + 1.5 * k[1], 0.72, false, 0.45, 0.55),
            V_ROOM => (1, 0.5 + 1.0 * k[1], 0.55, true, 0.25, 0.9),
            V_CHAMBER => (1, 0.8 + 1.6 * k[1], 0.6, true, 0.35, 0.75),
            V_HALL => (2, 0.6 + 1.0 * k[1], 0.5, false, 0.8, 0.85),
            // short, dense early sound, barely a tail
            V_AMBIENCE => (1, 0.35 + 0.5 * k[1], 0.62, true, 0.2, 0.95),
            // huge and dark
            V_VAULT => (2, 1.3 + 1.6 * k[1], 0.55, false, 1.3, 0.45),
            // a cheap old plate: dull and seasick
            V_LOFI => (0, 0.6 + 1.0 * k[1], 0.7, false, 1.6, 0.28),
            _ => (2, 1.0 + 1.7 * k[1], 0.5, false, 1.1, 0.7),
        };
        self.diff_g = dg;
        self.er = er;
        self.damp_a = op_a((damp_f * bright).max(500.0), sr);
        self.mod_d = k[4] * mod_ms * 0.001 * sr;
        for i in 0..8 {
            self.len_t[i] = (BASE[set][i] * scale * 0.001 * sr).min(self.lines[i].cap() as f32 - self.mod_d - 8.0);
            if first {
                self.len[i] = self.len_t[i];
            }
            self.g[i] = 10f32.powf(-3.0 * self.len_t[i] / (sr * rt));
            self.er_n[i] = ((BASE[3][i] * scale * 0.001 * sr) as usize).min(self.pre[0].cap() - (0.26 * sr) as usize);
        }
        for (j, ms) in DIFF_MS.iter().enumerate() {
            let n = (ms * scale.min(2.0) * 0.001 * sr) as usize + 1;
            self.diff[0][j].n = n;
            self.diff[1][j].n = n + 7 + j * 3;
        }
    }

    pub fn tick(&mut self, l: f32, r: f32) -> (f32, f32) {
        let duck = 1.0 / (1.0 + self.duck_k * self.duck.tick(0.5 * (l + r)));
        // pre-delay (smoothed so turning it never clicks hard)
        self.pre_sm += 0.0005 * (self.pre_n - self.pre_sm);
        self.pre[0].push(self.hp[0].tick(l));
        self.pre[1].push(self.hp[1].tick(r));
        let (pl, pr) = (self.pre[0].frac(self.pre_sm), self.pre[1].frac(self.pre_sm));
        if self.var == V_SPRING {
            let (a, b) = self.spring.tick(pl, pr);
            return (a * duck, b * duck);
        }
        let mut xl = pl;
        let mut xr = pr;
        for j in 0..4 {
            xl = self.diff[0][j].tick(xl, self.diff_g);
            xr = self.diff[1][j].tick(xr, self.diff_g);
        }
        let mut y = [0.0f32; 8];
        let inc = 1.0 / self.sr;
        for i in 0..8 {
            self.len[i] += 0.0002 * (self.len_t[i] - self.len[i]);
            self.mod_ph[i] = (self.mod_ph[i] + MOD_HZ[i] * inc).fract();
            let d = self.len[i] + self.mod_d * (1.0 + (TAU * self.mod_ph[i]).sin());
            y[i] = self.lines[i].frac(d);
        }
        let mut z = [0.0f32; 8];
        for i in 0..8 {
            z[i] = self.damp[i].tick(y[i], self.damp_a) * self.g[i];
        }
        hadamard8(&mut z);
        for i in 0..8 {
            let inj = if i % 2 == 0 { xl } else { xr };
            self.lines[i].push(z[i] + 0.5 * inj);
        }
        let mut ol = (y[0] - y[2] + y[4] - y[6]) * 0.6;
        let mut or = (y[1] - y[3] + y[5] - y[7]) * 0.6;
        if self.er {
            let mut g = 0.55;
            for i in 0..8 {
                let d = self.pre_sm as usize + self.er_n[i];
                if i % 2 == 0 {
                    ol += g * self.pre[0].at(d);
                    or += 0.4 * g * self.pre[1].at(d + 13);
                } else {
                    or += g * self.pre[1].at(d);
                    ol += 0.4 * g * self.pre[0].at(d + 11);
                }
                g *= 0.8;
            }
        }
        (ol * duck, or * duck)
    }
}

/// Spring tank: delay loops with a chain of first-order allpasses inside, which
/// smear each echo into the falling "drip" chirp springs are known for.
pub struct Spring {
    sr: f32,
    lines: [Line; 3],
    n: [usize; 3],
    ap: [[(f32, f32); 12]; 3],
    a: f32,
    g: [f32; 3],
    lp: [Lp1; 3],
    lp_a: f32,
    in_bp: [Biq; 4],
    out_lp: [Biq; 2],
    pub springs: usize,
}
impl Spring {
    pub fn new(sr: f32) -> Self {
        let m = (0.11 * 1.5 * sr) as usize + 8;
        let mut s = Self { sr, lines: [Line::new(m), Line::new(m), Line::new(m)], n: [1; 3], ap: [[(0.0, 0.0); 12]; 3], a: 0.6, g: [0.0; 3], lp: [Lp1::default(); 3], lp_a: 0.5, in_bp: [Biq::default(); 4], out_lp: [Biq::default(); 2], springs: 3 };
        s.in_bp[0].highpass(150.0, 0.7, sr);
        s.in_bp[1].highpass(150.0, 0.7, sr);
        s.in_bp[2].lowpass(4500.0, 0.7, sr);
        s.in_bp[3].lowpass(4500.0, 0.7, sr);
        s.out_lp[0].lowpass(5000.0, 0.7, sr);
        s.out_lp[1].lowpass(5000.0, 0.7, sr);
        s
    }
    pub fn reset(&mut self) {
        self.lines.iter_mut().for_each(|l| l.clear());
        self.ap = [[(0.0, 0.0); 12]; 3];
        self.lp = [Lp1::default(); 3];
        self.in_bp.iter_mut().for_each(|b| b.clear());
        self.out_lp.iter_mut().for_each(|b| b.clear());
    }
    fn control(&mut self, k: &[f32; MAX_K], rt: f32, damp_f: f32) {
        let sr = self.sr;
        let base = (0.07 - 0.045 * k[1]) * sr;
        for (i, m) in [1.0, 1.23, 1.47].iter().enumerate() {
            self.n[i] = ((base * m) as usize).clamp(8, self.lines[i].cap() - 2);
            self.g[i] = 10f32.powf(-3.0 * self.n[i] as f32 / (sr * rt)).min(0.97);
        }
        self.a = 0.45 + 0.35 * k[4];
        self.lp_a = op_a(damp_f.clamp(800.0, 9000.0), sr);
    }
    fn tick(&mut self, l: f32, r: f32) -> (f32, f32) {
        let xl = self.in_bp[0].tick(l);
        let xl = self.in_bp[2].tick(xl);
        let xr = self.in_bp[1].tick(r);
        let xr = self.in_bp[3].tick(xr);
        let mut out = [0.0f32; 3];
        let a = self.a;
        for s in 0..3 {
            let mut v = self.lines[s].at(self.n[s] - 1);
            for st in self.ap[s].iter_mut() {
                // first-order allpass: (a + z^-1) / (1 + a z^-1)
                let y = a * (v - st.1) + st.0;
                st.0 = v;
                st.1 = y;
                v = y;
            }
            v = self.lp[s].tick(v, self.lp_a);
            out[s] = v;
            let inj = match s {
                0 => xl,
                1 => xr,
                _ => 0.5 * (xl + xr),
            };
            self.lines[s].push(inj + self.g[s] * v);
        }
        let third = if self.springs >= 3 { 0.7 * out[2] } else { 0.0 };
        (self.out_lp[0].tick(out[0] + third) * 0.9, self.out_lp[1].tick(out[1] + third) * 0.9)
    }
}
