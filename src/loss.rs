//! LOSS: codec damage. A short-frame spectral "encoder" that does what low-bitrate
//! audio codecs do wrong: the top end disappears, quiet detail inside each band gets
//! thrown away frame by frame (the swirly "underwater" birdies), the side signal is
//! dropped above a frequency (joint stereo), and whole packets go missing.
//!
//! MP3: dropouts are silent gaps.  STREAM: a lost packet repeats the last one (the
//! glitchy buzz of a bad connection).  VOIP: narrow phone band, lost packets become
//! comfort noise.  Fixed latency of one frame (`latency()`), mid/side inside.

use crate::dsp::{Line, Rng};
use crate::params::*;
use realfft::{num_complex::Complex, ComplexToReal, RealFftPlanner, RealToComplex};
use std::sync::Arc;

pub struct Loss {
    sr: f32,
    n: usize,
    hop: usize,
    win: Vec<f32>,
    inb: [Vec<f32>; 2],
    acc: [Vec<f32>; 2],
    outq: [Vec<f32>; 2],
    pos: usize,
    fwd: Arc<dyn RealToComplex<f32>>,
    inv: Arc<dyn ComplexToReal<f32>>,
    tbuf: Vec<f32>,
    spec: [Vec<Complex<f32>>; 2],
    last: [Vec<Complex<f32>>; 2],
    scratch_f: Vec<Complex<f32>>,
    scratch_i: Vec<Complex<f32>>,
    rng: Rng,
    lost: u32,
    // settings
    var: usize,
    bitrate: f32,
    ploss: f32,
    joint: f32,
    stretch: f32,
    blur: f32,
    formant: f32,
    prev_mag: [Vec<f32>; 2],
    env_buf: Vec<f32>,
    /// STRETCH: frames left to hold
    held: u32,
    /// FLUTTER toggle: the decoded stream wavers like a bad transport
    pub flutter: bool,
    fl: [Line; 2],
    fph: f32,
    fph2: f32,
}

impl Loss {
    pub fn new(sr: f32) -> Self {
        let n = if sr > 60000.0 { 1024 } else { 512 };
        let hop = n / 4;
        let mut pl = RealFftPlanner::<f32>::new();
        let fwd = pl.plan_fft_forward(n);
        let inv = pl.plan_fft_inverse(n);
        let bins = n / 2 + 1;
        let scratch_f = fwd.make_scratch_vec();
        let scratch_i = inv.make_scratch_vec();
        // periodic sqrt-Hann for analysis and synthesis (Hann sums to 2 at 4x overlap)
        let win = (0..n).map(|i| (0.5 - 0.5 * (std::f32::consts::TAU * i as f32 / n as f32).cos()).sqrt()).collect();
        let z = || vec![0.0f32; n];
        let zc = || vec![Complex::new(0.0f32, 0.0); bins];
        Self {
            sr,
            n,
            hop,
            win,
            inb: [z(), z()],
            acc: [z(), z()],
            outq: [vec![0.0; hop], vec![0.0; hop]],
            pos: 0,
            fwd,
            inv,
            tbuf: z(),
            spec: [zc(), zc()],
            last: [zc(), zc()],
            scratch_f,
            scratch_i,
            rng: Rng(0x10_55_ee_01),
            lost: 0,
            var: 0,
            bitrate: 0.5,
            ploss: 0.0,
            joint: 0.4,
            stretch: 0.0,
            blur: 0.0,
            formant: 0.0,
            prev_mag: [vec![0.0; bins], vec![0.0; bins]],
            env_buf: vec![0.0; bins],
            held: 0,
            flutter: false,
            fl: [Line::new(4096), Line::new(4096)],
            fph: 0.0,
            fph2: 0.0,
        }
    }
    pub fn latency(&self) -> usize {
        self.n
    }
    pub fn reset(&mut self) {
        for c in 0..2 {
            self.inb[c].iter_mut().for_each(|x| *x = 0.0);
            self.acc[c].iter_mut().for_each(|x| *x = 0.0);
            self.outq[c].iter_mut().for_each(|x| *x = 0.0);
            self.last[c].iter_mut().for_each(|x| *x = Complex::new(0.0, 0.0));
        }
        self.pos = 0;
        self.lost = 0;
        self.fl.iter_mut().for_each(|l| l.clear());
    }
    pub fn control(&mut self, k: &[f32; MAX_K], var: usize) {
        self.var = var.min(2);
        self.bitrate = k[0];
        self.ploss = k[1];
        self.joint = k[2];
        self.stretch = k[3];
        self.blur = k[4];
        self.formant = k[5];
    }

    #[inline]
    pub fn tick(&mut self, l: f32, r: f32) -> (f32, f32) {
        let y = (self.outq[0][self.pos], self.outq[1][self.pos]);
        let w = self.n - self.hop + self.pos;
        // mid / side in
        self.inb[0][w] = 0.5 * (l + r);
        self.inb[1][w] = 0.5 * (l - r);
        self.pos += 1;
        if self.pos == self.hop {
            self.pos = 0;
            self.frame();
        }
        let (a, b) = (y.0 + y.1, y.0 - y.1);
        if !self.flutter {
            return (a, b);
        }
        // wow + flutter: a few hundred microseconds of wander
        self.fph = (self.fph + 6.3 / self.sr).fract();
        self.fph2 = (self.fph2 + 0.7 / self.sr).fract();
        let d = 0.002 * self.sr + self.sr * (0.00025 * (std::f32::consts::TAU * self.fph).sin() + 0.0006 * (std::f32::consts::TAU * self.fph2).sin());
        self.fl[0].push(a);
        self.fl[1].push(b);
        (self.fl[0].frac(d), self.fl[1].frac(d))
    }

    fn frame(&mut self) {
        let (n, hop) = (self.n, self.hop);
        let bins = n / 2 + 1;
        let hz_per_bin = self.sr / n as f32;
        // ---- what this frame's "encoder" keeps
        let b = self.bitrate;
        let voip = self.var == 2;
        let top = if voip { 3400.0f32.min(16500.0 * 0.17f32.powf(b)) } else { 16500.0 * 0.17f32.powf(b) };
        let bottom = if voip { 300.0 } else { 0.0 };
        let keep_db = 72.0 - 64.0 * b.powf(0.7);
        let step_rel = 0.002 + 0.3 * b * b;
        let joint_hz = loss_joint_hz(self.joint);
        // ---- packet loss: runs of 1..8 frames
        if self.lost > 0 {
            self.lost -= 1;
        } else if self.rng.u() < self.ploss * self.ploss * 0.5 {
            self.lost = 1 + (self.rng.u() * if self.var == 1 { 8.0 } else { 4.0 }) as u32;
        }
        let lost = self.lost > 0;
        // STRETCH: the jitter buffer stretches a frame now and then (held, phase rolling)
        if self.held > 0 {
            self.held -= 1;
        } else if !lost && self.rng.u() < self.stretch * 0.25 {
            self.held = 1 + (self.rng.u() * 3.0 * self.stretch) as u32;
        }
        let held = self.held > 0 && !lost;

        for c in 0..2 {
            for i in 0..n {
                self.tbuf[i] = self.inb[c][i] * self.win[i];
            }
            let _ = self.fwd.process_with_scratch(&mut self.tbuf, &mut self.spec[c], &mut self.scratch_f);
            let sp = &mut self.spec[c];
            if held {
                for (k, v) in sp.iter_mut().enumerate() {
                    let rot = Complex::from_polar(1.0, std::f32::consts::TAU * (k * hop) as f32 / n as f32);
                    self.last[c][k] *= rot;
                    *v = self.last[c][k];
                }
            } else if lost {
                match self.var {
                    // STREAM: the last good packet again, phases rolling on
                    1 => {
                        for (k, v) in sp.iter_mut().enumerate() {
                            let rot = Complex::from_polar(1.0, std::f32::consts::TAU * (k * hop) as f32 / n as f32);
                            self.last[c][k] *= rot;
                            *v = self.last[c][k];
                        }
                    }
                    // VOIP: comfort noise shaped like the last packet
                    2 => {
                        for (k, v) in sp.iter_mut().enumerate() {
                            let m = self.last[c][k].norm() * 0.15;
                            *v = Complex::from_polar(m, self.rng.u() * std::f32::consts::TAU);
                        }
                    }
                    // MP3: a gap
                    _ => sp.iter_mut().for_each(|v| *v = Complex::new(0.0, 0.0)),
                }
            } else {
                // band-by-band: anything far below the band's loudest bin is gone,
                // the rest is quantised (coarser as BITRATE drops)
                let mut lo = 1usize;
                while lo < bins {
                    let hi = ((lo as f32 * 1.35) as usize).max(lo + 2).min(bins);
                    let mut mmax = 0.0f32;
                    for v in &sp[lo..hi] {
                        mmax = mmax.max(v.norm());
                    }
                    let thr = mmax * 10f32.powf(-keep_db / 20.0) * if c == 1 { 2.0 } else { 1.0 };
                    let step = mmax * step_rel * if c == 1 { 2.0 } else { 1.0 };
                    for k in lo..hi {
                        let f = k as f32 * hz_per_bin;
                        let v = sp[k];
                        let m = v.norm();
                        let cut = f > top || f < bottom || (c == 1 && f > joint_hz) || m < thr;
                        sp[k] = if cut || m <= 0.0 {
                            Complex::new(0.0, 0.0)
                        } else if step > 0.0 {
                            let q = ((m / step) + 0.5 * (self.rng.u() - 0.5)).round() * step;
                            v * (q / m)
                        } else {
                            v
                        };
                    }
                    lo = hi;
                }
                // BLUR: the spectrum smears over time
                // FORMANT: magnitudes flattened to their envelope, phases loosened (the
                // robotic / whispery voice of very low bitrate speech codecs)
                if self.blur > 0.005 || self.formant > 0.005 {
                    let f = self.formant;
                    if f > 0.005 {
                        let w = 6usize;
                        for k in 0..bins {
                            let (a, b) = (k.saturating_sub(w), (k + w + 1).min(bins));
                            let mut acc = 0.0;
                            for v in &sp[a..b] {
                                acc += v.norm();
                            }
                            self.env_buf[k] = acc / (b - a) as f32;
                        }
                    }
                    let bl = self.blur * 0.92;
                    for k in 1..bins - 1 {
                        let v = sp[k];
                        let m = v.norm();
                        let mut t = m;
                        if f > 0.005 {
                            t = m + (self.env_buf[k] - m) * f;
                        }
                        let pm = self.prev_mag[c][k];
                        if bl > 0.0 {
                            t += (pm - t) * bl;
                        }
                        self.prev_mag[c][k] = t;
                        let ph = if f > 0.005 { v.arg() + f * std::f32::consts::PI * self.rng.b() } else { v.arg() };
                        sp[k] = Complex::from_polar(t, ph);
                    }
                }
                sp[0] = Complex::new(0.0, 0.0);
                sp[bins - 1] = Complex::new(0.0, 0.0);
                self.last[c].copy_from_slice(sp);
            }
            let _ = self.inv.process_with_scratch(&mut self.spec[c], &mut self.tbuf, &mut self.scratch_i);
            let g = 0.5 / n as f32;
            for i in 0..n {
                self.acc[c][i] += self.tbuf[i] * self.win[i] * g;
            }
            // hand out the finished hop, slide everything along
            self.outq[c].copy_from_slice(&self.acc[c][..hop]);
            self.acc[c].copy_within(hop.., 0);
            for x in self.acc[c][n - hop..].iter_mut() {
                *x = 0.0;
            }
            self.inb[c].copy_within(hop.., 0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// BITRATE at its best and no loss: close to transparent (delayed by latency()).
    #[test]
    fn best_quality_is_nearly_clean() {
        let sr = 48000.0;
        let mut x = Loss::new(sr);
        let mut k = [0.0; MAX_K];
        k[2] = 0.0;
        x.control(&k, 0);
        let lat = x.latency();
        let inp: Vec<f32> = (0..24000).map(|i| 0.3 * (std::f32::consts::TAU * 440.0 * i as f32 / sr).sin()).collect();
        let out: Vec<f32> = inp.iter().map(|&v| x.tick(v, v).0).collect();
        let err = (4000..24000).map(|i| (out[i] - inp[i - lat]).abs()).fold(0.0f32, f32::max);
        assert!(err < 0.03, "err {err}");
    }
}
