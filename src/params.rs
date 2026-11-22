//! Patina parameters. Ids are short and fixed: presets and sessions depend on them.
//!
//! The rack is 8 slots (`c1`..`c8`, which module sits there, 0 = empty) plus one
//! parameter set per module (prefix = module id, e.g. `echo_k0`). Each module can be
//! in the chain once, so its sound (reverb tails, echo buffer) survives reordering.

use crate::awk_ui;
use nice_plug::prelude::*;
use nice_plug_egui::EguiEditorState;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

pub const N_MODS: usize = 16;
pub const SLOTS: usize = 8;
pub const MAX_K: usize = 7;

pub const M_NOISE: usize = 0;
pub const M_CRUSH: usize = 1;
pub const M_MAGNETIC: usize = 2;
pub const M_CHOP: usize = 3;
pub const M_WOBBLE: usize = 4;
pub const M_ECHO: usize = 5;
pub const M_REVERB: usize = 6;
pub const M_CHORUS: usize = 7;
pub const M_TIME: usize = 8;
pub const M_REVERSE: usize = 9;
pub const M_HEAT: usize = 10;
pub const M_TAME: usize = 11;
pub const M_FILTER: usize = 12;
pub const M_WIDTH: usize = 13;
pub const M_LOSS: usize = 14;
pub const M_COMP: usize = 15;

/// REVERB flavors
pub const V_PLATE: usize = 0;
pub const V_GOLD: usize = 1;
pub const V_ROOM: usize = 2;
pub const V_CHAMBER: usize = 3;
pub const V_HALL: usize = 4;
pub const V_CATHEDRAL: usize = 5;
pub const V_SPRING: usize = 6;
pub const V_AMBIENCE: usize = 7;
pub const V_VAULT: usize = 8;
pub const V_LOFI: usize = 9;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cat {
    Texture,
    Motion,
    Space,
    Tone,
}

/// How the module's MIX blends: `Blend` crossfades, `Send` keeps the dry at full
/// up to 50 % (reverbs, echo) so a little MIX adds space without thinning the source.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Law {
    Blend,
    Send,
}

pub struct ModDef {
    pub id: &'static str,
    pub name: &'static str,
    pub cat: Cat,
    pub law: Law,
    pub vars: &'static [&'static str],
    /// (knob name, default 0..1)
    pub knobs: &'static [(&'static str, f32)],
    pub mix: f32,
    /// one line for the add menu / tooltip
    pub blurb: &'static str,
    /// FLUX: which knob the random movement pushes, and how far (0..1 of the knob)
    pub flux: (usize, f32),
    /// on when first added / in INIT
    pub on: bool,
}

pub const MODS: [ModDef; N_MODS] = [
    ModDef { id: "noise", name: "NOISE", cat: Cat::Texture, law: Law::Blend, vars: &["VINYL A", "TAPE", "HUM", "ROOM", "VINYL B", "SHELLAC", "DUST"], knobs: &[("LEVEL", 0.5), ("DUST", 0.3), ("TONE", 0.6), ("FOLLOW", 1.0), ("WIDTH", 0.5), ("SNAP", 0.35), ("POPS", 0.25)], mix: 1.0, blurb: "records (snap, crackle, pop), tape hiss, hum or room tone", flux: (0, 0.3), on: true },
    ModDef { id: "crush", name: "DIGITAL", cat: Cat::Texture, law: Law::Blend, vars: &["CLEAN", "DITHER"], knobs: &[("BITS", 0.5), ("RATE", 0.3), ("JITTER", 0.0), ("SMOOTH", 0.3)], mix: 0.7, blurb: "bit depth and sample rate, with jitter", flux: (1, 0.35), on: true },
    ModDef { id: "mag", name: "MAGNETIC", cat: Cat::Texture, law: Law::Blend, vars: &["CASSETTE", "REEL", "VHS"], knobs: &[("WEAR", 0.4), ("DROPOUTS", 0.2), ("SATURATE", 0.4), ("FLUTTER", 0.35), ("WEAR<>FLUT", 0.5), ("VOL<>FILT", 0.0), ("SHAPE", 0.3)], mix: 1.0, blurb: "worn tape: dull highs, dropouts, squash, flutter", flux: (1, 0.5), on: true },
    ModDef { id: "chop", name: "CHOP", cat: Cat::Texture, law: Law::Blend, vars: &["GATE", "STUTTER"], knobs: &[("RATE", 0.2), ("DEPTH", 0.7), ("SMOOTH", 0.3), ("SWING", 0.0), ("CHANCE", 1.0)], mix: 1.0, blurb: "tempo gate or stutter", flux: (1, 0.3), on: true },
    ModDef { id: "wob", name: "WOBBLE", cat: Cat::Motion, law: Law::Blend, vars: &["FREE", "SYNC"], knobs: &[("WOW", 0.4), ("RATE", 0.4), ("FLUTTER", 0.2), ("DRIFT", 0.2), ("STEREO", 0.3)], mix: 1.0, blurb: "wow, flutter and drift, free or on the beat", flux: (0, 0.4), on: true },
    ModDef { id: "echo", name: "ECHO", cat: Cat::Motion, law: Law::Send, vars: &["TAPE", "DIGITAL", "PING-PONG"], knobs: &[("TIME", 0.625), ("FEEDBACK", 0.35), ("TONE", 0.5), ("WOBBLE", 0.2), ("WIDTH", 0.5), ("THRU RACK", 0.0), ("GRAINS", 0.0)], mix: 0.3, blurb: "synced delay; THRU RACK sends the repeats back through every module, GRAINS scatters them", flux: (3, 0.5), on: true },
    ModDef { id: "verb", name: "SPACE", cat: Cat::Space, law: Law::Send, vars: &["PLATE", "GOLD PLATE", "ROOM", "CHAMBER", "HALL", "CATHEDRAL", "SPRING", "AMBIENCE", "VAULT", "LO-FI"], knobs: &[("DECAY", 0.4), ("SIZE", 0.5), ("PRE-DELAY", 0.1), ("DAMP", 0.4), ("MOD", 0.3), ("DUCK", 0.0)], mix: 0.3, blurb: "plates, rooms, halls, spring and more", flux: (2, 0.35), on: true },
    ModDef { id: "chorus", name: "CHORUS", cat: Cat::Motion, law: Law::Blend, vars: &["CHORUS", "ENSEMBLE", "FLANGER", "VIBRATO"], knobs: &[("RATE", 0.3), ("DEPTH", 0.5), ("FEEDBACK", 0.0), ("TONE", 0.6), ("WIDTH", 0.7)], mix: 0.5, blurb: "chorus, 3-voice ensemble, flanger or vibrato", flux: (1, 0.4), on: true },
    ModDef { id: "time", name: "VARISPEED", cat: Cat::Motion, law: Law::Blend, vars: &["1/2 SPEED", "2x SPEED", "TAPE STOP", "1/4 SPEED", "1/8 SPEED"], knobs: &[("CYCLE", 0.25), ("SMOOTH", 0.3), ("LENGTH", 0.5), ("CHANCE", 1.0)], mix: 1.0, blurb: "1/2, 1/4, 1/8 or 2x speed and tape stop, locked to the beat", flux: (3, 0.4), on: true },
    ModDef { id: "rev", name: "REVERSE", cat: Cat::Motion, law: Law::Blend, vars: &["SLICE", "SWELL"], knobs: &[("SIZE", 0.33), ("SMOOTH", 0.3), ("CHANCE", 1.0)], mix: 1.0, blurb: "plays each beat slice backwards", flux: (2, 0.4), on: true },
    ModDef { id: "heat", name: "DISTORT", cat: Cat::Tone, law: Law::Blend, vars: &["12AX7", "EL34", "TAPE", "IRON", "FET", "GERM.", "CLIP", "CLASSIC PRE", "PORTA"], knobs: &[("DRIVE", 0.375), ("COLOR", 0.35), ("TONE", 0.5), ("OVERLOAD", 0.0)], mix: 1.0, blurb: "KILN's saturation, level-matched; OVERLOAD pushes it past the edge", flux: (0, 0.3), on: true },
    ModDef { id: "tame", name: "TAME", cat: Cat::Tone, law: Law::Blend, vars: &[], knobs: &[("AMOUNT", 0.5), ("SHARP", 0.6), ("SPEED", 0.5)], mix: 1.0, blurb: "KILN's de-resonator", flux: (0, 0.3), on: true },
    ModDef { id: "filt", name: "FILTER", cat: Cat::Tone, law: Law::Blend, vars: &["LP", "HP", "BP", "PHONE"], knobs: &[("CUTOFF", 0.6), ("RES", 0.3), ("MOVE", 0.0), ("RATE", 0.5), ("ENV", 0.0), ("DRIVE", 0.0)], mix: 1.0, blurb: "filter with synced movement and envelope", flux: (0, 0.25), on: true },
    ModDef { id: "width", name: "WIDTH", cat: Cat::Tone, law: Law::Blend, vars: &[], knobs: &[("WIDTH", 0.5), ("HAAS", 0.0), ("MONO BASS", 0.0), ("PAN", 0.5)], mix: 1.0, blurb: "wider, Haas, mono bass, pan", flux: (0, 0.3), on: true },
    ModDef { id: "loss", name: "CODEC", cat: Cat::Texture, law: Law::Blend, vars: &["MP3", "STREAM", "VOIP"], knobs: &[("LOSSY", 0.5), ("DROPOUTS", 0.15), ("JOINT", 0.4), ("STRETCH", 0.0), ("BLUR", 0.0), ("FORMANT", 0.0)], mix: 1.0, blurb: "codec damage: low bitrate swirl, dropouts, joint stereo", flux: (1, 0.5), on: true },
    ModDef { id: "comp", name: "COMPRESS", cat: Cat::Tone, law: Law::Blend, vars: &crate::comp::MODELS, knobs: &[("INPUT", 0.4), ("RATIO", 0.5), ("ATTACK", 0.3), ("RELEASE", 0.4), ("COLOR", 0.3), ("AUTO GAIN", 1.0), ("SC HPF", 0.2)], mix: 1.0, blurb: "six compressors: 670 vari-mu, DS, FAT, ALPHA, SH-A, SH-B", flux: (0, 0.2), on: true },
];

/// Knob names that change with the flavor (REVERB's SPRING).
pub fn knob_name(m: usize, k: usize, var: usize) -> &'static str {
    match (m, var, k) {
        (M_COMP, v, k) => crate::comp::knob_name(v, k),
        (M_REVERB, V_SPRING, 1) => "TENSION",
        (M_REVERB, V_SPRING, 4) => "DRIP",
        (M_CHOP, 1, 1) => "REPEATS",
        _ => MODS[m].knobs.get(k).map(|x| x.0).unwrap_or(""),
    }
}

/// What the module is called on the faceplate: MAGNETIC and CODEC are the two
/// modes of LOSS.
pub fn ui_name(m: usize) -> &'static str {
    match m {
        M_MAGNETIC | M_LOSS => "LOSS",
        _ => MODS[m].name,
    }
}
pub const SHAPES: [&str; 5] = ["6 dB", "12 dB", "24 dB", "48 dB", "RESO"];
pub const TONE_MODES: [&str; 4] = ["TILT", "MID", "SPEAKER", "MIC"];

/// DISTORT variants -> KILN (type, variant). CLASSIC PRE = class-A preamp with
/// transformer; PORTA = worn cassette portastudio.
pub const HEAT_MAP: [(i32, i32); 9] = [(0, 0), (1, 0), (2, 0), (3, 2), (4, 0), (4, 1), (5, 0), (0, 2), (2, 2)];
// KILN type ids (used by sat.rs)
pub const T_PRE: i32 = 0;
pub const T_POWER: i32 = 1;
pub const T_TAPE: i32 = 2;
pub const T_IRON: i32 = 3;
pub const T_SOLID: i32 = 4;
pub const T_DIGITAL: i32 = 5;

/// Tempo divisions (name, length in quarter notes).
pub const CHOP_DIVS: [(&str, f32); 6] = [("1/4", 1.0), ("1/8", 0.5), ("1/8T", 1.0 / 3.0), ("1/16", 0.25), ("1/16T", 1.0 / 6.0), ("1/32", 0.125)];
pub const ECHO_DIVS: [(&str, f32); 9] = [("1/16", 0.25), ("1/8T", 1.0 / 3.0), ("1/16.", 0.375), ("1/8", 0.5), ("1/4T", 2.0 / 3.0), ("1/8.", 0.75), ("1/4", 1.0), ("1/4.", 1.5), ("1/2", 2.0)];
pub const TIME_DIVS: [(&str, f32); 5] = [("1 BAR", 4.0), ("1/2", 2.0), ("1/4", 1.0), ("1/8", 0.5), ("1/16", 0.25)];
pub const REV_DIVS: [(&str, f32); 4] = [("1/2", 2.0), ("1/4", 1.0), ("1/8", 0.5), ("1/16", 0.25)];
pub const LFO_DIVS: [(&str, f32); 7] = [("4 BARS", 16.0), ("2 BARS", 8.0), ("1 BAR", 4.0), ("1/2", 2.0), ("1/4", 1.0), ("1/8", 0.5), ("1/16", 0.25)];

pub fn pick<T: Copy>(list: &[T], v: f32) -> T {
    list[((v.clamp(0.0, 1.0) * (list.len() - 1) as f32).round() as usize).min(list.len() - 1)]
}

// ---- knob value mappings (shared by DSP and display) ----
pub fn noise_level_db(v: f32) -> f32 {
    -60.0 + 50.0 * v
}
pub fn crush_bits(v: f32) -> f32 {
    16.0 - 14.0 * v
}
pub fn crush_rate(v: f32) -> f32 {
    48000.0 * (1.0f32 / 48.0).powf(v)
}
pub fn wob_free_hz(v: f32) -> f32 {
    0.1 * 60f32.powf(v)
}
pub fn cutoff_hz(v: f32) -> f32 {
    20.0 * 1000f32.powf(v)
}
pub fn pre_ms(v: f32) -> f32 {
    250.0 * v * v
}
pub fn mono_bass_hz(v: f32) -> f32 {
    if v < 0.01 {
        0.0
    } else {
        40.0 * 7.5f32.powf(v)
    }
}
pub fn rt60(var: usize, v: f32) -> f32 {
    match var {
        V_PLATE => 0.3 * 30f32.powf(v),
        V_GOLD => 0.4 * 30f32.powf(v),
        V_ROOM => 0.15 * 20f32.powf(v),
        V_CHAMBER => 0.25 * 20f32.powf(v),
        V_HALL => 0.8 * 25f32.powf(v),
        V_CATHEDRAL => 1.2 * 25f32.powf(v),
        V_AMBIENCE => 0.1 * 12f32.powf(v),
        V_VAULT => 2.0 * 15f32.powf(v),
        V_LOFI => 0.4 * 20f32.powf(v),
        _ => 0.5 * 12f32.powf(v),
    }
}
pub fn loss_kbps(v: f32) -> f32 {
    320.0 * (8.0f32 / 320.0).powf(v)
}
/// side above this frequency is thrown away (joint stereo)
pub fn loss_joint_hz(v: f32) -> f32 {
    20000.0 * (0.02f32).powf(v)
}
pub fn chorus_hz(v: f32) -> f32 {
    0.05 * 100f32.powf(v)
}

fn hz(f: f32) -> String {
    if f >= 1000.0 {
        format!("{:.1}k", f / 1000.0)
    } else {
        format!("{:.0}", f)
    }
}
fn pc(v: f32) -> String {
    format!("{:.0}%", v * 100.0)
}

/// What a knob shows, in real units.
pub fn knob_text(m: usize, k: usize, v: f32, var: usize) -> String {
    match (m, k) {
        (M_COMP, k) => crate::comp::knob_text(var, k, v),
        (M_NOISE, 0) => format!("{:.0} dB", noise_level_db(v)),
        (M_NOISE, 2) => format!("{} Hz", hz(800.0 * 20f32.powf(v))),
        (M_CRUSH, 0) => format!("{:.1} bit", crush_bits(v)),
        (M_CRUSH, 1) => format!("{} Hz", hz(crush_rate(v))),
        (M_CHOP, 0) => pick(&CHOP_DIVS, v).0.to_string(),
        (M_CHOP, 1) if var == 1 => format!("x{}", pick(&[1, 2, 4, 8], v)),
        (M_WOBBLE, 1) if var == 1 => pick(&LFO_DIVS, v).0.to_string(),
        (M_WOBBLE, 1) => format!("{:.2} Hz", wob_free_hz(v)),
        (M_ECHO, 0) => pick(&ECHO_DIVS, v).0.to_string(),
        (M_ECHO, 1) => pc(v * 0.95),
        (M_ECHO, 2) | (M_HEAT, 2) => {
            let d = (v - 0.5) * 12.0;
            if d.abs() < 0.05 {
                "FLAT".into()
            } else if m == M_ECHO {
                if d < 0.0 { format!("DARK {:.0}", -d) } else { format!("BRIGHT {:.0}", d) }
            } else {
                format!("{:+.1} dB", d)
            }
        }
        (M_REVERB, 0) => format!("{:.1} s", rt60(var, v)),
        (M_REVERB, 2) => format!("{:.0} ms", pre_ms(v)),
        (M_REVERB, 3) => format!("{} Hz", hz(20000.0 * 0.05f32.powf(v))),
        (M_CHORUS, 0) => format!("{:.2} Hz", chorus_hz(v)),
        (M_CHORUS, 2) => pc(v * 0.9),
        (M_TIME, 0) => pick(&TIME_DIVS, v).0.to_string(),
        (M_TIME, 1) | (M_REVERSE, 1) => format!("{:.0} ms", 1.0 + 29.0 * v),
        (M_REVERSE, 0) => pick(&REV_DIVS, v).0.to_string(),
        (M_HEAT, 0) => format!("{:.1} dB", v * 24.0),
        (M_HEAT, 3) => if v < 0.005 { "OFF".into() } else { format!("+{:.0} dB", v * 18.0) },
        (M_ECHO, 5) => if v < 0.005 { "OFF".into() } else { pc(v) },
        (M_ECHO, 6) => if v < 0.005 { "OFF".into() } else { pc(v) },
        (M_LOSS, 0) => format!("{:.0} kbps", loss_kbps(v)),
        (M_LOSS, 1) => pc(v * 0.5),
        (M_MAGNETIC, 4) => {
            let b = (v - 0.5) * 200.0;
            if b.abs() < 2.0 { "BOTH".into() } else if b < 0.0 { format!("WEAR {:.0}", -b) } else { format!("FLUT {:.0}", b) }
        }
        (M_MAGNETIC, 5) => if v < 0.02 { "VOLUME".into() } else if v > 0.98 { "FILTER".into() } else { format!("{:.0}% FILT", v * 100.0) },
        (M_MAGNETIC, 6) => if v < 0.33 { "SOFT".into() } else if v < 0.67 { "MEDIUM".into() } else { "HARD".into() },
        (M_LOSS, 2) => format!("{} Hz", hz(loss_joint_hz(v))),
        (M_FILTER, 0) => format!("{} Hz", hz(cutoff_hz(v))),
        (M_FILTER, 2) => format!("{:.1} oct", v * 4.0),
        (M_FILTER, 3) => pick(&LFO_DIVS, v).0.to_string(),
        (M_FILTER, 5) => format!("{:.0} dB", v * 24.0),
        (M_WIDTH, 0) => pc(v * 2.0),
        (M_WIDTH, 1) => format!("{:.1} ms", v * 25.0),
        (M_WIDTH, 2) => {
            let f = mono_bass_hz(v);
            if f == 0.0 { "OFF".into() } else { format!("{} Hz", hz(f)) }
        }
        (M_WIDTH, 3) => {
            let p = (v - 0.5) * 200.0;
            if p.abs() < 1.0 { "C".into() } else if p < 0.0 { format!("L{:.0}", -p) } else { format!("R{:.0}", p) }
        }
        _ => pc(v),
    }
}

/// One module's parameters. Unused knob slots are hidden from the host.
#[derive(Params)]
pub struct ModParams {
    #[id = "on"]
    pub on: BoolParam,
    #[id = "mix"]
    pub mix: FloatParam,
    #[id = "var"]
    pub var: IntParam,
    #[id = "lo"]
    pub lo: FloatParam,
    #[id = "hi"]
    pub hi: FloatParam,
    #[id = "k0"]
    pub k0: FloatParam,
    #[id = "k1"]
    pub k1: FloatParam,
    #[id = "k2"]
    pub k2: FloatParam,
    #[id = "k3"]
    pub k3: FloatParam,
    #[id = "k4"]
    pub k4: FloatParam,
    #[id = "k5"]
    pub k5: FloatParam,
    #[id = "k6"]
    pub k6: FloatParam,
    /// FLUX: random movement over time (0 = still)
    #[id = "flux"]
    pub flux: FloatParam,
    /// ECHO send: how much of the echo's repeats go through this module
    #[id = "esend"]
    pub esend: FloatParam,
    /// RANGE mode: off = FOCUS (outside the range passes clean), on = CUT (outside is removed)
    #[id = "cut"]
    pub cut: BoolParam,
}

fn title(s: &str) -> String {
    let mut out = String::new();
    for (i, w) in s.split(' ').enumerate() {
        if i > 0 {
            out.push(' ');
        }
        let mut c = w.chars();
        if let Some(f) = c.next() {
            out.push(f);
            out.extend(c.flat_map(|x| x.to_lowercase()));
        }
    }
    out
}

impl ModParams {
    pub fn new(m: usize) -> Self {
        let d = &MODS[m];
        let nm = title(d.name);
        let knob = |k: usize| {
            let (name, def, used) = d.knobs.get(k).map(|&(n, v)| (format!("{} {}", nm, title(n)), v, true)).unwrap_or((format!("{} -{}", nm, k), 0.0, false));
            let p = FloatParam::new(name, def, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_value_to_string(Arc::new(move |v| knob_text(m, k, v, 0)))
                .with_string_to_value(formatters::s2v_f32_percentage());
            if used {
                p
            } else {
                p.hide()
            }
        };
        let nvar = d.vars.len().max(1) as i32;
        let vars = d.vars;
        let freq = |name: String, def: f32| {
            FloatParam::new(name, def, FloatRange::Skewed { min: 20.0, max: 20000.0, factor: FloatRange::skew_factor(-2.0) })
                .with_unit(" Hz")
                .with_value_to_string(formatters::v2s_f32_hz_then_khz(0))
                .with_string_to_value(formatters::s2v_f32_hz_then_khz())
        };
        Self {
            on: BoolParam::new(format!("{} On", nm), d.on),
            mix: FloatParam::new(format!("{} Mix", nm), d.mix, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_unit("%")
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            var: IntParam::new(format!("{} Flavor", nm), 0, IntRange::Linear { min: 0, max: nvar - 1 })
                .with_value_to_string(Arc::new(move |v| vars.get(v as usize).map(|s| s.to_string()).unwrap_or_default())),
            lo: freq(format!("{} Range Low", nm), 20.0),
            hi: freq(format!("{} Range High", nm), 20000.0),
            k0: knob(0),
            k1: knob(1),
            k2: knob(2),
            k3: knob(3),
            k4: knob(4),
            k5: knob(5),
            k6: knob(6),
            flux: FloatParam::new(format!("{} Flux", nm), 0.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_unit("%")
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            cut: BoolParam::new(format!("{} Cut", nm), false),
            esend: FloatParam::new(format!("{} Echo Send", nm), 0.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_unit("%")
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
        }
    }
    pub fn knobs(&self) -> [&FloatParam; MAX_K] {
        [&self.k0, &self.k1, &self.k2, &self.k3, &self.k4, &self.k5, &self.k6]
    }
}

fn pct(name: &str, def: f32) -> FloatParam {
    FloatParam::new(name, def, FloatRange::Linear { min: 0.0, max: 1.0 })
        .with_unit("%")
        .with_value_to_string(formatters::v2s_f32_percentage(0))
        .with_string_to_value(formatters::s2v_f32_percentage())
}
fn freq(name: &str, def: f32) -> FloatParam {
    FloatParam::new(name, def, FloatRange::Skewed { min: 20.0, max: 20000.0, factor: FloatRange::skew_factor(-2.0) })
        .with_unit(" Hz")
        .with_value_to_string(formatters::v2s_f32_hz_then_khz(0))
        .with_string_to_value(formatters::s2v_f32_hz_then_khz())
}
fn slot(i: usize, def: i32) -> IntParam {
    IntParam::new(format!("Slot {}", i), def, IntRange::Linear { min: 0, max: N_MODS as i32 })
        .with_value_to_string(Arc::new(|v| if v <= 0 { "EMPTY".into() } else { MODS[(v - 1) as usize].name.to_string() }))
        .non_automatable()
}

#[derive(Params)]
pub struct PatinaParams {
    /// window size (hosts keep track of it; not persisted)
    pub editor_state: Arc<EguiEditorState>,
    /// paper (light) look instead of the default dark one
    #[persist = "paper"]
    pub paper: Arc<AtomicBool>,

    #[id = "gmix"]
    pub mix: FloatParam,
    #[id = "bypass"]
    pub bypass: BoolParam,
    #[id = "safe"]
    pub safe: BoolParam,
    #[id = "os"]
    pub os: IntParam,
    /// MAGNITUDE: scales every module's MIX at once
    #[id = "amount"]
    pub amount: FloatParam,
    #[id = "out"]
    pub out: FloatParam,
    #[id = "in"]
    pub input: FloatParam,
    /// loudness match: the rack comes out as loud as it went in
    #[id = "match"]
    pub matching: BoolParam,

    #[id = "c1"]
    pub c1: IntParam,
    #[id = "c2"]
    pub c2: IntParam,
    #[id = "c3"]
    pub c3: IntParam,
    #[id = "c4"]
    pub c4: IntParam,
    #[id = "c5"]
    pub c5: IntParam,
    #[id = "c6"]
    pub c6: IntParam,
    #[id = "c7"]
    pub c7: IntParam,
    #[id = "c8"]
    pub c8: IntParam,

    #[nested(id_prefix = "noise", group = "Noise")]
    pub noise: ModParams,
    #[nested(id_prefix = "crush", group = "Crush")]
    pub crush: ModParams,
    #[nested(id_prefix = "mag", group = "Magnetic")]
    pub mag: ModParams,
    #[nested(id_prefix = "chop", group = "Chop")]
    pub chop: ModParams,
    #[nested(id_prefix = "wob", group = "Wobble")]
    pub wob: ModParams,
    #[nested(id_prefix = "echo", group = "Echo")]
    pub echo: ModParams,
    #[nested(id_prefix = "verb", group = "Reverb")]
    pub verb: ModParams,
    #[nested(id_prefix = "chorus", group = "Chorus")]
    pub chorus: ModParams,
    #[nested(id_prefix = "time", group = "Time")]
    pub time: ModParams,
    #[nested(id_prefix = "rev", group = "Reverse")]
    pub rev: ModParams,
    #[nested(id_prefix = "heat", group = "Heat")]
    pub heat: ModParams,
    #[nested(id_prefix = "tame", group = "Tame")]
    pub tame: ModParams,
    #[nested(id_prefix = "filt", group = "Filter")]
    pub filt: ModParams,
    #[nested(id_prefix = "width", group = "Width")]
    pub width: ModParams,
    #[nested(id_prefix = "loss", group = "Loss")]
    pub loss: ModParams,
    #[nested(id_prefix = "comp", group = "Compress")]
    pub comp: ModParams,

    // ---- ECHO strip (under the rack)
    #[id = "eon"]
    pub e_on: BoolParam,
    #[id = "eping"]
    pub e_ping: BoolParam,
    #[id = "escrub"]
    pub e_scrub: BoolParam,
    #[id = "educk"]
    pub e_duck: FloatParam,
    #[id = "esmooth"]
    pub e_smooth: FloatParam,
    #[id = "epitch"]
    pub e_pitch: FloatParam,

    // ---- LOSS toggles (per mode)
    #[id = "mwear"]
    pub mag_wear: BoolParam,
    #[id = "mflut"]
    pub mag_flut: BoolParam,
    #[id = "cwear"]
    pub cod_wear: BoolParam,
    #[id = "cflut"]
    pub cod_flut: BoolParam,

    // ---- global FOCUS / CUT, TONE, COMPRESS
    #[id = "glo"]
    pub g_lo: FloatParam,
    #[id = "ghi"]
    pub g_hi: FloatParam,
    #[id = "gcut"]
    pub g_cut: BoolParam,
    #[id = "gshape"]
    pub g_shape: IntParam,
    #[id = "tone"]
    pub tone: FloatParam,
    #[id = "tmode"]
    pub tone_mode: IntParam,
    #[id = "mcomp"]
    pub m_comp: FloatParam,
    #[id = "cmodel"]
    pub comp_model: IntParam,
}

impl Default for PatinaParams {
    fn default() -> Self {
        let os = IntParam::new("Oversampling", 1, IntRange::Linear { min: 0, max: 3 }).with_value_to_string(Arc::new(|v| awk_ui::OS_NAMES[v.clamp(0, 3) as usize].to_string()));
        Self {
            editor_state: EguiEditorState::from_size(nice_plug::editor::dpi::LogicalSize::new((crate::editor::WIDTH) as f64, (crate::editor::HEIGHT) as f64), 1.0),
            paper: Arc::new(AtomicBool::new(false)),
            mix: awk_ui::mix_param(),
            bypass: awk_ui::bypass_param(),
            safe: awk_ui::safe_param(),
            os,
            amount: FloatParam::new("Magnitude", 1.0, FloatRange::Linear { min: 0.0, max: 1.5 })
                .with_unit("%")
                .with_value_to_string(formatters::v2s_f32_percentage(0))
                .with_string_to_value(formatters::s2v_f32_percentage()),
            out: FloatParam::new("Output", 0.0, FloatRange::Linear { min: -12.0, max: 12.0 }).with_unit(" dB").with_value_to_string(formatters::v2s_f32_rounded(1)),
            input: FloatParam::new("Input", 0.0, FloatRange::Linear { min: -12.0, max: 12.0 }).with_unit(" dB").with_value_to_string(formatters::v2s_f32_rounded(1)),
            matching: BoolParam::new("Loudness Match", false),
            // default chain, the classic order: NOISE, WOBBLE, DISTORT, DIGITAL (off), SPACE, MAGNETIC
            c1: slot(1, (M_NOISE + 1) as i32),
            c2: slot(2, (M_WOBBLE + 1) as i32),
            c3: slot(3, (M_HEAT + 1) as i32),
            c4: slot(4, (M_REVERB + 1) as i32),
            c5: slot(5, (M_MAGNETIC + 1) as i32),
            c6: slot(6, 0),
            c7: slot(7, 0),
            c8: slot(8, 0),
            noise: ModParams::new(M_NOISE),
            crush: ModParams::new(M_CRUSH),
            mag: ModParams::new(M_MAGNETIC),
            chop: ModParams::new(M_CHOP),
            wob: ModParams::new(M_WOBBLE),
            echo: ModParams::new(M_ECHO),
            verb: ModParams::new(M_REVERB),
            chorus: ModParams::new(M_CHORUS),
            time: ModParams::new(M_TIME),
            rev: ModParams::new(M_REVERSE),
            heat: ModParams::new(M_HEAT),
            tame: ModParams::new(M_TAME),
            filt: ModParams::new(M_FILTER),
            width: ModParams::new(M_WIDTH),
            loss: ModParams::new(M_LOSS),
            comp: ModParams::new(M_COMP),
            e_on: BoolParam::new("Echo On", false),
            e_ping: BoolParam::new("Echo Ping-Pong", false),
            e_scrub: BoolParam::new("Echo Scrub", true),
            e_duck: pct("Echo Duck", 0.0),
            e_smooth: pct("Echo Smooth", 0.0),
            e_pitch: FloatParam::new("Echo Pitch", 0.0, FloatRange::Linear { min: -12.0, max: 12.0 }).with_step_size(1.0).with_unit(" st").with_value_to_string(formatters::v2s_f32_rounded(0)),
            mag_wear: BoolParam::new("Magnetic Wear", true),
            mag_flut: BoolParam::new("Magnetic Flutter", true),
            cod_wear: BoolParam::new("Codec Wear", true),
            cod_flut: BoolParam::new("Codec Flutter", false),
            g_lo: freq("Focus Low", 20.0),
            g_hi: freq("Focus High", 20000.0),
            g_cut: BoolParam::new("Focus Cut", false),
            g_shape: IntParam::new("Cut Shape", 2, IntRange::Linear { min: 0, max: 4 }).with_value_to_string(Arc::new(|v| SHAPES[v.clamp(0, 4) as usize].to_string())),
            tone: FloatParam::new("Tone", 0.0, FloatRange::Linear { min: -1.0, max: 1.0 }).with_value_to_string(formatters::v2s_f32_rounded(2)),
            tone_mode: IntParam::new("Tone Mode", 0, IntRange::Linear { min: 0, max: 3 }).with_value_to_string(Arc::new(|v| TONE_MODES[v.clamp(0, 3) as usize].to_string())),
            m_comp: pct("Compress", 0.0),
            comp_model: IntParam::new("Compress Model", 0, IntRange::Linear { min: 0, max: 5 }).with_value_to_string(Arc::new(|v| crate::comp::MODELS[v.clamp(0, 5) as usize].to_string())),
        }
    }
}

impl PatinaParams {
    pub fn slots(&self) -> [&IntParam; SLOTS] {
        [&self.c1, &self.c2, &self.c3, &self.c4, &self.c5, &self.c6, &self.c7, &self.c8]
    }
    pub fn m(&self, i: usize) -> &ModParams {
        match i {
            M_NOISE => &self.noise,
            M_CRUSH => &self.crush,
            M_MAGNETIC => &self.mag,
            M_CHOP => &self.chop,
            M_WOBBLE => &self.wob,
            M_ECHO => &self.echo,
            M_REVERB => &self.verb,
            M_CHORUS => &self.chorus,
            M_TIME => &self.time,
            M_REVERSE => &self.rev,
            M_HEAT => &self.heat,
            M_TAME => &self.tame,
            M_FILTER => &self.filt,
            M_LOSS => &self.loss,
            M_COMP => &self.comp,
            _ => &self.width,
        }
    }
}
