//! Patina — awk studios. A colour rack: up to 8 modules in any order, then the
//! ECHO strip, then the finishing stages.
//!
//! in -> global FOCUS split -> slot 1 .. slot 8 -> ECHO -> (FOCUS rest back in)
//!    -> global CUT -> TONE -> COMPRESS -> loudness match -> OUTPUT -> MIX -> SAFE
//!
//! Every module has FOCUS (only its range is touched) or CUT (outside its range is
//! removed), FLUX (random movement) and an ECHO SEND: a twin of the module that only
//! processes the echo's repeats, so each trip round the echo goes through it again.
//! DISTORT and COMPRESS are oversampled and the master COMPRESS too; the latency of
//! those three is always reported (held constant); LOSS/CODEC adds a frame while
//! it's in the rack.

use nice_plug::prelude::*;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

mod analyzer;
mod awk_ui;
pub mod comp;
pub mod dsp;

mod editor;
mod nice_app;
pub mod fx;
pub mod loss;
pub mod master;
mod oversample;
pub mod params;
mod presets;
pub mod sat;
pub mod tame;
pub mod verb;

use dsp::*;
use fx::*;
pub use params::*;
use sat::{Blk, Sat, Split};

/// What the editor reads.
pub struct Shared {
    pub meters: awk_ui::Meters,
    pub spec_in: awk_ui::SpecRing,
    pub spec_out: awk_ui::SpecRing,
    pub bpm: AtomicU32,
    /// TAME cut per band (dB)
    pub cut: [AtomicU32; tame::BANDS],
    /// FLUX position per module (-1..1 x amount), for the display
    pub flux: [AtomicU32; N_MODS],
    /// current latency (samples)
    pub lat: AtomicU32,
    /// ECHO output level (peak, linear)
    pub e_vu: AtomicU32,
    /// gain reduction (dB): COMPRESS module, master COMPRESS
    pub comp_gr: AtomicU32,
    pub mcomp_gr: AtomicU32,
}
impl Default for Shared {
    fn default() -> Self {
        Self {
            meters: awk_ui::Meters::default(),
            spec_in: awk_ui::SpecRing::new(analyzer::FFT),
            spec_out: awk_ui::SpecRing::new(analyzer::FFT),
            bpm: AtomicU32::new(120f32.to_bits()),
            cut: std::array::from_fn(|_| AtomicU32::new(0)),
            flux: std::array::from_fn(|_| AtomicU32::new(0)),
            lat: AtomicU32::new(0),
            e_vu: AtomicU32::new(0),
            comp_gr: AtomicU32::new(0),
            mcomp_gr: AtomicU32::new(0),
        }
    }
}
pub fn get(a: &AtomicU32) -> f32 {
    f32::from_bits(a.load(Ordering::Relaxed))
}
fn put(a: &AtomicU32, v: f32) {
    a.store(v.to_bits(), Ordering::Relaxed);
}

const MAX_LAT: usize = 64;
const MOD_LAT: usize = 1100;
const STEP: usize = 32;

#[derive(Clone, Copy, Debug)]
pub struct ModState {
    pub on: bool,
    pub mix: f32,
    pub var: usize,
    pub lo: f32,
    pub hi: f32,
    pub k: [f32; MAX_K],
    pub flux: f32,
    /// CUT instead of FOCUS
    pub cut: bool,
    /// ECHO send
    pub esend: f32,
}
impl ModState {
    pub fn default_for(m: usize) -> Self {
        let d = &MODS[m];
        let mut k = [0.0; MAX_K];
        for (i, &(_, v)) in d.knobs.iter().enumerate() {
            k[i] = v;
        }
        Self { on: true, mix: d.mix, var: 0, lo: 20.0, hi: 20000.0, k, flux: 0.0, cut: false, esend: 0.0 }
    }
}

/// ECHO strip, LOSS toggles, global FOCUS / CUT, TONE, COMPRESS.
#[derive(Clone, Copy, Debug)]
pub struct Extra {
    pub e_on: bool,
    pub e_ping: bool,
    pub e_scrub: bool,
    pub e_duck: f32,
    pub e_smooth: f32,
    pub e_pitch: f32,
    pub mag_wear: bool,
    pub mag_flut: bool,
    pub cod_wear: bool,
    pub cod_flut: bool,
    pub g_lo: f32,
    pub g_hi: f32,
    pub g_cut: bool,
    pub g_shape: i32,
    pub tone: f32,
    pub tone_mode: i32,
    pub comp: f32,
    pub comp_model: usize,
}
impl Default for Extra {
    fn default() -> Self {
        Self { e_on: false, e_ping: false, e_scrub: true, e_duck: 0.0, e_smooth: 0.0, e_pitch: 0.0, mag_wear: true, mag_flut: true, cod_wear: true, cod_flut: false, g_lo: 20.0, g_hi: 20000.0, g_cut: false, g_shape: 2, tone: 0.0, tone_mode: 0, comp: 0.0, comp_model: 0 }
    }
}

/// One block's settings (the plugin reads them from the params; tests build them).
#[derive(Clone, Copy, Debug)]
pub struct State {
    /// slot -> module + 1 (0 = empty)
    pub chain: [usize; SLOTS],
    pub m: [ModState; N_MODS],
    pub amount: f32,
    pub out_db: f32,
    pub mix: f32,
    pub safe: bool,
    pub bypass: bool,
    pub os: i32,
    pub in_db: f32,
    pub matching: bool,
    pub x: Extra,
}
impl Default for State {
    fn default() -> Self {
        Self { chain: [0; SLOTS], m: std::array::from_fn(ModState::default_for), amount: 1.0, out_db: 0.0, mix: 1.0, safe: true, bypass: false, os: 1, in_db: 0.0, matching: false, x: Extra::default() }
    }
}
impl State {
    /// just these modules, in this order (ECHO in the list = the ECHO strip on)
    pub fn with(mods: &[usize]) -> Self {
        let mut s = Self::default();
        for (i, &m) in mods.iter().enumerate().take(SLOTS) {
            s.chain[i] = m + 1;
        }
        s
    }
    pub fn from_params(p: &PatinaParams) -> Self {
        let x = Extra {
            e_on: p.e_on.value(),
            e_ping: p.e_ping.value(),
            e_scrub: p.e_scrub.value(),
            e_duck: p.e_duck.value(),
            e_smooth: p.e_smooth.value(),
            e_pitch: p.e_pitch.value(),
            mag_wear: p.mag_wear.value(),
            mag_flut: p.mag_flut.value(),
            cod_wear: p.cod_wear.value(),
            cod_flut: p.cod_flut.value(),
            g_lo: p.g_lo.value(),
            g_hi: p.g_hi.value(),
            g_cut: p.g_cut.value(),
            g_shape: p.g_shape.value(),
            tone: p.tone.value(),
            tone_mode: p.tone_mode.value(),
            comp: p.m_comp.value(),
            comp_model: p.comp_model.value().clamp(0, 5) as usize,
        };
        let mut s = Self { chain: [0; SLOTS], m: std::array::from_fn(ModState::default_for), amount: p.amount.value(), out_db: p.out.value(), mix: p.mix.value(), safe: p.safe.value(), bypass: p.bypass.value(), os: p.os.value(), in_db: p.input.value(), matching: p.matching.value(), x };
        for (i, c) in p.slots().iter().enumerate() {
            s.chain[i] = c.value().clamp(0, N_MODS as i32) as usize;
        }
        for m in 0..N_MODS {
            let q = p.m(m);
            let ks = q.knobs();
            s.m[m] = ModState { on: q.on.value(), mix: q.mix.value(), var: q.var.value().max(0) as usize, lo: q.lo.value(), hi: q.hi.value(), k: std::array::from_fn(|i| ks[i].value()), flux: q.flux.value(), cut: q.cut.value(), esend: q.esend.value() };
        }
        s
    }
    /// the rack modules in play, in order, each once (ECHO lives in its strip)
    pub fn order(&self) -> ([usize; SLOTS], usize) {
        let mut out = [0; SLOTS];
        let mut n = 0;
        for &c in self.chain.iter() {
            if c == 0 || c - 1 == M_ECHO {
                continue;
            }
            let m = c - 1;
            if !out[..n].contains(&m) {
                out[n] = m;
                n += 1;
            }
        }
        (out, n)
    }
    /// ECHO strip on (old presets put ECHO in a slot: that turns it on too)
    pub fn echo_on(&self) -> bool {
        (self.x.e_on || self.chain.contains(&(M_ECHO + 1))) && self.m[M_ECHO].on
    }
}

/// Host transport for this block.
#[derive(Clone, Copy, Debug, Default)]
pub struct Transport {
    pub bpm: Option<f64>,
    pub beat: Option<f64>,
    pub playing: bool,
}

/// One full set of module processors. The engine has two: the rack, and a twin
/// that only ever hears the echo's repeats (ECHO SEND).
struct Mods {
    noise: Noise,
    crush: Crush,
    mag: Magnetic,
    chop: Chop,
    wob: Wobble,
    verb: verb::Reverb,
    chorus: Chorus,
    time: Time,
    rev: Reverse,
    filt: Filter,
    width: Width,
    heat: Sat,
    heat_blk: Blk,
    heat_drive: f32,
    heat_tone: f32,
    tame: tame::Tame,
    tame_set: tame::Settings,
    loss: loss::Loss,
    comp: comp::Comp,
    split: Vec<Split>,
}
impl Mods {
    fn new(sr: f32, f: usize) -> Self {
        Self {
            noise: Noise::new(sr),
            crush: Crush::new(sr),
            mag: Magnetic::new(sr),
            chop: Chop::new(sr),
            wob: Wobble::new(sr),
            verb: verb::Reverb::new(sr),
            chorus: Chorus::new(sr),
            time: Time::new(sr),
            rev: Reverse::new(sr),
            filt: Filter::new(sr),
            width: Width::new(sr),
            heat: Sat::new(sr, f),
            heat_blk: Blk::new(0, 0, 0.35, 0.0),
            heat_drive: 1.0,
            heat_tone: 0.0,
            tame: tame::Tame::new(sr),
            tame_set: tame::Settings { amount: 0.5, sharp: 0.6, speed: 0.5, lo: 20.0, hi: 20000.0 },
            loss: loss::Loss::new(sr),
            comp: comp::Comp::new(sr, f),
            split: (0..N_MODS).map(|_| Split::new(sr)).collect(),
        }
    }
    fn reset(&mut self, m: usize) {
        match m {
            M_NOISE => self.noise.reset(),
            M_CRUSH => self.crush.reset(),
            M_MAGNETIC => self.mag.reset(),
            M_CHOP => self.chop.reset(),
            M_WOBBLE => self.wob.reset(),
            M_REVERB => self.verb.reset(),
            M_CHORUS => self.chorus.reset(),
            M_TIME => self.time.reset(),
            M_REVERSE => self.rev.reset(),
            M_HEAT => self.heat.clear(),
            M_TAME => self.tame.clear(),
            M_FILTER => self.filt.reset(),
            M_LOSS => self.loss.reset(),
            M_COMP => self.comp.reset(),
            M_WIDTH => self.width.reset(),
            _ => {}
        }
        self.split[m].clear();
    }
    fn control(&mut self, m: usize, k: &[f32; MAX_K], ms: &ModState, c: &Ctx, cod_flut: bool) {
        let v = ms.var;
        match m {
            M_NOISE => {
                self.noise.control(k, v);
                self.noise.set_range(ms.lo, ms.hi);
            }
            M_CRUSH => self.crush.control(k, v),
            M_MAGNETIC => self.mag.control(k, v),
            M_CHOP => self.chop.control(k, v, c),
            M_WOBBLE => self.wob.control(k, v),
            M_REVERB => self.verb.control(k, v),
            M_CHORUS => self.chorus.control(k, v),
            M_TIME => self.time.control(k, v, c),
            M_REVERSE => self.rev.control(k, v, c),
            M_HEAT => {
                let (ty, var) = HEAT_MAP[v.min(HEAT_MAP.len() - 1)];
                self.heat_blk = Blk::new(ty, var, k[1], 0.0);
                self.heat_drive = db(k[0] * 24.0 + k[3] * 18.0);
                self.heat_tone = (k[2] - 0.5) * 12.0;
            }
            M_TAME => self.tame_set = tame::Settings { amount: k[0], sharp: k[1], speed: k[2], lo: ms.lo, hi: ms.hi },
            M_FILTER => self.filt.control(k, v, c),
            M_LOSS => {
                self.loss.control(k, v);
                self.loss.flutter = cod_flut;
            }
            M_COMP => self.comp.control(k, v.min(5)),
            M_WIDTH => self.width.control(k),
            _ => {}
        }
        if m != M_NOISE && m != M_TAME {
            self.split[m].set(ms.lo, ms.hi);
        }
    }
    #[inline]
    fn tick(&mut self, m: usize, l: f32, r: f32, c: &Ctx) -> (f32, f32) {
        match m {
            M_NOISE => self.noise.tick(l, r),
            M_CRUSH => self.crush.tick(l, r),
            M_MAGNETIC => self.mag.tick(l, r),
            M_CHOP => self.chop.tick(l, r, c),
            M_WOBBLE => self.wob.tick(l, r, c),
            M_REVERB => self.verb.tick(l, r),
            M_CHORUS => self.chorus.tick(l, r),
            M_TIME => self.time.tick(l, r, c),
            M_REVERSE => self.rev.tick(l, r, c),
            M_HEAT => self.heat.tick(l, r, &self.heat_blk, self.heat_drive, self.heat_tone, true),
            M_TAME => self.tame.tick(l, r, &self.tame_set),
            M_FILTER => self.filt.tick(l, r),
            M_LOSS => self.loss.tick(l, r),
            M_COMP => self.comp.tick(l, r),
            M_WIDTH => self.width.tick(l, r),
            _ => (l, r),
        }
    }
}

pub struct Engine {
    pub sr: f32,
    n: u64,
    beat: f64,
    main: Mods,
    twin: Box<Mods>,
    tw_on: [bool; N_MODS],
    echo: Echo,
    ex: EchoX,
    e_act: f32,
    e_mix: f32,
    duck_env: f32,
    e_vu: f32,
    flux: [Flux; N_MODS],
    /// FLUX position per module (for the display)
    pub flux_v: [f32; N_MODS],
    in_sm: f32,
    env_in: f32,
    env_out: f32,
    match_g: f32,
    /// one oversampler's latency (DISTORT, COMPRESS and the master COMPRESS each)
    os_lat: usize,
    llat: usize,
    /// total latency right now
    pub lat: usize,
    mdd: Vec<[awk_ui::DryDelay; 2]>,
    mcd: Vec<[awk_ui::DryDelay; 2]>,
    end_dd: [awk_ui::DryDelay; 2],
    dry_dd: [awk_ui::DryDelay; 2],
    act: [f32; N_MODS],
    mixs: [f32; N_MODS],
    ksm: [[f32; MAX_K]; N_MODS],
    fresh: [bool; N_MODS],
    chain: [usize; SLOTS],
    n_chain: usize,
    fade: f32,
    out_sm: f32,
    // global FOCUS / CUT, TONE, COMPRESS
    gsplit: Split,
    g_dd: [awk_ui::DryDelay; 2],
    cutf: master::CutF,
    tone: master::Tone,
    mcomp: comp::Comp,
    mc_dd: [awk_ui::DryDelay; 2],
    mc_mix: f32,
    byp: awk_ui::BypassRamp,
    ceil: awk_ui::Ceiling,
    meter: awk_ui::PeakFollower,
    pub shared: Arc<Shared>,
}

fn dd2(n: usize) -> [awk_ui::DryDelay; 2] {
    [awk_ui::DryDelay::new(n), awk_ui::DryDelay::new(n)]
}

impl Engine {
    pub fn new(sr: f32, os: i32) -> Self {
        let f = oversample::factor_for(os);
        let main = Mods::new(sr, f);
        let llat = main.loss.latency();
        let mut e = Self {
            sr,
            n: 0,
            beat: 0.0,
            main,
            twin: Box::new(Mods::new(sr, f)),
            tw_on: [false; N_MODS],
            echo: Echo::new(sr),
            ex: EchoX::default(),
            e_act: 0.0,
            e_mix: 0.0,
            duck_env: 0.0,
            e_vu: 0.0,
            flux: std::array::from_fn(|i| Flux::new(0x9e37_79b9u32.wrapping_mul(i as u32 + 7))),
            flux_v: [0.0; N_MODS],
            in_sm: 1.0,
            env_in: 0.0,
            env_out: 0.0,
            match_g: 1.0,
            os_lat: oversample::latency(f),
            llat,
            lat: 3 * oversample::latency(f),
            mdd: (0..N_MODS).map(|_| dd2(MOD_LAT)).collect(),
            mcd: (0..N_MODS).map(|_| dd2(MOD_LAT)).collect(),
            end_dd: dd2(MAX_LAT * 2),
            dry_dd: dd2(MAX_LAT * 3 + MOD_LAT),
            act: [0.0; N_MODS],
            mixs: [0.0; N_MODS],
            ksm: [[0.0; MAX_K]; N_MODS],
            fresh: [true; N_MODS],
            chain: [0; SLOTS],
            n_chain: 0,
            fade: 1.0,
            out_sm: 1.0,
            gsplit: Split::new(sr),
            g_dd: dd2(MAX_LAT * 2 + MOD_LAT),
            cutf: master::CutF::default(),
            tone: master::Tone::default(),
            mcomp: comp::Comp::new(sr, f),
            mc_dd: dd2(MAX_LAT),
            mc_mix: 0.0,
            byp: awk_ui::BypassRamp::default(),
            ceil: awk_ui::Ceiling::default(),
            meter: awk_ui::PeakFollower::default(),
            shared: Arc::new(Shared::default()),
        };
        e.byp.set_sr(sr);
        e.ceil.set_sr(sr);
        e.meter.set_sr(sr);
        e.shared.spec_in.sr.store(sr.to_bits(), Ordering::Relaxed);
        e.shared.spec_out.sr.store(sr.to_bits(), Ordering::Relaxed);
        e
    }
    pub fn with_shared(mut self, s: Arc<Shared>) -> Self {
        s.spec_in.sr.store(self.sr.to_bits(), Ordering::Relaxed);
        s.spec_out.sr.store(self.sr.to_bits(), Ordering::Relaxed);
        self.shared = s;
        self
    }
    pub fn os_factor(&self) -> usize {
        self.main.heat.factor()
    }

    fn reset_mod(&mut self, m: usize) {
        if m == M_ECHO {
            self.echo.reset();
        }
        self.main.reset(m);
        self.twin.reset(m);
        self.mdd[m].iter_mut().chain(self.mcd[m].iter_mut()).for_each(|d| d.clear());
        self.fresh[m] = true;
    }
    pub fn reset(&mut self) {
        for m in 0..N_MODS {
            self.reset_mod(m);
        }
        self.echo.reset();
        for d in self.end_dd.iter_mut().chain(self.dry_dd.iter_mut()).chain(self.g_dd.iter_mut()).chain(self.mc_dd.iter_mut()) {
            d.clear();
        }
        self.gsplit.clear();
        self.cutf.clear();
        self.tone.clear();
        self.mcomp.reset();
    }

    /// Delay a module adds (and the dry around it gets).
    fn lat_of(&self, m: usize) -> usize {
        match m {
            M_HEAT | M_COMP => self.os_lat,
            M_LOSS => self.llat,
            _ => 0,
        }
    }
    /// The rack (slots) is always 2 oversamplers long, plus CODEC's frame while it's in.
    fn rack_lat(&self, chain: &[usize]) -> usize {
        2 * self.os_lat + if chain.contains(&M_LOSS) { self.llat } else { 0 }
    }
    /// Latency for a chain: the rack plus the master COMPRESS.
    pub fn lat_for(&self, chain: &[usize]) -> usize {
        self.rack_lat(chain) + self.os_lat
    }
    /// The latency the plugin should report for these settings.
    pub fn lat_for_state(&self, st: &State) -> usize {
        let (o, n) = st.order();
        self.lat_for(&o[..n])
    }

    /// Smoothed knobs + FLUX + LOSS toggles for module `m`.
    fn knobs(&mut self, m: usize, ms: &ModState, x: &Extra) -> [f32; MAX_K] {
        let mut kk = self.ksm[m];
        let fv = self.flux[m].step(STEP as f32 / self.sr, ms.flux);
        self.flux_v[m] = fv * ms.flux;
        if ms.flux > 0.001 {
            let (i, d) = MODS[m].flux;
            kk[i] = (kk[i] + ms.flux * d * fv).clamp(0.0, 1.0);
        }
        if m == M_MAGNETIC {
            if !x.mag_wear {
                kk[0] = 0.0;
                kk[1] = 0.0;
            }
            if !x.mag_flut {
                kk[3] = 0.0;
            }
        }
        if m == M_LOSS && !x.cod_wear {
            kk[1] = 0.0;
        }
        kk
    }
    fn smooth_knobs(&mut self, m: usize, ms: &ModState, a: f32, amount: f32) {
        for k in 0..MAX_K {
            if self.fresh[m] {
                self.ksm[m][k] = ms.k[k];
            } else {
                self.ksm[m][k] += a * (ms.k[k] - self.ksm[m][k]);
            }
        }
        if self.fresh[m] {
            self.fresh[m] = false;
            self.act[m] = if ms.on { 1.0 } else { 0.0 };
            self.mixs[m] = (ms.mix * amount).clamp(0.0, 1.0);
        }
    }

    /// Runs everything on plain slices.
    pub fn process(&mut self, l: &mut [f32], r: &mut [f32], st: &State, tr: Transport) {
        let sr = self.sr;
        let bpm = tr.bpm.unwrap_or(120.0).clamp(20.0, 400.0) as f32;
        self.shared.bpm.store(bpm.to_bits(), Ordering::Relaxed);
        if tr.playing {
            if let Some(b) = tr.beat {
                self.beat = b;
            }
        }
        let spb = sr * 60.0 / bpm;
        let (want, n_want) = st.order();
        if self.n == 0 {
            // first block: start with the chain as it is, no fade
            self.chain = want;
            self.n_chain = n_want;
        }
        let changed = n_want != self.n_chain || want[..n_want] != self.chain[..self.n_chain];
        let fade_c = 1.0 / (0.003 * sr);
        let a_step = tc(0.03, sr / STEP as f32);
        let a_mix = tc(0.02, sr);
        let act_c = 1.0 / (0.02 * sr);
        let amount = st.amount;
        let x = st.x;
        let out_t = db(st.out_db);
        let os_lat = self.os_lat;
        let chain_now = self.chain;
        let n_now = self.n_chain;
        let n_os = chain_now[..n_now].iter().filter(|&&m| m == M_HEAT || m == M_COMP).count();
        let pad = os_lat * 2 - os_lat * n_os.min(2);
        let rack_lat = self.rack_lat(&chain_now[..n_now]);
        self.lat = self.lat_for(&chain_now[..n_now]);
        let total = self.lat;
        let in_t = db(st.in_db);
        let a_env = tc(0.3, sr);
        let a_mg = tc(0.15, sr);
        // echo strip
        let echo_t = st.echo_on();
        let em = &st.m[M_ECHO];
        self.ex = EchoX { ping: x.e_ping, scrub: x.e_scrub, smooth: x.e_smooth, pitch: x.e_pitch, lo: em.lo, hi: em.hi };
        let thru = em.k[5];
        let mut sends = [0.0f32; N_MODS];
        for &m in &chain_now[..n_now] {
            let s = st.m[m].esend.max(thru);
            sends[m] = if st.m[m].on { s } else { 0.0 };
            if sends[m] > 0.0 && !self.tw_on[m] {
                self.twin.reset(m);
            }
            self.tw_on[m] = sends[m] > 0.0;
        }
        let a_duck = (tc(0.005, sr), tc(0.25, sr));
        // global stages
        self.gsplit.set(x.g_lo, x.g_hi);
        let g_focus = !x.g_cut && self.gsplit.active();
        if x.g_cut {
            self.cutf.set(x.g_lo, x.g_hi, x.g_shape, sr);
        }
        self.tone.set(x.tone_mode, x.tone, sr);
        let tone_on = self.tone.active();
        let mc_t = (x.comp * 4.0).min(1.0);
        self.mcomp.control(&master::master_comp_k(x.comp_model, x.comp), x.comp_model);
        let len = l.len();
        let mut i0 = 0;
        while i0 < len {
            let i1 = (i0 + STEP).min(len);
            let c0 = Ctx { sr, spb, beat: self.beat, n: self.n };
            for ci in 0..self.n_chain {
                let m = self.chain[ci];
                let ms = st.m[m];
                self.smooth_knobs(m, &ms, a_step, amount);
                let kk = self.knobs(m, &ms, &x);
                self.main.control(m, &kk, &ms, &c0, x.cod_flut);
                if sends[m] > 0.0 {
                    self.twin.control(m, &kk, &ms, &c0, x.cod_flut);
                }
            }
            if echo_t || self.e_act > 0.0 {
                self.smooth_knobs(M_ECHO, em, a_step, amount);
                let kk = self.knobs(M_ECHO, em, &x);
                self.echo.control(&kk, em.var, &c0, &self.ex);
            }
            for i in i0..i1 {
                let c = Ctx { sr, spb, beat: self.beat, n: self.n };
                let (il, ir) = (l[i], r[i]);
                self.in_sm += a_mix * (in_t - self.in_sm);
                let (xi0, xi1) = (il * self.in_sm, ir * self.in_sm);
                // global FOCUS: only the range goes into the rack
                let ((mut x0, o0), (mut x1, o1)) = if g_focus { (self.gsplit.tick(0, xi0), self.gsplit.tick(1, xi1)) } else { ((xi0, 0.0), (xi1, 0.0)) };
                for ci in 0..self.n_chain {
                    let m = self.chain[ci];
                    let ms = &st.m[m];
                    let lm = self.lat_of(m);
                    let tgt = if ms.on { 1.0 } else { 0.0 };
                    if self.act[m] != tgt {
                        self.act[m] = if tgt > self.act[m] { (self.act[m] + act_c).min(1.0) } else { (self.act[m] - act_c).max(0.0) };
                    }
                    if self.act[m] <= 0.0 {
                        if lm > 0 {
                            x0 = self.mdd[m][0].tick(x0, lm);
                            x1 = self.mdd[m][1].tick(x1, lm);
                        }
                        continue;
                    }
                    self.mixs[m] += a_mix * ((ms.mix * amount).clamp(0.0, 1.0) - self.mixs[m]);
                    let g = self.mixs[m] * self.act[m];
                    let split = m != M_NOISE && m != M_TAME && self.main.split[m].active();
                    let ((mut d0, mut c0), (mut d1, mut c1)) = if split { (self.main.split[m].tick(0, x0), self.main.split[m].tick(1, x1)) } else { ((x0, 0.0), (x1, 0.0)) };
                    let (w0, w1) = self.main.tick(m, d0, d1, &c);
                    if lm > 0 {
                        d0 = self.mdd[m][0].tick(d0, lm);
                        d1 = self.mdd[m][1].tick(d1, lm);
                        if split {
                            c0 = self.mcd[m][0].tick(c0, lm);
                            c1 = self.mcd[m][1].tick(c1, lm);
                        }
                    }
                    let (dg, wg) = match MODS[m].law {
                        Law::Blend => (1.0 - g, g),
                        Law::Send => ((2.0 * (1.0 - g)).min(1.0), (2.0 * g).min(1.0)),
                    };
                    // CUT: what's outside the range goes away as the module comes in
                    let keep = if ms.cut { 1.0 - g } else { 1.0 };
                    x0 = d0 * dg + w0 * wg + c0 * keep;
                    x1 = d1 * dg + w1 * wg + c1 * keep;
                }
                if pad > 0 {
                    x0 = self.end_dd[0].tick(x0, pad);
                    x1 = self.end_dd[1].tick(x1, pad);
                }
                // chain edits: dip for 3 ms, swap, come back
                if changed || self.fade < 1.0 {
                    if changed {
                        self.fade = (self.fade - fade_c).max(0.0);
                    } else {
                        self.fade = (self.fade + fade_c).min(1.0);
                    }
                    x0 *= self.fade;
                    x1 *= self.fade;
                }
                // ---- ECHO strip
                if echo_t || self.e_act > 0.0 {
                    let tgt = if echo_t { 1.0 } else { 0.0 };
                    if self.e_act != tgt {
                        self.e_act = if tgt > self.e_act { (self.e_act + act_c).min(1.0) } else { (self.e_act - act_c).max(0.0) };
                    }
                    self.e_mix += a_mix * ((em.mix * amount).clamp(0.0, 1.0) - self.e_mix);
                    let g = self.e_mix * self.e_act;
                    let y = self.echo.read();
                    // the repeats go round through every module they're sent to
                    let mut f = y;
                    for ci in 0..self.n_chain {
                        let m = self.chain[ci];
                        let s = sends[m];
                        if s <= 0.0 {
                            continue;
                        }
                        let ms = &st.m[m];
                        // MAGNITUDE 0 = the module only plays on the repeats, at its usual amount
                        let gm = if ms.mix > 0.01 { self.mixs[m] } else { MODS[m].mix };
                        let tw = &mut self.twin;
                        let split = m != M_NOISE && m != M_TAME && tw.split[m].active();
                        let ((d0, cc0), (d1, cc1)) = if split { (tw.split[m].tick(0, f[0]), tw.split[m].tick(1, f[1])) } else { ((f[0], 0.0), (f[1], 0.0)) };
                        let (w0, w1) = tw.tick(m, d0, d1, &c);
                        let (dg, wg) = match MODS[m].law {
                            Law::Blend => (1.0 - gm, gm),
                            Law::Send => ((2.0 * (1.0 - gm)).min(1.0), (2.0 * gm).min(1.0)),
                        };
                        let keep = if ms.cut { 1.0 - gm } else { 1.0 };
                        let t0 = d0 * dg + w0 * wg + cc0 * keep;
                        let t1 = d1 * dg + w1 * wg + cc1 * keep;
                        f = [f[0] + (t0 - f[0]) * s, f[1] + (t1 - f[1]) * s];
                    }
                    let f = [if f[0].is_finite() { f[0] } else { 0.0 }, if f[1].is_finite() { f[1] } else { 0.0 }];
                    self.echo.write(x0 * self.e_act, x1 * self.e_act, f);
                    let (yl, yr) = self.echo.spread(y);
                    // DUCK: the repeats get out of the way while the dry is playing
                    let e = il.abs().max(ir.abs());
                    self.duck_env += if e > self.duck_env { a_duck.0 } else { a_duck.1 } * (e - self.duck_env);
                    let gd = if x.e_duck > 0.001 {
                        let nrm = ((20.0 * self.duck_env.max(1e-6).log10() + 50.0) / 40.0).clamp(0.0, 1.0);
                        db(-24.0 * x.e_duck * nrm)
                    } else {
                        1.0
                    };
                    let (dg, wg) = ((2.0 * (1.0 - g)).min(1.0), (2.0 * g).min(1.0));
                    let (el, er) = (yl * wg * gd, yr * wg * gd);
                    x0 = x0 * dg + el;
                    x1 = x1 * dg + er;
                    let pk = el.abs().max(er.abs());
                    self.e_vu = if pk > self.e_vu { pk } else { self.e_vu * 0.99985 };
                }
                // global FOCUS: the rest goes back in, lined up with the rack
                let b0 = self.g_dd[0].tick(o0, rack_lat);
                let b1 = self.g_dd[1].tick(o1, rack_lat);
                x0 += b0;
                x1 += b1;
                // global CUT
                if x.g_cut {
                    x0 = self.cutf.tick(0, x0);
                    x1 = self.cutf.tick(1, x1);
                }
                // TONE
                if tone_on {
                    x0 = self.tone.tick(0, x0);
                    x1 = self.tone.tick(1, x1);
                }
                // COMPRESS (master), level-matched blend so 0 is truly off
                let (p0, p1) = (self.mc_dd[0].tick(x0, os_lat), self.mc_dd[1].tick(x1, os_lat));
                self.mc_mix += a_mix * (mc_t - self.mc_mix);
                if mc_t > 0.0 || self.mc_mix > 1e-4 {
                    let (q0, q1) = self.mcomp.tick(x0, x1);
                    x0 = p0 + (q0 - p0) * self.mc_mix;
                    x1 = p1 + (q1 - p1) * self.mc_mix;
                } else {
                    x0 = p0;
                    x1 = p1;
                }
                let dl = self.dry_dd[0].tick(il, total);
                let dr = self.dry_dd[1].tick(ir, total);
                // LOUDNESS MATCH: rack out follows the (aligned) input's loudness
                let pin = 0.5 * (dl * dl + dr * dr);
                let pout = 0.5 * (x0 * x0 + x1 * x1);
                self.env_in += a_env * (pin - self.env_in);
                self.env_out += a_env * (pout - self.env_out);
                let mg_t = if st.matching && self.env_in > 1e-9 && self.env_out > 1e-9 { (self.env_in / self.env_out).sqrt().clamp(0.25, 4.0) } else if st.matching { self.match_g } else { 1.0 };
                self.match_g += a_mg * (mg_t - self.match_g);
                x0 *= self.match_g;
                x1 *= self.match_g;
                self.out_sm += a_mix * (out_t - self.out_sm);
                let (ol, or) = (dl + (x0 * self.out_sm - dl) * st.mix, dr + (x1 * self.out_sm - dr) * st.mix);
                let wet = self.byp.next(st.bypass);
                let (ol, or) = (dl + (ol - dl) * wet, dr + (or - dr) * wet);
                let (ol, or) = self.ceil.tick(ol, or, st.safe);
                let (ol, or) = if ol.is_finite() && or.is_finite() { (ol, or) } else { (0.0, 0.0) };
                self.meter.feed(dl, dr, ol, or);
                self.shared.spec_in.push(0.5 * (il + ir));
                self.shared.spec_out.push(0.5 * (ol + or));
                l[i] = ol;
                r[i] = or;
                self.n += 1;
                self.beat += 1.0 / spb as f64;
            }
            // swap the chain once faded out
            if changed && self.fade <= 0.0 {
                let old = self.chain;
                let n_old = self.n_chain;
                self.chain = want;
                self.n_chain = n_want;
                for &m in &want[..n_want] {
                    if !old[..n_old].contains(&m) {
                        self.reset_mod(m);
                    }
                }
                // the rest of this block runs with the new chain
                let rest = i1;
                if rest < len {
                    let (_, lb) = l.split_at_mut(rest);
                    let (_, rb) = r.split_at_mut(rest);
                    self.process(lb, rb, st, Transport { bpm: tr.bpm, beat: None, playing: false });
                }
                break;
            }
            i0 = i1;
        }
        if self.chain[..self.n_chain].contains(&M_TAME) {
            for k in 0..tame::BANDS {
                self.shared.cut[k].store(self.main.tame.cut[k].to_bits(), Ordering::Relaxed);
            }
        }
        for m in 0..N_MODS {
            put(&self.shared.flux[m], self.flux_v[m]);
        }
        put(&self.shared.e_vu, self.e_vu);
        put(&self.shared.comp_gr, self.main.comp.gr_db);
        put(&self.shared.mcomp_gr, if self.mc_mix > 1e-3 { self.mcomp.gr_db * self.mc_mix } else { 0.0 });
        self.shared.lat.store(self.lat as u32, Ordering::Relaxed);
        self.meter.store(&self.shared.meters);
        self.ceil.store(&self.shared.meters);
    }
}

pub struct Patina {
    params: Arc<PatinaParams>,
    shared: Arc<Shared>,
    eng: Engine,
    reported: u32,
}

impl Default for Patina {
    fn default() -> Self {
        let shared = Arc::new(Shared::default());
        Self { params: Arc::new(PatinaParams::default()), shared: shared.clone(), eng: Engine::new(48000.0, 1).with_shared(shared), reported: 0 }
    }
}

impl Plugin for Patina {
    const NAME: &'static str = "Patina";
    const VENDOR: &'static str = "awk studios";
    const URL: &'static str = "";
    const EMAIL: &'static str = "";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout { main_input_channels: NonZeroU32::new(2), main_output_channels: NonZeroU32::new(2), ..AudioIOLayout::const_default() }];
    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = false;
    type Editor = crate::nice_app::AwkEditor;
    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Self::Editor> {
        editor::create(self.params.clone(), self.shared.clone())
    }

    fn activate(&mut self, _layout: &AudioIOLayout, cfg: &BufferConfig, ctx: &mut impl ActivateContext<Self>) -> bool {
        self.eng = Engine::new(cfg.sample_rate, self.params.os.value()).with_shared(self.shared.clone());
        self.reported = self.eng.lat_for_state(&State::from_params(&self.params)) as u32;
        ctx.set_latency_samples(self.reported);
        true
    }

    fn reset(&mut self) {
        self.eng.reset();
    }

    fn process(&mut self, buffer: &mut Buffer, _aux: &mut AuxiliaryBuffers, context: &mut impl ProcessContext<Self>) -> ProcessStatus {
        let st = State::from_params(&self.params);
        if oversample::factor_for(st.os) != self.eng.os_factor() {
            // rebuild for the new oversampling (allocates; only when it changes)
            let sr = self.eng.sr;
            self.eng = Engine::new(sr, st.os).with_shared(self.shared.clone());
        }
        // LOSS in / out of the rack (or new oversampling) changes the latency
        let want = self.eng.lat_for_state(&st) as u32;
        if want != self.reported {
            self.reported = want;
            context.set_latency_samples(want);
        }
        let t = context.transport();
        let tr = Transport { bpm: t.tempo, beat: t.pos_beats(), playing: t.playing };
        let sl = buffer.as_slice();
        if sl.len() < 2 {
            return ProcessStatus::Normal;
        }
        let (a, b) = sl.split_at_mut(1);
        self.eng.process(&mut *a[0], &mut *b[0], &st, tr);
        ProcessStatus::Normal
    }
}

impl ClapPlugin for Patina {
    const CLAP_ID: &'static str = "com.awknode.patina";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("Colour rack: lo-fi, motion, space and tone modules in any order");
    const CLAP_MANUAL_URL: Option<&'static str> = None;
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::AudioEffect, ClapFeature::Distortion, ClapFeature::Reverb, ClapFeature::Stereo];
}

impl Vst3Plugin for Patina {
    const VST3_CLASS_ID: [u8; 16] = *b"AwknodePatinaFx1";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[Vst3SubCategory::Fx, Vst3SubCategory::Distortion, Vst3SubCategory::Reverb];
}

nice_export_clap!(Patina);
nice_export_vst3!(Patina);
