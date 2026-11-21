//! TAME: zero-latency de-resonance. A bank of narrow band detectors (log-spaced,
//! linked stereo) watches the spectrum. A band that sticks out above its neighbours
//! (a resonance) gets a dynamic bell cut on the signal path, sized by how far it
//! sticks out. Broad tilt is ignored because each band is compared with the
//! average of the bands around it. No FFT, so no added latency.

use std::f32::consts::PI;

pub const BANDS: usize = 40;
const F_LO: f32 = 60.0;
const F_HI: f32 = 18000.0;
/// Control-rate step (samples) for the detectors -> gains -> filter coefficients.
const CR: usize = 32;
const MAX_CUT_DB: f32 = 18.0;

pub fn band_freq(k: usize) -> f32 {
    F_LO * (F_HI / F_LO).powf(k as f32 / (BANDS - 1) as f32)
}

/// Band-pass detector (Simper SVF, band output).
#[derive(Clone, Copy, Default)]
struct Bp {
    g: f32,
    k: f32,
    s1: f32,
    s2: f32,
}
impl Bp {
    fn set(&mut self, f: f32, q: f32, sr: f32) {
        self.g = (PI * (f / sr).min(0.49)).tan();
        self.k = 1.0 / q;
    }
    #[inline]
    fn tick(&mut self, x: f32) -> f32 {
        let a1 = 1.0 / (1.0 + self.g * (self.g + self.k));
        let v3 = x - self.s2;
        let v1 = a1 * self.s1 + self.g * a1 * v3;
        let v2 = self.s2 + self.g * v1;
        self.s1 = 2.0 * v1 - self.s1;
        self.s2 = 2.0 * v2 - self.s2;
        v1 * self.k
    }
}

/// Peaking bell (RBJ), transposed direct form II, coefficients set at control rate.
#[derive(Clone, Copy)]
struct Bell {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
    z: [[f32; 2]; 2],
    active: bool,
}
impl Default for Bell {
    fn default() -> Self {
        Self { b0: 1.0, b1: 0.0, b2: 0.0, a1: 0.0, a2: 0.0, z: [[0.0; 2]; 2], active: false }
    }
}
impl Bell {
    fn set(&mut self, f: f32, q: f32, gain_db: f32, sr: f32) {
        if gain_db > -0.05 {
            // Flat: skip the filter but keep it ready to fade back in smoothly.
            if self.active {
                self.active = false;
                self.z = [[0.0; 2]; 2];
            }
            return;
        }
        self.active = true;
        let a = 10f32.powf(gain_db / 40.0);
        let w = 2.0 * PI * (f / sr).min(0.49);
        let (sn, cs) = w.sin_cos();
        let al = sn / (2.0 * q);
        let a0 = 1.0 + al / a;
        self.b0 = (1.0 + al * a) / a0;
        self.b1 = -2.0 * cs / a0;
        self.b2 = (1.0 - al * a) / a0;
        self.a1 = -2.0 * cs / a0;
        self.a2 = (1.0 - al / a) / a0;
    }
    #[inline]
    fn tick(&mut self, ch: usize, x: f32) -> f32 {
        if !self.active {
            return x;
        }
        let z = &mut self.z[ch];
        let y = self.b0 * x + z[0];
        z[0] = self.b1 * x - self.a1 * y + z[1];
        z[1] = self.b2 * x - self.a2 * y;
        y
    }
}

pub struct Settings {
    pub amount: f32,
    pub sharp: f32,
    pub speed: f32,
    pub lo: f32,
    pub hi: f32,
}

pub struct Tame {
    sr: f32,
    det: [Bp; BANDS],
    env: [f32; BANDS],
    acc: [f32; BANDS],
    /// current cut per band in dB (<= 0), smoothed
    pub cut: [f32; BANDS],
    bells: [Bell; BANDS],
    n: usize,
    q_set: f32,
}

impl Tame {
    pub fn new(sr: f32) -> Self {
        let mut t = Self { sr, det: [Bp::default(); BANDS], env: [0.0; BANDS], acc: [0.0; BANDS], cut: [0.0; BANDS], bells: [Bell::default(); BANDS], n: 0, q_set: -1.0 };
        t.set_q(0.6);
        t
    }
    fn set_q(&mut self, sharp: f32) {
        // Detectors a bit narrower than the band spacing so neighbours separate.
        let spacing = (F_HI / F_LO).log2() / (BANDS - 1) as f32; // octaves
        let q = 1.0 / (2f32.powf(spacing) - 1.0) * (0.9 + sharp * 0.8);
        for (k, d) in self.det.iter_mut().enumerate() {
            d.set(band_freq(k), q, self.sr);
        }
        self.q_set = sharp;
    }
    pub fn clear(&mut self) {
        for d in self.det.iter_mut() {
            d.s1 = 0.0;
            d.s2 = 0.0;
        }
        for b in self.bells.iter_mut() {
            b.z = [[0.0; 2]; 2];
            b.active = false;
        }
        self.env = [0.0; BANDS];
        self.acc = [0.0; BANDS];
        self.cut = [0.0; BANDS];
        self.n = 0;
    }

    /// One stereo sample. Returns the tamed sample.
    #[inline]
    pub fn tick(&mut self, l: f32, r: f32, s: &Settings) -> (f32, f32) {
        let m = 0.5 * (l + r);
        for k in 0..BANDS {
            let y = self.det[k].tick(m);
            self.acc[k] += y * y;
        }
        self.n += 1;
        if self.n >= CR {
            self.control(s);
            self.n = 0;
        }
        let (mut a, mut b) = (l, r);
        for bell in self.bells.iter_mut() {
            a = bell.tick(0, a);
            b = bell.tick(1, b);
        }
        (a, b)
    }

    fn control(&mut self, s: &Settings) {
        if (s.sharp - self.q_set).abs() > 0.02 {
            self.set_q(s.sharp);
        }
        let dt = CR as f32 / self.sr;
        // SPEED: 0 = slow and smooth (attack 40 ms, release 400 ms), 1 = fast (2 / 40 ms).
        let att_t = 0.040 * (0.05f32).powf(s.speed);
        let rel_t = 0.400 * (0.1f32).powf(s.speed);
        let (ea, er) = ((-dt / 0.003).exp(), (-dt / 0.030).exp());
        let mut db = [0.0f32; BANDS];
        for k in 0..BANDS {
            let p = self.acc[k] / CR as f32;
            self.acc[k] = 0.0;
            let c = if p > self.env[k] { ea } else { er };
            self.env[k] = p + c * (self.env[k] - p);
            db[k] = 10.0 * (self.env[k] + 1e-12).log10();
        }
        // Neighbourhood: sharp = compare with near bands only (only spikes), soft = wider.
        let w = (2.0 + (1.0 - s.sharp) * 5.0).round() as isize;
        let (ga, gr) = ((-dt / att_t).exp(), (-dt / rel_t).exp());
        let q_bell = {
            let spacing = (F_HI / F_LO).log2() / (BANDS - 1) as f32;
            1.0 / (2f32.powf(spacing) - 1.0) * (0.6 + s.sharp * 1.0)
        };
        for k in 0..BANDS {
            let f = band_freq(k);
            let mut target = 0.0;
            // Signal floor: below -70 dB there's nothing worth taming.
            if f >= s.lo && f <= s.hi && db[k] > -70.0 && s.amount > 0.0 {
                let (mut sum, mut n) = (0.0, 0.0);
                for j in -w..=w {
                    if j.abs() < 1 {
                        continue;
                    }
                    let i = k as isize + j;
                    if i >= 0 && (i as usize) < BANDS {
                        sum += db[i as usize];
                        n += 1.0;
                    }
                }
                if n > 0.0 {
                    let excess = db[k] - sum / n - 3.0; // 3 dB is normal ripple
                    if excess > 0.0 {
                        target = -(excess * (0.35 + 0.9 * s.amount) * s.amount).min(MAX_CUT_DB * s.amount);
                    }
                }
            }
            let g = if target < self.cut[k] { ga } else { gr };
            self.cut[k] = target + g * (self.cut[k] - target);
            self.bells[k].set(f, q_bell, self.cut[k], self.sr);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn noise(seed: &mut u32) -> f32 {
        *seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        (*seed >> 8) as f32 / 8388608.0 - 1.0
    }

    /// A whistle in noise gets cut; plain noise is left nearly alone.
    #[test]
    fn cuts_a_resonance_not_flat_noise() {
        let sr = 48000.0;
        let s = Settings { amount: 0.8, sharp: 0.6, speed: 0.5, lo: 100.0, hi: 16000.0 };
        let run = |tone: f32| {
            let mut t = Tame::new(sr);
            let mut seed = 1u32;
            let mut worst = 0.0f32;
            for n in 0..48000 {
                let x = 0.1 * noise(&mut seed) + tone * (2.0 * PI * 2900.0 * n as f32 / sr).sin();
                let _ = t.tick(x, x, &s);
                if n > 24000 {
                    worst = worst.min(t.cut.iter().cloned().fold(0.0, f32::min));
                }
            }
            (worst, t)
        };
        let (flat, _) = run(0.0);
        let (res, t) = run(0.3);
        let k = (0..BANDS).min_by(|a, b| t.cut[*a].total_cmp(&t.cut[*b])).unwrap();
        assert!(flat > -2.5, "noise alone got {flat} dB");
        assert!(res < -6.0, "whistle only cut {res} dB");
        let f = band_freq(k);
        assert!(f > 2300.0 && f < 3700.0, "cut landed at {f} Hz");
    }

    /// Silence stays silence, and nothing blows up.
    #[test]
    fn silence_and_stability() {
        let mut t = Tame::new(44100.0);
        let s = Settings { amount: 1.0, sharp: 1.0, speed: 1.0, lo: 40.0, hi: 20000.0 };
        for n in 0..44100 {
            let x = if n < 22050 { (n as f32 * 0.3).sin() } else { 0.0 };
            let (a, b) = t.tick(x, x, &s);
            assert!(a.is_finite() && b.is_finite() && a.abs() < 4.0);
            if n > 40000 {
                assert!(a.abs() < 1e-4, "tail {a}");
            }
        }
    }
}
