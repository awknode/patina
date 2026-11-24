//! The Patina modules (except REVERB, in verb.rs; HEAT and TAME reuse KILN's engines).
//! Each one: `control()` once per 32-sample step with smoothed knob values,
//! `tick()` per sample returning the fully-wet signal (MIX is applied by the rack).

use crate::dsp::*;
use crate::params::*;

/// Timing for this sample.
#[derive(Clone, Copy, Debug)]
pub struct Ctx {
    pub sr: f32,
    /// samples per quarter note
    pub spb: f32,
    /// position in quarter notes
    pub beat: f64,
    /// running sample counter
    pub n: u64,
}

// ------------------------------------------------------------------ NOISE

pub struct Noise {
    sr: f32,
    rng: Rng,
    pink: [[f32; 3]; 2],
    brown: [f32; 2],
    /// surface noise band (high-pass + low-pass)
    surf: [[Biq; 2]; 2],
    /// crackle: impulses into a resonator whose pitch is re-rolled per crackle
    cr_svf: [Svf; 2],
    cr_imp: [f32; 2],
    /// snaps: louder, sharper, ringing a little
    sn_svf: [Svf; 2],
    sn_imp: [f32; 2],
    /// pops: low thumps
    pop_env: [f32; 2],
    pop_ph: [f32; 2],
    pop_f: f32,
    rumble: [Lp1; 2],
    tone: [[Lp1; 2]; 2],
    range: [[Biq; 2]; 2],
    range_on: (bool, bool),
    rng_lo: f32,
    rng_hi: f32,
    env: Env,
    hum_ph: f32,
    /// record rotation (0..1) and the spot on it where the scratch sits
    rot: f32,
    scratch_at: f32,
    var: usize,
    level: f32,
    dust: f32,
    tone_a: f32,
    follow: f32,
    /// GATE: open while music plays, held a moment, then fades out
    gate_g: f32,
    hold: f32,
    width: f32,
    snap: f32,
    pops: f32,
}

/// Per flavor: (surface, crackle rate /s at DUST 1, snap rate /s at SNAP 1, pop rate /s at POPS 1, rpm, scratch)
const NOISE_V: [(f32, f32, f32, f32, f32, bool); 7] = [
    // VINYL A: clean-ish record, fine crackle
    (0.45, 900.0, 4.0, 0.8, 33.33, false),
    // TAPE: hiss only (warm), a rare tick
    (0.0, 0.0, 0.5, 0.0, 0.0, false),
    // HUM
    (0.0, 0.0, 0.0, 0.0, 0.0, false),
    // ROOM
    (0.0, 0.0, 0.0, 0.0, 0.0, false),
    // VINYL B: snap, crackle, pop
    (0.35, 3200.0, 14.0, 3.0, 33.33, true),
    // SHELLAC: a 78, loud surface roar, fast rotation
    (1.0, 1600.0, 8.0, 1.5, 78.0, true),
    // DUST: just the crackle, snaps and pops, no surface noise
    (0.0, 1800.0, 10.0, 2.0, 33.33, false),
];

impl Noise {
    pub fn new(sr: f32) -> Self {
        let mut n = Self {
            sr,
            rng: Rng(0x9e37_79b9),
            pink: [[0.0; 3]; 2],
            brown: [0.0; 2],
            surf: [[Biq::default(); 2]; 2],
            cr_svf: [Svf::default(); 2],
            cr_imp: [0.0; 2],
            sn_svf: [Svf::default(); 2],
            sn_imp: [0.0; 2],
            pop_env: [0.0; 2],
            pop_ph: [0.0; 2],
            pop_f: 400.0,
            rumble: [Lp1::default(); 2],
            tone: [[Lp1::default(); 2]; 2],
            range: [[Biq::default(); 2]; 2],
            range_on: (false, false),
            rng_lo: -1.0,
            rng_hi: -1.0,
            env: Env::new(0.005, 0.3, sr),
            hum_ph: 0.0,
            rot: 0.0,
            scratch_at: 0.37,
            var: 0,
            level: 0.0,
            dust: 0.0,
            tone_a: 1.0,
            follow: 0.5,
            gate_g: 0.0,
            hold: 0.0,
            width: 1.0,
            snap: 0.0,
            pops: 0.0,
        };
        for ch in 0..2 {
            n.surf[ch][0].highpass(300.0, 0.6, sr);
            n.surf[ch][1].lowpass(6000.0, 0.6, sr);
            n.cr_svf[ch].set(3000.0, 0.9, sr);
            n.sn_svf[ch].set(2600.0, 2.5, sr);
        }
        n
    }
    pub fn reset(&mut self) {
        self.pink = [[0.0; 3]; 2];
        self.brown = [0.0; 2];
        self.cr_imp = [0.0; 2];
        self.sn_imp = [0.0; 2];
        self.pop_env = [0.0; 2];
        self.env.e = 0.0;
        for ch in 0..2 {
            self.cr_svf[ch].clear();
            self.sn_svf[ch].clear();
            self.surf[ch].iter_mut().for_each(|b| b.clear());
        }
        for c in self.range.iter_mut().flatten() {
            c.clear();
        }
    }
    pub fn set_range(&mut self, lo: f32, hi: f32) {
        if (lo - self.rng_lo).abs() < 0.5 && (hi - self.rng_hi).abs() < 0.5 {
            return;
        }
        self.rng_lo = lo;
        self.rng_hi = hi;
        self.range_on = (lo > 20.5, hi < 19900.0);
        for ch in 0..2 {
            self.range[ch][0].highpass(lo, 0.7, self.sr);
            self.range[ch][1].lowpass(hi, 0.7, self.sr);
        }
    }
    pub fn control(&mut self, k: &[f32; MAX_K], var: usize) {
        self.var = var.min(NOISE_V.len() - 1);
        self.level = db(noise_level_db(k[0]));
        self.dust = k[1];
        self.tone_a = op_a(800.0 * 20f32.powf(k[2]), self.sr);
        self.follow = k[3];
        self.width = k[4];
        self.snap = k[5];
        self.pops = k[6];
    }
    /// Which side(s) an event lands on: mostly centred, WIDTH spreads it.
    #[inline]
    fn sides(&mut self) -> [f32; 2] {
        let pan = self.rng.b() * self.width;
        [(1.0 - pan).min(1.0), (1.0 + pan).min(1.0)]
    }
    pub fn tick(&mut self, l: f32, r: f32) -> (f32, f32) {
        let e = self.env.tick(l.abs().max(r.abs()));
        let sr = self.sr;
        // FOLLOW: 0 = ALWAYS on. Up to 50% = GATE: a steady level while anything plays (no pumping),
        // fading out after 250 ms of silence (50% = all the way out). Above 50% = RIDE: it also
        // rises and falls with the music's level (the old behaviour at 100%).
        // gate on the input itself (-80 dB), not the slow level follower: the hiss must not hang on
        if l.abs().max(r.abs()) > 1e-4 {
            self.hold = 0.25 * sr;
        } else {
            self.hold -= 1.0;
        }
        let tgt = if self.hold > 0.0 { 1.0 } else { 0.0 };
        let k = if tgt > self.gate_g { 1.0 / (0.03 * sr) } else { 1.0 / (0.12 * sr) };
        self.gate_g += k * (tgt - self.gate_g);
        if tgt == 0.0 && self.gate_g < 1e-5 {
            self.gate_g = 0.0;
        }
        let fk = self.follow;
        let (depth, ride) = if fk <= 0.02 { (0.0, 0.0) } else if fk <= 0.5 { (fk / 0.5, 0.0) } else { (1.0, (fk - 0.5) / 0.5) };
        let g = self.level * (1.0 - depth * (1.0 - self.gate_g)) * ((1.0 - ride) + ride * e / (e + 0.03));
        let (surface, cr_rate, sn_rate, pop_rate, rpm, scratch) = NOISE_V[self.var];
        // record rotation: crackle gets denser over a worn arc, and the scratch ticks once a turn
        let mut wear_arc = 1.0;
        if rpm > 0.0 {
            let prev = self.rot;
            self.rot = (self.rot + rpm / 60.0 / sr).fract();
            let d = ((self.rot - 0.6).abs()).min(1.0 - (self.rot - 0.6).abs());
            wear_arc = 0.7 + 0.9 * (-d * d * 40.0).exp();
            if scratch && prev < self.scratch_at && self.rot >= self.scratch_at {
                let a = (0.35 + 0.5 * self.snap) * 1.2;
                let s = self.sides();
                for ch in 0..2 {
                    self.sn_imp[ch] += a * s[ch];
                }
            }
        }
        // crackle: lots of tiny ticks, a few big ones (power law), pitch rolled per tick
        let cr = self.dust * self.dust * cr_rate * wear_arc;
        if cr > 0.0 && self.rng.u() < cr / sr {
            let a = 0.08 + 0.9 * self.rng.u().powi(6);
            let a = if self.rng.u() < 0.8 { a } else { -a };
            let f = 1500.0 + 5000.0 * self.rng.u();
            let s = self.sides();
            for ch in 0..2 {
                self.cr_svf[ch].set(f, 0.8, sr);
                self.cr_imp[ch] += a * s[ch];
            }
        }
        // snaps: sharp and loud, with a short ring
        let sn = self.snap * self.snap * sn_rate * wear_arc;
        if sn > 0.0 && self.rng.u() < sn / sr {
            let a = 0.3 + 0.7 * self.rng.u().powi(2);
            let f = 1800.0 + 2500.0 * self.rng.u();
            let s = self.sides();
            for ch in 0..2 {
                self.sn_svf[ch].set(f, 2.0 + 2.0 * self.rng.u(), sr);
                self.sn_imp[ch] += a * s[ch];
            }
        }
        // pops: low thumps
        let pr = self.pops * self.pops * pop_rate;
        if pr > 0.0 && self.rng.u() < pr / sr {
            let a = 0.4 + 0.6 * self.rng.u();
            self.pop_f = 180.0 + 500.0 * self.rng.u();
            let s = self.sides();
            for ch in 0..2 {
                self.pop_env[ch] = (self.pop_env[ch] + a * s[ch]).min(1.5);
                self.pop_ph[ch] = 0.0;
            }
        }
        if self.var == 2 {
            self.hum_ph = (self.hum_ph + 50.0 / sr).fract();
        }
        let ph = TAU * self.hum_ph;
        let pop_k = (-1.0 / (0.012 * sr)).exp();
        let mut n = [0.0f32; 2];
        for ch in 0..2 {
            let w = self.rng.b();
            let p = &mut self.pink[ch];
            p[0] = 0.99765 * p[0] + w * 0.099_046;
            p[1] = 0.963 * p[1] + w * 0.296_516_4;
            p[2] = 0.57 * p[2] + w * 1.052_691_3;
            let pink = (p[0] + p[1] + p[2] + w * 0.1848) * 0.11;
            self.brown[ch] = (self.brown[ch] * 0.998 + w * 0.04).clamp(-1.0, 1.0);
            // event voices
            let ci = self.cr_imp[ch];
            self.cr_imp[ch] = 0.0;
            let (_, cbp, chp) = self.cr_svf[ch].tick(ci * 6.0);
            let crackle = cbp * 0.7 + chp * 0.5;
            let si = self.sn_imp[ch];
            self.sn_imp[ch] = 0.0;
            let (_, sbp, shp) = self.sn_svf[ch].tick(si * 4.0);
            let snap = sbp * 0.8 + shp * 0.6 + si * 0.6;
            let pop = if self.pop_env[ch] > 1e-4 {
                self.pop_ph[ch] += self.pop_f / sr;
                let v = (TAU * self.pop_ph[ch]).sin() * self.pop_env[ch];
                self.pop_env[ch] *= pop_k;
                v * 0.9
            } else {
                0.0
            };
            let events = crackle + snap + pop;
            let v = match self.var {
                // TAPE: warm hiss (pink through the TONE filters, no raw white top end)
                1 => 0.55 * pink + 0.2 * events,
                2 => {
                    let buzz = self.dust * 0.5 * ((ph * 5.0).sin() * 0.5 + (ph * 7.0).sin() * 0.35 + (ph * 9.0).sin() * 0.25);
                    (ph.sin() + 0.5 * (2.0 * ph).sin() + 0.3 * (3.0 * ph).sin() + buzz) * 0.5 + 0.03 * pink
                }
                3 => 1.2 * self.brown[ch] + 0.35 * pink,
                _ => {
                    // records: surface noise (banded, swelling with the rotation), rumble, events
                    let s0 = self.surf[ch][0].tick(pink);
                    let sf = self.surf[ch][1].tick(s0) * surface * (0.6 + 0.4 * wear_arc);
                    let rum = 1.2 * self.rumble[ch].tick(w, 0.0025) * surface;
                    sf + rum + events
                }
            };
            let t = &mut self.tone[ch];
            let t0 = t[0].tick(v, self.tone_a);
            n[ch] = t[1].tick(t0, self.tone_a);
        }
        let m = 0.5 * (n[0] + n[1]);
        let w = if self.var == 2 { self.width * 0.2 } else { self.width };
        let mut nl = m + (n[0] - m) * w;
        let mut nr = m + (n[1] - m) * w;
        if self.range_on.0 {
            nl = self.range[0][0].tick(nl);
            nr = self.range[1][0].tick(nr);
        }
        if self.range_on.1 {
            nl = self.range[0][1].tick(nl);
            nr = self.range[1][1].tick(nr);
        }
        (l + g * soft(nl, 1.0), r + g * soft(nr, 1.0))
    }
}

// ------------------------------------------------------------------ CRUSH

pub struct Crush {
    sr: f32,
    rng: Rng,
    ph: f32,
    inc: f32,
    jit: f32,
    cur_j: f32,
    hold: [f32; 2],
    step: f32,
    dither: bool,
    lp: [[Lp1; 2]; 2],
    lp_a: f32,
}
impl Crush {
    pub fn new(sr: f32) -> Self {
        Self { sr, rng: Rng(0x1234_5678), ph: 0.0, inc: 1.0, jit: 0.0, cur_j: 0.0, hold: [0.0; 2], step: 0.0, dither: false, lp: [[Lp1::default(); 2]; 2], lp_a: 1.0 }
    }
    pub fn reset(&mut self) {
        self.hold = [0.0; 2];
        self.lp = [[Lp1::default(); 2]; 2];
    }
    pub fn control(&mut self, k: &[f32; MAX_K], var: usize) {
        let rate = crush_rate(k[1]).min(self.sr);
        self.inc = rate / self.sr;
        self.step = 2.0 / 2f32.powf(crush_bits(k[0]));
        self.jit = k[2];
        self.dither = var == 1;
        let fc = (20000f32.ln() + ((rate * 0.45).max(200.0).ln() - 20000f32.ln()) * k[3]).exp();
        self.lp_a = if k[3] < 0.01 { 1.0 } else { op_a(fc, self.sr) };
    }
    #[inline]
    fn q(&mut self, x: f32) -> f32 {
        // dither only while something plays: silence in stays silence out
        let x = if self.dither && x.abs() > 1e-5 { x + (self.rng.u() - self.rng.u()) * self.step } else { x };
        (x / self.step).round() * self.step
    }
    pub fn tick(&mut self, l: f32, r: f32) -> (f32, f32) {
        self.ph += self.inc * (1.0 + self.jit * self.cur_j);
        if self.ph >= 1.0 {
            self.ph -= 1.0;
            self.cur_j = self.rng.b() * 0.9;
            self.hold = [self.q(l), self.q(r)];
        }
        let mut o = [0.0; 2];
        for ch in 0..2 {
            let a = self.lp_a;
            let f = &mut self.lp[ch];
            let f0 = f[0].tick(self.hold[ch], a);
            o[ch] = f[1].tick(f0, a);
        }
        (o[0], o[1])
    }
}

// ------------------------------------------------------------------ MAGNETIC

pub struct Magnetic {
    sr: f32,
    rng: Rng,
    line: [Line; 2],
    wow_ph: f32,
    flut_ph: f32,
    drift: f32,
    drift_t: f32,
    drift_sm: f32,
    wow_d: f32,
    flut_d: f32,
    sat_g: f32,
    mk: f32,
    hp: [Biq; 2],
    bump: [Biq; 2],
    lp: [Biq; 2],
    lp2: [Lp1; 2],
    lp2_a: f32,
    drop_rate: f32,
    drop_amt: f32,
    drop_left: f32,
    drop_target: f32,
    drop_g: f32,
    env: Env,
    hiss: f32,
    /// dropouts: 0 = volume dips, 1 = the top end dips
    vf: f32,
    drop_k: f32,
    dlp: [Lp1; 2],
    /// CHOW machines (ChowTape hysteresis + loss): None = the classic engine
    chow: Option<usize>,
    hyst: [crate::chow::Hyst; 2],
    chow_mk: f32,
    chow_clip: f32,
}
impl Magnetic {
    pub fn new(sr: f32) -> Self {
        Self { sr, rng: Rng(0x0bad_cafe), line: [Line::new((0.03 * sr) as usize), Line::new((0.03 * sr) as usize)], wow_ph: 0.0, flut_ph: 0.0, drift: 0.0, drift_t: 0.0, drift_sm: 0.0, wow_d: 0.0, flut_d: 0.0, sat_g: 1.0, mk: 1.0, hp: [Biq::default(); 2], bump: [Biq::default(); 2], lp: [Biq::default(); 2], lp2: [Lp1::default(); 2], lp2_a: 1.0, drop_rate: 0.0, drop_amt: 0.0, drop_left: 0.0, drop_target: 1.0, drop_g: 1.0, env: Env::new(0.005, 0.3, sr), hiss: 0.0, vf: 0.0, drop_k: 0.002, dlp: [Lp1::default(); 2], chow: None, hyst: [crate::chow::Hyst::new(sr); 2], chow_mk: 1.0, chow_clip: 10.0 }
    }
    pub fn reset(&mut self) {
        self.line.iter_mut().for_each(|l| l.clear());
        for b in self.hp.iter_mut().chain(self.bump.iter_mut()).chain(self.lp.iter_mut()) {
            b.clear();
        }
        self.drop_g = 1.0;
        self.drop_left = 0.0;
        self.hyst.iter_mut().for_each(|h| h.reset());
    }
    pub fn control(&mut self, k: &[f32; MAX_K], var: usize) {
        let sr = self.sr;
        let v = var % 3;
        let chow = if var >= 3 { Some(v) } else { None };
        if chow.is_some() && self.chow != chow {
            self.hyst.iter_mut().for_each(|h| h.reset());
        }
        self.chow = chow;
        let base_lp = [12000.0, 18000.0, 7000.0][v];
        let hp = [40.0, 25.0, 80.0][v];
        let (bdb, bf) = [(1.5, 90.0), (2.5, 55.0), (1.0, 120.0)][v];
        let (ws, fs) = [(1.3, 1.0), (0.6, 0.5), (1.6, 1.4)][v];
        // WEAR <-> FLUTTER: the bar leans the module one way or the other
        let bal = k[4];
        let wear = k[0] * (2.0 * (1.0 - bal)).min(1.0);
        let flut = k[3] * (2.0 * bal).min(1.0);
        let f = (base_lp * 0.15f32.powf(wear)).max(1500.0);
        for ch in 0..2 {
            self.hp[ch].highpass(hp, 0.7, sr);
            self.bump[ch].peak(bf, 1.0, bdb, sr);
            self.lp[ch].lowpass(f, 0.75, sr);
        }
        self.lp2_a = op_a(f * 1.5, sr);
        self.drop_rate = k[1].powf(1.5) * 2.5;
        self.drop_amt = k[1];
        self.sat_g = 1.0 + k[2] * 6.0;
        self.mk = 1.0 + k[2] * 0.8;
        self.wow_d = flut * ws * 0.0025 * sr;
        self.flut_d = flut * fs * 0.00012 * sr;
        self.vf = k[5];
        self.drop_k = 0.0005 * 100f32.powf(k[6]);
        self.hiss = db([-62.0, -70.0, -56.0][v]) * wear;
        if let Some(m) = chow {
            // CHOW: SATURATE = tape drive into the hysteresis, WEAR = head-to-tape spacing
            let (speed, gap, thick, sat, width, clip, max_sp) = crate::chow::MACHINES[m];
            let drive = 0.1 + 0.9 * k[2];
            for h in self.hyst.iter_mut() {
                h.cook(drive, width, sat);
            }
            // ChowTape's makeup, then level-matched to the classic engine across SATURATE (fair A/B)
            let trim = [1.0, 1.33, 1.0][m];
            self.chow_mk = crate::chow::Hyst::makeup(width, sat) * trim / (0.4 + 1.5 * drive);
            self.chow_clip = clip;
            let spacing = 0.1 + wear * wear * max_sp;
            let corner = crate::chow::loss_corner(speed, spacing, thick, gap, (0.45 * sr).min(22000.0));
            let (bf, bdb) = crate::chow::head_bump(speed, gap);
            for ch in 0..2 {
                self.hp[ch].highpass(18.0, 0.7, sr);
                self.bump[ch].peak(bf, 2.0, bdb, sr);
                self.lp[ch].lowpass(corner.min(0.45 * sr), 0.6, sr);
            }
            self.lp2_a = 1.0;
            self.sat_g = 1.0;
            self.mk = 1.0;
        }
    }
    pub fn tick(&mut self, l: f32, r: f32) -> (f32, f32) {
        let sr = self.sr;
        let e = self.env.tick(l.abs().max(r.abs()));
        self.wow_ph = (self.wow_ph + 0.55 / sr).fract();
        self.flut_ph = (self.flut_ph + 6.3 / sr).fract();
        self.drift_t += 1.0 / sr;
        if self.drift_t > 0.15 {
            self.drift_t = 0.0;
            self.drift = self.rng.b();
        }
        self.drift_sm += 0.0005 * (self.drift - self.drift_sm);
        let d = 2.0 + self.wow_d * (0.5 - 0.5 * (TAU * self.wow_ph).cos()) * (1.0 + 0.3 * self.drift_sm) + self.flut_d * (1.0 + (TAU * self.flut_ph).sin());
        // dropouts
        if self.drop_left > 0.0 {
            self.drop_left -= 1.0;
            if self.drop_left <= 0.0 {
                self.drop_target = 1.0;
            }
        } else if self.drop_rate > 0.0 && self.rng.u() < self.drop_rate / sr {
            self.drop_left = (0.03 + 0.17 * self.rng.u()) * sr;
            self.drop_target = db(-(3.0 + 12.0 * self.rng.u()) * self.drop_amt);
        }
        self.drop_g += self.drop_k * (self.drop_target - self.drop_g);
        let gv = 1.0 - (1.0 - self.drop_g) * (1.0 - self.vf);
        let fa = if self.vf > 0.001 && self.drop_g < 0.999 { op_a(300.0 + 19000.0 * self.drop_g.powf(4.0 * self.vf), sr) } else { 1.0 };
        let _ = self.hiss;
        let ins = [l, r];
        let mut o = [0.0; 2];
        for ch in 0..2 {
            self.line[ch].push(ins[ch]);
            let mut y = self.line[ch].cubic(d);
            y = if self.chow.is_some() {
                self.hyst[ch].tick(y.clamp(-self.chow_clip, self.chow_clip)) * self.chow_mk
            } else {
                soft(y, self.sat_g) * self.mk
            };
            y = self.hp[ch].tick(y);
            y = self.bump[ch].tick(y);
            y = self.lp[ch].tick(y);
            y = self.lp2[ch].tick(y, self.lp2_a);
            if fa < 1.0 {
                y = self.dlp[ch].tick(y, fa);
            } else {
                self.dlp[ch].z = y;
            }
            // (no injected hiss: it rode up with the music and read as harsh top end; NOISE TAPE does hiss)
            o[ch] = y * gv;
        }
        (o[0], o[1])
    }
}

// ------------------------------------------------------------------ CHOP

pub struct Chop {
    rng: Rng,
    line: [Line; 2],
    var: usize,
    div: f32,
    depth: f32,
    smooth_a: f32,
    fade_s: f32,
    swing: f32,
    chance: f32,
    repeats: f32,
    g: f32,
    last_key: i64,
    gated: bool,
    rep: bool,
    t0: u64,
    slice: f32,
    xf: f32,
}
impl Chop {
    pub fn new(sr: f32) -> Self {
        Self { rng: Rng(0x5eed_1234), line: [Line::new((2.5 * sr) as usize), Line::new((2.5 * sr) as usize)], var: 0, div: 0.5, depth: 0.7, smooth_a: 0.1, fade_s: 64.0, swing: 0.0, chance: 1.0, repeats: 1.0, g: 1.0, last_key: i64::MIN, gated: false, rep: false, t0: 0, slice: 1.0, xf: 0.0 }
    }
    pub fn reset(&mut self) {
        self.line.iter_mut().for_each(|l| l.clear());
        self.g = 1.0;
        self.last_key = i64::MIN;
        self.rep = false;
        self.xf = 0.0;
    }
    pub fn control(&mut self, k: &[f32; MAX_K], var: usize, c: &Ctx) {
        self.var = var;
        self.div = pick(&CHOP_DIVS, k[0]).1;
        self.depth = k[1];
        let ms = 0.5 + 20.0 * k[2];
        self.smooth_a = tc(ms * 0.001, c.sr);
        self.fade_s = (ms * 0.001 * c.sr).max(8.0);
        self.swing = k[3];
        self.chance = k[4];
        self.repeats = pick(&[1.0, 2.0, 4.0, 8.0], k[1]);
    }
    pub fn tick(&mut self, l: f32, r: f32, c: &Ctx) -> (f32, f32) {
        self.line[0].push(l);
        self.line[1].push(r);
        // steps come in pairs; SWING pushes the second step of each pair later
        let pair = c.beat / (2.0 * self.div as f64);
        let pi = pair.floor() as i64;
        let in_b = (pair - pi as f64) as f32 >= 0.5 + 0.25 * self.swing;
        let key = pi * 2 + in_b as i64;
        let new_step = key != self.last_key;
        if new_step {
            self.last_key = key;
            let hit = self.rng.u() < self.chance;
            if self.var == 0 {
                self.gated = in_b && hit;
            } else {
                // stutter: repeat the slice that just played
                self.rep = hit && key > 0;
                self.t0 = c.n;
                self.slice = (self.div * c.spb / self.repeats).clamp(16.0, self.line[0].cap() as f32 * 0.4);
            }
        }
        if self.var == 0 {
            let target = if self.gated { 1.0 - self.depth } else { 1.0 };
            self.g += self.smooth_a * (target - self.g);
            return (l * self.g, r * self.g);
        }
        let tgt = if self.rep { 1.0 } else { 0.0 };
        self.xf += (tgt - self.xf).clamp(-0.01, 0.01);
        if self.xf <= 0.0 {
            return (l, r);
        }
        let t = (c.n - self.t0) as f32;
        let m = t % self.slice;
        let age = (t + self.slice - m) as usize;
        let w = (m / self.fade_s).min((self.slice - m) / self.fade_s).min(1.0);
        let (a, b) = (self.line[0].at(age) * w, self.line[1].at(age) * w);
        (l + (a - l) * self.xf, r + (b - r) * self.xf)
    }
}

// ------------------------------------------------------------------ WOBBLE

pub struct Wobble {
    sr: f32,
    rng: Rng,
    line: [Line; 2],
    sync: bool,
    hz: f32,
    div: f32,
    ph: f32,
    fph: f32,
    wow: f32,
    flut: f32,
    drift: f32,
    stereo: f32,
    dr: [f32; 2],
    dr_t: [f32; 2],
    dr_c: f32,
}
impl Wobble {
    pub fn new(sr: f32) -> Self {
        Self { sr, rng: Rng(0x7777_1111), line: [Line::new((0.03 * sr) as usize), Line::new((0.03 * sr) as usize)], sync: false, hz: 1.0, div: 4.0, ph: 0.0, fph: 0.0, wow: 0.0, flut: 0.0, drift: 0.0, stereo: 0.0, dr: [0.0; 2], dr_t: [0.0; 2], dr_c: 0.0 }
    }
    pub fn reset(&mut self) {
        self.line.iter_mut().for_each(|l| l.clear());
    }
    pub fn control(&mut self, k: &[f32; MAX_K], var: usize) {
        let sr = self.sr;
        self.sync = var == 1;
        self.hz = wob_free_hz(k[1]);
        self.div = pick(&LFO_DIVS, k[1]).1;
        self.wow = k[0] * 0.004 * sr;
        self.flut = k[2] * 0.0003 * sr;
        self.drift = k[3] * 0.003 * sr;
        self.stereo = k[4];
        self.dr_c += 32.0;
        if self.dr_c > 0.25 * sr {
            self.dr_c = 0.0;
            let a = self.rng.b();
            let b = self.rng.b();
            self.dr_t = [a, a + (b - a) * self.stereo];
        }
    }
    pub fn tick(&mut self, l: f32, r: f32, c: &Ctx) -> (f32, f32) {
        let sr = self.sr;
        let ph = if self.sync {
            (c.beat / self.div as f64).fract() as f32
        } else {
            self.ph = (self.ph + self.hz / sr).fract();
            self.ph
        };
        self.fph = (self.fph + 7.3 / sr).fract();
        let ins = [l, r];
        let mut o = [0.0; 2];
        for ch in 0..2 {
            self.dr[ch] += 0.00015 * (self.dr_t[ch] - self.dr[ch]);
            let p = ph + ch as f32 * self.stereo * 0.5;
            let d = 1.0 + 0.001 * sr + self.wow * (0.5 - 0.5 * (TAU * p).cos()) + self.flut * (1.0 + (TAU * (self.fph + ch as f32 * 0.25)).sin()) + self.drift * (1.0 + self.dr[ch]);
            self.line[ch].push(ins[ch]);
            o[ch] = self.line[ch].cubic(d);
        }
        (o[0], o[1])
    }
}

// ------------------------------------------------------------------ ECHO

/// ECHO strip extras (params outside the module's seven knobs).
#[derive(Clone, Copy, Debug)]
pub struct EchoX {
    pub ping: bool,
    /// SCRUB on: TIME changes slide like tape (pitch bends); off: clean crossfade
    pub scrub: bool,
    pub smooth: f32,
    /// semitones, applied to every trip round the loop (and to pitched grains)
    pub pitch: f32,
    /// CUT inside the loop
    pub lo: f32,
    pub hi: f32,
}
impl Default for EchoX {
    fn default() -> Self {
        Self { ping: false, scrub: true, smooth: 0.0, pitch: 0.0, lo: 20.0, hi: 20000.0 }
    }
}

/// Two crossfading taps: a cheap pitch shifter for the feedback loop.
#[derive(Clone)]
struct PShift {
    line: Line,
    ph: f32,
}
impl PShift {
    fn new() -> Self {
        Self { line: Line::new(8192), ph: 0.0 }
    }
    #[inline]
    fn tick(&mut self, x: f32, ratio: f32, win: f32) -> f32 {
        self.line.push(x);
        self.ph = (self.ph + (1.0 - ratio) / win).rem_euclid(1.0);
        let p2 = (self.ph + 0.5).fract();
        let w1 = 0.5 - 0.5 * (TAU * self.ph).cos();
        let w2 = 0.5 - 0.5 * (TAU * p2).cos();
        self.line.cubic(self.ph * win + 2.0) * w1 + self.line.cubic(p2 * win + 2.0) * w2
    }
}

pub struct Echo {
    sr: f32,
    line: [Line; 2],
    var: usize,
    d: f32,
    d_t: f32,
    /// crossfade from the old time (SCRUB off)
    d_old: f32,
    xf: f32,
    fb: f32,
    lp: [Lp1; 2],
    hp: [Lp1; 2],
    lp_a: f32,
    hp_a: f32,
    cut_lp: [[Lp1; 2]; 2],
    cut_hp: [[Lp1; 2]; 2],
    clp_a: f32,
    chp_a: f32,
    cut_on: (bool, bool),
    wob: f32,
    ph: f32,
    width: f32,
    /// THRU RACK: every module in the rack gets at least this much of the repeats
    pub thru: f32,
    /// GRAINS: how much of the repeats are scattered grains
    grains: f32,
    gr: [Grain; 6],
    g_next: f32,
    rng: Rng,
    x: EchoX,
    ratio: f32,
    ps: [PShift; 2],
    sm_f: f32,
    sm_s: f32,
    sm_lp: [Lp1; 2],
    sm_a: f32,
    /// the last repeat (before WIDTH), for the meter
    pub last: (f32, f32),
}

/// One grain read from the echo line.
#[derive(Clone, Copy, Default)]
struct Grain {
    on: bool,
    d: f32,
    rate: f32,
    age: f32,
    len: f32,
    pan: f32,
}

impl Echo {
    pub fn new(sr: f32) -> Self {
        Self {
            sr,
            line: [Line::new((4.0 * sr) as usize), Line::new((4.0 * sr) as usize)],
            var: 0,
            d: -1.0,
            d_t: 1.0,
            d_old: 1.0,
            xf: 1.0,
            fb: 0.0,
            lp: [Lp1::default(); 2],
            hp: [Lp1::default(); 2],
            lp_a: 1.0,
            hp_a: 0.0,
            cut_lp: [[Lp1::default(); 2]; 2],
            cut_hp: [[Lp1::default(); 2]; 2],
            clp_a: 1.0,
            chp_a: 0.0,
            cut_on: (false, false),
            wob: 0.0,
            ph: 0.0,
            width: 1.0,
            thru: 0.0,
            grains: 0.0,
            gr: [Grain::default(); 6],
            g_next: 0.0,
            rng: Rng(0x5eed_1234),
            x: EchoX::default(),
            ratio: 1.0,
            ps: [PShift::new(), PShift::new()],
            sm_f: 0.0,
            sm_s: 0.0,
            sm_lp: [Lp1::default(); 2],
            sm_a: 1.0,
            last: (0.0, 0.0),
        }
    }
    pub fn reset(&mut self) {
        self.line.iter_mut().for_each(|l| l.clear());
        self.lp = [Lp1::default(); 2];
        self.hp = [Lp1::default(); 2];
        self.cut_lp = [[Lp1::default(); 2]; 2];
        self.cut_hp = [[Lp1::default(); 2]; 2];
        self.d = -1.0;
        self.xf = 1.0;
        self.gr = [Grain::default(); 6];
        self.ps = [PShift::new(), PShift::new()];
        self.sm_f = 0.0;
        self.sm_s = 0.0;
        self.last = (0.0, 0.0);
    }
    pub fn control(&mut self, k: &[f32; MAX_K], var: usize, c: &Ctx, x: &EchoX) {
        let sr = self.sr;
        self.var = var;
        self.x = *x;
        let t = (pick(&ECHO_DIVS, k[0]).1 * c.spb).clamp(16.0, self.line[0].cap() as f32 * 0.9);
        if self.d < 0.0 {
            self.d = t;
            self.d_old = t;
        }
        if !x.scrub && (t - self.d_t).abs() > 1.0 && self.xf >= 1.0 {
            // clean jump: fade from where we are to the new time
            self.d_old = self.d;
            self.d = t;
            self.xf = 0.0;
        }
        self.d_t = t;
        self.fb = k[1] * 0.95;
        let tn = k[2];
        let lp_f = 1200.0 * 16f32.powf(tn) * if var == 0 { 0.6 } else { 1.0 };
        self.lp_a = op_a(lp_f.min(19000.0), sr);
        self.hp_a = op_a(40.0 + 300.0 * tn, sr);
        self.wob = k[3] * if var == 0 { 0.003 } else { 0.0012 } * sr;
        self.width = k[4] * 2.0;
        self.thru = k[5];
        self.grains = k[6];
        self.ratio = 2f32.powf(x.pitch / 12.0);
        self.cut_on = (x.lo > 21.0, x.hi < 19900.0);
        self.chp_a = op_a(x.lo, sr);
        self.clp_a = op_a(x.hi, sr);
        self.sm_a = op_a(20000.0 * 0.15f32.powf(x.smooth), sr);
    }
    /// Grains: up to 6 windowed reads scattered around the echo time; some pitched
    /// (PITCH, or an octave either way when PITCH is 0) or backwards as GRAINS goes up.
    fn grain_tick(&mut self) -> [f32; 2] {
        let sr = self.sr;
        let g = self.grains;
        self.g_next -= 1.0;
        if self.g_next <= 0.0 {
            if let Some(slot) = self.gr.iter_mut().position(|x| !x.on) {
                let pitched = (self.ratio - 1.0).abs() > 0.01;
                let r = &mut self.rng;
                let len = (0.03 + 0.12 * r.u()) * sr;
                let u = r.u();
                let (up, down) = if pitched { (self.ratio, self.ratio) } else { (2.0, 0.5) };
                let rate = if u < 0.18 * g { up } else if u < 0.36 * g { down } else if u < 0.48 * g { -1.0 } else { 1.0 + 0.01 * r.b() };
                let cap = self.line[0].cap() as f32 - 4.0;
                let mut d = self.d * (1.0 + g * 0.8 * r.b()) + 8.0;
                if rate > 1.0 {
                    d += len * (rate - 1.0);
                }
                d = d.clamp(8.0, cap - len * 2.0);
                self.gr[slot] = Grain { on: true, d, rate, age: 0.0, len, pan: 0.5 + 0.45 * self.rng.b() };
            }
            self.g_next = (0.02 + 0.06 * self.rng.u()) * sr * (1.4 - g);
        }
        let mut o = [0.0f32; 2];
        let cap = self.line[0].cap() as f32 - 4.0;
        for gn in self.gr.iter_mut().filter(|x| x.on) {
            let w = 0.5 - 0.5 * (TAU * gn.age / gn.len).cos();
            let d = gn.d.clamp(2.0, cap);
            let (a, b) = (self.line[0].cubic(d), self.line[1].cubic(d));
            let v = 0.5 * (a + b);
            o[0] += w * (0.5 * a + v * (1.0 - gn.pan));
            o[1] += w * (0.5 * b + v * gn.pan);
            gn.d += 1.0 - gn.rate;
            gn.age += 1.0;
            if gn.age >= gn.len {
                gn.on = false;
            }
        }
        [o[0] * 0.55, o[1] * 0.55]
    }
    #[inline]
    fn read_at(&self, ch: usize, d: f32, m: f32, spread: f32) -> f32 {
        self.line[ch].frac(d * (1.0 + spread * ch as f32) + m)
    }
    /// The repeats coming off the line now (before WIDTH). Also kept in `last`.
    pub fn read(&mut self) -> [f32; 2] {
        let sr = self.sr;
        if self.x.scrub {
            // tape: the heads slide to the new time, pitch bends on the way
            self.d += 0.00025 * (self.d_t - self.d);
        }
        if self.xf < 1.0 {
            self.xf = (self.xf + 1.0 / (0.03 * sr)).min(1.0);
        }
        self.ph = (self.ph + 0.7 / sr).fract();
        let spread = if self.ping() { 0.0 } else { 0.015 * self.width.min(1.0) };
        let mut y = [0.0f32; 2];
        for ch in 0..2 {
            let m = self.wob * (0.5 + 0.5 * (TAU * (self.ph + 0.3 * ch as f32)).sin());
            let mut raw = self.read_at(ch, self.d, m, spread);
            if self.xf < 1.0 {
                let o = self.read_at(ch, self.d_old, m, spread);
                let a = (self.xf * std::f32::consts::FRAC_PI_2).sin();
                let b = (self.xf * std::f32::consts::FRAC_PI_2).cos();
                raw = raw * a + o * b;
            }
            // tone in the loop: every repeat gets darker / thinner
            let lo = self.lp[ch].tick(raw, self.lp_a);
            let mut v = lo - self.hp[ch].tick(lo, self.hp_a);
            // CUT in the loop (2 poles each side)
            if self.cut_on.1 {
                v = self.cut_lp[ch][0].tick(v, self.clp_a);
                v = self.cut_lp[ch][1].tick(v, self.clp_a);
            }
            if self.cut_on.0 {
                let a = self.chp_a;
                let h = v - self.cut_hp[ch][0].tick(v, a);
                v = h - self.cut_hp[ch][1].tick(h, a);
            }
            y[ch] = if self.var == 0 { soft(v, 1.6) } else { v };
        }
        if self.grains > 0.005 {
            let g = self.grain_tick();
            let gm = self.grains;
            for ch in 0..2 {
                y[ch] += (g[ch] - y[ch]) * gm;
            }
        }
        self.last = (y[0], y[1]);
        y
    }
    fn ping(&self) -> bool {
        self.var == 2 || self.x.ping
    }
    /// Feeds the line: `l`/`r` is the new audio going in, `f` the repeats that go
    /// round again (already through any modules they were sent to).
    pub fn write(&mut self, l: f32, r: f32, f: [f32; 2]) {
        let sr = self.sr;
        // SMOOTH: transients going in are rounded off (and the top end softened)
        let (mut il, mut ir) = (l, r);
        if self.x.smooth > 0.005 {
            let e = l.abs().max(r.abs());
            self.sm_f += if e > self.sm_f { 0.5 } else { 0.0015 } * (e - self.sm_f);
            self.sm_s += if e > self.sm_s { 0.0015 } else { 0.0004 } * (e - self.sm_s);
            let g = if self.sm_f > self.sm_s && self.sm_f > 1e-5 { (self.sm_s / self.sm_f).powf(self.x.smooth * 1.5) } else { 1.0 };
            il = self.sm_lp[0].tick(l * g, self.sm_a);
            ir = self.sm_lp[1].tick(r * g, self.sm_a);
        }
        let fb = self.fb;
        let mut f = f;
        // PITCH: every trip round the loop shifts again (shimmer / falling repeats)
        if (self.ratio - 1.0).abs() > 0.001 {
            let win = 0.06 * sr;
            for ch in 0..2 {
                f[ch] = self.ps[ch].tick(f[ch], self.ratio, win);
            }
        }
        if self.ping() {
            self.line[0].push(soft(0.5 * (il + ir) + fb * f[1], 1.2));
            self.line[1].push(soft(fb * f[0], 1.2));
        } else {
            self.line[0].push(soft(il + fb * f[0], 1.2));
            self.line[1].push(soft(ir + fb * f[1], 1.2));
        }
    }
    /// WIDTH on the repeats.
    pub fn spread(&self, y: [f32; 2]) -> (f32, f32) {
        let m = 0.5 * (y[0] + y[1]);
        let s = 0.5 * (y[0] - y[1]) * self.width;
        (m + s, m - s)
    }
    /// All in one (no modules in the loop).
    pub fn tick(&mut self, l: f32, r: f32) -> (f32, f32) {
        let y = self.read();
        self.write(l, r, y);
        self.spread(y)
    }
}

// ------------------------------------------------------------------ CHORUS

pub struct Chorus {
    sr: f32,
    line: [Line; 2],
    var: usize,
    hz: f32,
    depth: f32,
    fb: f32,
    tone_a: f32,
    width: f32,
    ph: f32,
    ph2: f32,
    last: [f32; 2],
    lp: [Lp1; 2],
}
impl Chorus {
    pub fn new(sr: f32) -> Self {
        Self { sr, line: [Line::new((0.05 * sr) as usize), Line::new((0.05 * sr) as usize)], var: 0, hz: 0.5, depth: 0.5, fb: 0.0, tone_a: 1.0, width: 0.7, ph: 0.0, ph2: 0.0, last: [0.0; 2], lp: [Lp1::default(); 2] }
    }
    pub fn reset(&mut self) {
        self.line.iter_mut().for_each(|l| l.clear());
        self.last = [0.0; 2];
    }
    pub fn control(&mut self, k: &[f32; MAX_K], var: usize) {
        self.var = var;
        self.hz = chorus_hz(k[0]);
        self.depth = k[1];
        self.fb = k[2] * 0.9 * if var == 2 { 1.0 } else { 0.5 };
        self.tone_a = op_a(2000.0 * 10f32.powf(k[3]), self.sr);
        self.width = k[4];
    }
    pub fn tick(&mut self, l: f32, r: f32) -> (f32, f32) {
        let ms = 0.001 * self.sr;
        self.ph = (self.ph + self.hz / self.sr).fract();
        self.ph2 = (self.ph2 + self.hz * 6.1 / self.sr).fract();
        let ins = [l, r];
        let mut o = [0.0; 2];
        for ch in 0..2 {
            let off = ch as f32 * self.width * 0.5;
            let lfo = |p: f32| 0.5 - 0.5 * (TAU * (p + off).fract()).cos();
            self.line[ch].push(ins[ch] + self.fb * self.last[ch]);
            let y = match self.var {
                1 => {
                    let mut s = 0.0;
                    for v in 0..3 {
                        let p = self.ph + v as f32 / 3.0;
                        let d = (10.0 + 3.0 * self.depth * lfo(p) + 0.5 * self.depth * lfo(self.ph2 + v as f32 / 3.0)) * ms;
                        s += self.line[ch].cubic(d);
                    }
                    s * 0.45
                }
                2 => self.line[ch].cubic((0.3 + 5.0 * self.depth * lfo(self.ph)) * ms),
                3 => self.line[ch].cubic((2.0 + 4.0 * self.depth * lfo(self.ph)) * ms),
                _ => self.line[ch].cubic((12.0 + 6.0 * self.depth * lfo(self.ph)) * ms),
            };
            self.last[ch] = y;
            o[ch] = self.lp[ch].tick(y, self.tone_a);
        }
        (o[0], o[1])
    }
}

// ------------------------------------------------------------------ TIME

/// Half-time / double-time / tape stop, restarting every CYCLE on the beat.
pub struct Time {
    rng: Rng,
    line: [Line; 2],
    var: usize,
    div: f32,
    fade: f32,
    len: f32,
    chance: f32,
    last: i64,
    active: bool,
    t0: u64,
    cyc: f32,
}
impl Time {
    pub fn new(sr: f32) -> Self {
        Self { rng: Rng(0x2468_ace0), line: [Line::new((8.0 * sr) as usize), Line::new((8.0 * sr) as usize)], var: 0, div: 2.0, fade: 64.0, len: 0.5, chance: 1.0, last: i64::MIN, active: false, t0: 0, cyc: 1.0 }
    }
    pub fn reset(&mut self) {
        self.line.iter_mut().for_each(|l| l.clear());
        self.last = i64::MIN;
        self.active = false;
    }
    pub fn control(&mut self, k: &[f32; MAX_K], var: usize, c: &Ctx) {
        self.var = var;
        self.div = pick(&TIME_DIVS, k[0]).1;
        self.fade = ((1.0 + 29.0 * k[1]) * 0.001 * c.sr).max(8.0);
        self.len = k[2];
        self.chance = k[3];
    }
    pub fn tick(&mut self, l: f32, r: f32, c: &Ctx) -> (f32, f32) {
        self.line[0].push(l);
        self.line[1].push(r);
        let cy = (c.beat / self.div as f64).floor() as i64;
        if cy != self.last {
            self.last = cy;
            self.active = self.rng.u() < self.chance;
            self.t0 = c.n;
            self.cyc = (self.div * c.spb).clamp(64.0, self.line[0].cap() as f32 * 0.45);
        }
        if !self.active {
            return (l, r);
        }
        let t = (c.n - self.t0) as f32;
        let big = self.cyc;
        let age = match self.var {
            // the start of this cycle, played at 1/2, 1/4 or 1/8 speed
            0 => t * 0.5,
            3 => t * 0.75,
            4 => t * 0.875,
            // the last cycle at double speed, twice
            1 => t + big - (2.0 * t) % big,
            // tape stop over the last LENGTH of the cycle
            _ => {
                let start = big * (1.0 - 0.95 * self.len);
                if t < start {
                    0.0
                } else {
                    let tt = big - start;
                    let u = ((t - start) / tt).min(1.0);
                    t - (start + tt * (u - 0.5 * u * u))
                }
            }
        };
        let w = (t / self.fade).min((big - t).max(0.0) / self.fade).min(1.0);
        let rd = |ln: &Line| if age < 1.0 { ln.at(0) } else { ln.cubic(age) };
        (rd(&self.line[0]) * w, rd(&self.line[1]) * w)
    }
}

// ------------------------------------------------------------------ REVERSE

pub struct Reverse {
    rng: Rng,
    line: [Line; 2],
    var: usize,
    div: f32,
    fade: f32,
    chance: f32,
    last: i64,
    active: bool,
    t0: u64,
    sl: f32,
}
impl Reverse {
    pub fn new(sr: f32) -> Self {
        Self { rng: Rng(0x1357_9bdf), line: [Line::new((5.0 * sr) as usize), Line::new((5.0 * sr) as usize)], var: 0, div: 1.0, fade: 64.0, chance: 1.0, last: i64::MIN, active: false, t0: 0, sl: 1.0 }
    }
    pub fn reset(&mut self) {
        self.line.iter_mut().for_each(|l| l.clear());
        self.last = i64::MIN;
        self.active = false;
    }
    pub fn control(&mut self, k: &[f32; MAX_K], var: usize, c: &Ctx) {
        self.var = var;
        self.div = pick(&REV_DIVS, k[0]).1;
        self.fade = ((1.0 + 29.0 * k[1]) * 0.001 * c.sr).max(8.0);
        self.chance = k[2];
    }
    pub fn tick(&mut self, l: f32, r: f32, c: &Ctx) -> (f32, f32) {
        self.line[0].push(l);
        self.line[1].push(r);
        let s = (c.beat / self.div as f64).floor() as i64;
        if s != self.last {
            self.last = s;
            self.active = self.rng.u() < self.chance;
            self.t0 = c.n;
            self.sl = (self.div * c.spb).clamp(64.0, self.line[0].cap() as f32 * 0.45);
        }
        if !self.active {
            return (l, r);
        }
        // the previous slice, backwards
        let t = (c.n - self.t0) as f32;
        let age = (2.0 * t + 1.0) as usize;
        let mut w = (t / self.fade).min((self.sl - t).max(0.0) / self.fade).min(1.0);
        if self.var == 1 {
            w *= ((t / self.sl).min(1.0)).powf(1.5) * 1.6;
        }
        (self.line[0].at(age) * w, self.line[1].at(age) * w)
    }
}

// ------------------------------------------------------------------ FILTER

pub struct Filter {
    sr: f32,
    f: [[Svf; 2]; 2],
    env: Env,
    var: usize,
    cut: f32,
    q: f32,
    mv: f32,
    div: f32,
    env_amt: f32,
    drive: f32,
    mk: f32,
}
impl Filter {
    pub fn new(sr: f32) -> Self {
        Self { sr, f: [[Svf::default(); 2]; 2], env: Env::new(0.005, 0.12, sr), var: 0, cut: 1000.0, q: 0.7, mv: 0.0, div: 2.0, env_amt: 0.0, drive: 1.0, mk: 1.0 }
    }
    pub fn reset(&mut self) {
        self.f.iter_mut().flatten().for_each(|s| s.clear());
        self.env.e = 0.0;
    }
    pub fn control(&mut self, k: &[f32; MAX_K], var: usize, c: &Ctx) {
        self.var = var;
        self.cut = cutoff_hz(k[0]);
        self.q = 0.5 * 30f32.powf(k[1]);
        self.mv = k[2];
        self.div = pick(&LFO_DIVS, k[3]).1;
        self.env_amt = k[4];
        self.drive = db(k[5] * 24.0 + if var == 3 { 6.0 } else { 0.0 });
        self.mk = self.drive.powf(0.3);
        let lfo = (TAU * (c.beat / self.div as f64).fract() as f32).sin();
        let e = self.env.e;
        let en = e / (e + 0.1);
        let f = (self.cut * 2f32.powf(self.mv * 2.0 * lfo + self.env_amt * 6.0 * en)).clamp(20.0, self.sr * 0.45);
        for ch in 0..2 {
            if var == 3 {
                self.f[ch][0].set(f / 2.5, 0.9, self.sr);
                self.f[ch][1].set(f * 2.5, 0.9, self.sr);
            } else {
                self.f[ch][0].set(f, self.q, self.sr);
            }
        }
    }
    pub fn tick(&mut self, l: f32, r: f32) -> (f32, f32) {
        self.env.tick(0.5 * (l + r));
        let ins = [l, r];
        let mut o = [0.0; 2];
        for ch in 0..2 {
            let x = soft(ins[ch], self.drive) * self.mk;
            let (lp, bp, hp) = self.f[ch][0].tick(x);
            o[ch] = match self.var {
                0 => lp,
                1 => hp,
                2 => bp * self.f[ch][0].k,
                _ => self.f[ch][1].tick(hp).0,
            };
        }
        (o[0], o[1])
    }
}

// ------------------------------------------------------------------ WIDTH

pub struct Width {
    sr: f32,
    line: Line,
    w: f32,
    haas: f32,
    mb_a: f32,
    pan: f32,
    lp: [Lp1; 2],
}
impl Width {
    pub fn new(sr: f32) -> Self {
        Self { sr, line: Line::new((0.03 * sr) as usize), w: 1.0, haas: 0.0, mb_a: 0.0, pan: 0.0, lp: [Lp1::default(); 2] }
    }
    pub fn reset(&mut self) {
        self.line.clear();
        self.lp = [Lp1::default(); 2];
    }
    pub fn control(&mut self, k: &[f32; MAX_K]) {
        self.w = k[0] * 2.0;
        self.haas = k[1] * 0.025 * self.sr;
        let f = mono_bass_hz(k[2]);
        self.mb_a = if f > 0.0 { op_a(f, self.sr) } else { 0.0 };
        self.pan = k[3] * 2.0 - 1.0;
    }
    pub fn tick(&mut self, l: f32, r: f32) -> (f32, f32) {
        let m = 0.5 * (l + r);
        let mut s = 0.5 * (l - r);
        if self.mb_a > 0.0 {
            let l0 = self.lp[0].tick(s, self.mb_a);
            let lo = self.lp[1].tick(l0, self.mb_a);
            s -= lo;
        }
        s *= self.w;
        let (mut a, b) = (m + s, m - s);
        self.line.push(b);
        let mut b = if self.haas >= 1.0 { self.line.frac(self.haas) } else { b };
        if self.pan < 0.0 {
            b *= 1.0 + self.pan;
        } else {
            a *= 1.0 - self.pan;
        }
        (a, b)
    }
}
