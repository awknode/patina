//! The finishing stages after the rack: global CUT filters (with shapes), TONE
//! (TILT, MID, SPEAKER, MIC) and the master COMPRESS settings.

use crate::dsp::*;
use crate::params::MAX_K;

/// Low / high cut with a choice of shape: 6, 12, 24, 48 dB/oct or a resonant 12.
pub struct CutF {
    hp: [[Biq; 4]; 2],
    lp: [[Biq; 4]; 2],
    hp1: [Lp1; 2],
    lp1: [Lp1; 2],
    key: (f32, f32, i32),
    n: usize,
    one: bool,
    hp_on: bool,
    lp_on: bool,
    a_hp: f32,
    a_lp: f32,
}
impl Default for CutF {
    fn default() -> Self {
        Self { hp: [[Biq::default(); 4]; 2], lp: [[Biq::default(); 4]; 2], hp1: [Lp1::default(); 2], lp1: [Lp1::default(); 2], key: (-1.0, -1.0, -1), n: 0, one: false, hp_on: false, lp_on: false, a_hp: 0.0, a_lp: 1.0 }
    }
}
impl CutF {
    pub fn clear(&mut self) {
        for ch in 0..2 {
            self.hp[ch].iter_mut().chain(self.lp[ch].iter_mut()).for_each(|b| b.clear());
        }
        self.hp1 = [Lp1::default(); 2];
        self.lp1 = [Lp1::default(); 2];
    }
    pub fn set(&mut self, lo: f32, hi: f32, shape: i32, sr: f32) {
        if self.key == (lo, hi, shape) {
            return;
        }
        self.key = (lo, hi, shape);
        self.hp_on = lo > 21.0;
        self.lp_on = hi < 19900.0;
        self.one = shape == 0;
        self.a_hp = op_a(lo, sr);
        self.a_lp = op_a(hi, sr);
        let qs: &[f32] = match shape {
            1 => &[0.707],
            2 => &[0.541, 1.307],
            3 => &[0.510, 0.601, 0.900, 2.563],
            4 => &[3.0],
            _ => &[],
        };
        self.n = qs.len();
        for ch in 0..2 {
            for (i, &q) in qs.iter().enumerate() {
                self.hp[ch][i].highpass(lo, q, sr);
                self.lp[ch][i].lowpass(hi, q, sr);
            }
        }
    }
    #[inline]
    pub fn tick(&mut self, ch: usize, x: f32) -> f32 {
        let mut y = x;
        if self.one {
            if self.hp_on {
                y -= self.hp1[ch].tick(y, self.a_hp);
            }
            if self.lp_on {
                y = self.lp1[ch].tick(y, self.a_lp);
            }
            return y;
        }
        if self.hp_on {
            for b in self.hp[ch].iter_mut().take(self.n) {
                y = b.tick(y);
            }
        }
        if self.lp_on {
            for b in self.lp[ch].iter_mut().take(self.n) {
                y = b.tick(y);
            }
        }
        y
    }
}

/// TONE: one bipolar knob, four voicings.
///   TILT  darker <-> brighter around 800 Hz (±6 dB)
///   MID   scoop <-> push at 1.2 kHz (±9 dB)
///   SPEAKER  left = small speaker, right = radio; how far = how much
///   MIC      left = ribbon, right = condenser
pub struct Tone {
    lp: [Lp1; 2],
    a: f32,
    g_lo: f32,
    g_hi: f32,
    bq: [[Biq; 3]; 2],
    nb: usize,
    amt: f32,
    mode: i32,
    key: (i32, f32),
    drive: bool,
}
impl Default for Tone {
    fn default() -> Self {
        Self { lp: [Lp1::default(); 2], a: 0.1, g_lo: 1.0, g_hi: 1.0, bq: [[Biq::default(); 3]; 2], nb: 0, amt: 0.0, mode: 0, key: (-1, 9.0), drive: false }
    }
}
impl Tone {
    pub fn clear(&mut self) {
        self.lp = [Lp1::default(); 2];
        for ch in 0..2 {
            self.bq[ch].iter_mut().for_each(|b| b.clear());
        }
    }
    pub fn active(&self) -> bool {
        self.key.1.abs() > 0.004
    }
    pub fn set(&mut self, mode: i32, t: f32, sr: f32) {
        if self.key == (mode, t) {
            return;
        }
        self.key = (mode, t);
        self.mode = mode;
        self.amt = t.abs();
        self.drive = false;
        match mode {
            0 => {
                self.a = op_a(800.0, sr);
                self.g_lo = db(-6.0 * t);
                self.g_hi = db(6.0 * t);
            }
            1 => {
                self.nb = 1;
                for ch in 0..2 {
                    self.bq[ch][0].peak(1200.0, 0.7, 9.0 * t, sr);
                }
            }
            2 => {
                self.nb = 3;
                self.drive = true;
                let (h, l, p, g) = if t < 0.0 { (400.0, 3500.0, 2200.0, 6.0) } else { (150.0, 7000.0, 1500.0, 4.0) };
                for ch in 0..2 {
                    self.bq[ch][0].highpass(h, 0.7, sr);
                    self.bq[ch][1].lowpass(l, 0.9, sr);
                    self.bq[ch][2].peak(p, 1.2, g, sr);
                }
            }
            _ => {
                self.nb = 3;
                let (h, l, lq, p, pq, g) = if t < 0.0 { (40.0, 8000.0, 0.6, 220.0, 0.8, 3.0) } else { (70.0, 15000.0, 0.7, 5000.0, 1.0, 5.0) };
                for ch in 0..2 {
                    self.bq[ch][0].highpass(h, 0.7, sr);
                    self.bq[ch][1].lowpass(l, lq, sr);
                    self.bq[ch][2].peak(p, pq, g, sr);
                }
            }
        }
    }
    #[inline]
    pub fn tick(&mut self, ch: usize, x: f32) -> f32 {
        match self.mode {
            0 => {
                let l = self.lp[ch].tick(x, self.a);
                l * self.g_lo + (x - l) * self.g_hi
            }
            1 => self.bq[ch][0].tick(x),
            _ => {
                let mut y = x;
                for b in self.bq[ch].iter_mut().take(self.nb) {
                    y = b.tick(y);
                }
                if self.drive {
                    y = soft(y * 1.4, 1.8) / 1.4;
                }
                x + (y - x) * self.amt
            }
        }
    }
}

/// Master COMPRESS: one knob drives a glue setting of the chosen model.
pub fn master_comp_k(model: usize, a: f32) -> [f32; MAX_K] {
    // (ratio/mode/bias, attack-or-time, release-or-tubes)
    let (k1, k2, k3) = match model {
        0 => (0.5, 0.2, 0.0),
        1 => (0.29, 0.83, 0.4),
        2 => (0.33, 0.0, 0.2),
        3 => (0.4, 0.65, 0.62),
        _ => (0.4, 0.81, 0.44),
    };
    [a * 0.85, k1, k2, k3, 0.3, 1.0, 0.3]
}
