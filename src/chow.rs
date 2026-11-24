//! CHOW tape engine for MAGNETIC: magnetic hysteresis + playback loss.
//!
//! Ported from ChowTape / AnalogTapeModel by Jatin Chowdhury (GPLv3):
//! https://github.com/jatinchowdhury18/AnalogTapeModel
//!   Hysteresis: Plugin/Source/Processors/Hysteresis/HysteresisOps.h, HysteresisProcessing.{h,cpp}
//!     (Jiles-Atherton model, alpha-transform derivative, RK4 solver), makeup from HysteresisProcessor.
//!   Loss: Plugin/Source/Processors/Loss_Effects/LossFilter.cpp (spacing, thickness and gap loss,
//!     head bump). ChowTape builds the loss as a linear-phase FIR; here its response is matched by a
//!     zero-latency 2nd-order low-pass at the same -3 dB point, so PATINA's latency doesn't change.
//! Paper: https://ccrma.stanford.edu/~jatin/420/tape/TapeModel_DAFx.pdf

const ALPHA: f64 = 1.6e-3;

#[derive(Clone, Copy)]
pub struct Hyst {
    t: f64,
    m_n1: f64,
    h_n1: f64,
    hd_n1: f64,
    // cooked parameters
    ms: f64,
    a: f64,
    k: f64,
    nc: f64,
    ms_oa_tc: f64,
    ms_oa_tc_talpha: f64,
}

impl Hyst {
    pub fn new(sr: f32) -> Self {
        let mut h = Self { t: 1.0 / sr as f64, m_n1: 0.0, h_n1: 0.0, hd_n1: 0.0, ms: 1.0, a: 0.25, k: 0.47875, nc: 0.83, ms_oa_tc: 0.0, ms_oa_tc_talpha: 0.0 };
        h.cook(0.5, 0.5, 0.5);
        h
    }
    pub fn reset(&mut self) {
        self.m_n1 = 0.0;
        self.h_n1 = 0.0;
        self.hd_n1 = 0.0;
    }
    /// drive, width (1 - bias), saturation: all 0..1, as in ChowTape.
    pub fn cook(&mut self, drive: f32, width: f32, sat: f32) {
        let (drive, width, sat) = (drive as f64, width.clamp(0.0, 0.99) as f64, sat as f64);
        self.ms = 0.5 + 1.5 * (1.0 - sat);
        self.a = self.ms / (0.01 + 6.0 * drive);
        let c = (1.0 - width).sqrt() - 0.01;
        self.k = 0.47875;
        self.nc = 1.0 - c;
        let ms_oa = self.ms / self.a;
        self.ms_oa_tc = c * ms_oa;
        self.ms_oa_tc_talpha = ALPHA * self.ms_oa_tc;
    }
    /// ChowTape's output makeup for these settings.
    pub fn makeup(width: f32, sat: f32) -> f32 {
        (1.0 + 0.6 * width) / (0.5 + 1.5 * (1.0 - sat))
    }

    /// dM/dt (HysteresisOps::hysteresisFunc)
    #[inline]
    fn func(&self, m: f64, h: f64, hd: f64) -> f64 {
        let q = (h + m * ALPHA) / self.a;
        let (lang, lang_d) = if q.abs() < 0.001 {
            (q / 3.0, 1.0 / 3.0)
        } else {
            let coth = 1.0 / q.tanh();
            let oq = 1.0 / q;
            (coth - oq, oq * oq - coth * coth + 1.0)
        };
        let m_diff = lang * self.ms - m;
        let delta = if hd >= 0.0 { 1.0 } else { -1.0 };
        let same = (delta > 0.0) == (m_diff > 0.0) && m_diff != 0.0;
        let kap1 = if same { self.nc } else { 0.0 };
        let f1_denom = self.nc * delta * self.k - ALPHA * m_diff;
        let f1 = kap1 * m_diff / f1_denom;
        let f2 = lang_d * self.ms_oa_tc;
        let f3 = 1.0 - lang_d * self.ms_oa_tc_talpha;
        hd * (f1 + f2) / f3
    }

    /// One sample: H in (the clipped, driven signal), M out (RK4).
    #[inline]
    pub fn tick(&mut self, x: f32) -> f32 {
        let h = x as f64;
        const DA: f64 = 0.75;
        let mut hd = ((1.0 + DA) / self.t) * (h - self.h_n1) - DA * self.hd_n1;
        let (h12, hd12) = ((h + self.h_n1) * 0.5, (hd + self.hd_n1) * 0.5);
        let k1 = self.func(self.m_n1, self.h_n1, self.hd_n1) * self.t;
        let k2 = self.func(self.m_n1 + k1 * 0.5, h12, hd12) * self.t;
        let k3 = self.func(self.m_n1 + k2 * 0.5, h12, hd12) * self.t;
        let k4 = self.func(self.m_n1 + k3, h, hd) * self.t;
        let mut m = self.m_n1 + k1 / 6.0 + k2 / 3.0 + k3 / 3.0 + k4 / 6.0;
        // instability guard (as ChowTape)
        if !m.is_finite() || m.abs() > 20.0 {
            m = 0.0;
            hd = 0.0;
        }
        self.m_n1 = m;
        self.h_n1 = h;
        self.hd_n1 = hd;
        m as f32
    }
}

/// Playback loss |H(f)| for tape speed (inches/s), spacing, thickness, gap (microns).
pub fn loss_mag(f: f32, speed_ips: f32, spacing_um: f32, thick_um: f32, gap_um: f32) -> f32 {
    let k = std::f32::consts::TAU * f.max(20.0) / (speed_ips * 0.0254);
    let mut h = (-k * spacing_um * 1e-6).exp();
    let kt = k * thick_um * 1e-6;
    h *= (1.0 - (-kt).exp()) / kt;
    let kg = k * gap_um * 1e-6 / 2.0;
    h *= (kg.sin() / kg).abs();
    h
}

/// Frequency where the loss reaches -3 dB (searched up to `top`).
pub fn loss_corner(speed_ips: f32, spacing_um: f32, thick_um: f32, gap_um: f32, top: f32) -> f32 {
    let (mut lo, mut hi) = (100.0f32.ln(), top.ln());
    if loss_mag(top, speed_ips, spacing_um, thick_um, gap_um) > 0.7071 {
        return top;
    }
    for _ in 0..30 {
        let mid = 0.5 * (lo + hi);
        if loss_mag(mid.exp(), speed_ips, spacing_um, thick_um, gap_um) > 0.7071 {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    (0.5 * (lo + hi)).exp()
}

/// Head bump (freq Hz, gain dB), as ChowTape's calcHeadBumpFilter.
pub fn head_bump(speed_ips: f32, gap_um: f32) -> (f32, f32) {
    let f = speed_ips * 0.0254 / (gap_um * 1e-6 * 500.0);
    let g = (1.5 * (1000.0 - (f - 100.0).abs()) / 1000.0).max(1.0);
    (f.clamp(20.0, 400.0), 20.0 * g.log10())
}

/// Machines for CHOW CASSETTE / REEL / VHS:
/// (speed ips, gap um, thickness um, saturation, width (1 - bias), input clip, spacing at full WEAR um)
/// Corners with clean tape: cassette ~12 kHz, reel full range, VHS ~5.5 kHz; full WEAR ~1.5 kHz.
pub const MACHINES: [(f32, f32, f32, f32, f32, f32, f32); 3] = [(1.875, 1.0, 0.1, 0.55, 0.45, 10.0, 1.7), (15.0, 1.0, 0.1, 0.4, 0.6, 12.5, 12.0), (1.3, 2.0, 0.1, 0.7, 0.3, 8.0, 1.1)];

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn chow_hyst_is_stable_and_saturates() {
        let mut h = Hyst::new(48000.0);
        h.cook(0.6, 0.5, 0.5);
        let mk = Hyst::makeup(0.5, 0.5);
        let mut pk_small = 0.0f32;
        let mut pk_big = 0.0f32;
        for n in 0..48000 {
            let s = (std::f32::consts::TAU * 100.0 * n as f32 / 48000.0).sin();
            let y = h.tick(s * 0.05) * mk;
            assert!(y.is_finite());
            if n > 4800 {
                pk_small = pk_small.max(y.abs());
            }
        }
        h.reset();
        for n in 0..48000 {
            let s = (std::f32::consts::TAU * 100.0 * n as f32 / 48000.0).sin();
            let y = h.tick(s * 2.0) * mk;
            assert!(y.is_finite());
            if n > 4800 {
                pk_big = pk_big.max(y.abs());
            }
        }
        println!("small in 0.05 -> {pk_small:.4}, big in 2.0 -> {pk_big:.4} (gain {:.2} vs {:.2})", pk_small / 0.05, pk_big / 2.0);
        assert!(pk_big / 2.0 < pk_small / 0.05, "big signals should be squashed more");
        for (sp, gap, th, ..) in MACHINES {
            println!("machine {sp} ips: corner {:.0} Hz, bump {:?}", loss_corner(sp, 0.1, th, gap, 22000.0), head_bump(sp, gap));
        }
    }
}
