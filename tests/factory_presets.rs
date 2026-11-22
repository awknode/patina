//! Factory presets. `cargo test --test factory_presets` checks every preset names
//! real params with legal values and runs it (bounded, finite); with PRESET_OUT=dir
//! it also writes the .awkp files. Anything a preset doesn't mention loads at its default.
use nice_plug::prelude::*;
use patina::*;
use serde_json::{json, Map, Value};

/// (module, [(field, value)]) — field is on/mix/var/lo/hi/flux/cut/esend/k0..k6.
/// ECHO in the list = the ECHO strip on (it doesn't take a slot).
type Mod = (usize, &'static [(&'static str, f64)]);
/// (name, chain in order with its settings, global extras)
type Preset = (&'static str, &'static [Mod], &'static [(&'static str, f64)]);

const PRESETS: &[Preset] = &[
    // VOCAL
    ("VOCAL - Old Radio", &[(M_FILTER, &[("var", 3.0), ("k0", 0.62), ("k5", 0.3)]), (M_HEAT, &[("var", 5.0), ("k0", 0.45)]), (M_NOISE, &[("var", 2.0), ("k0", 0.35)]), (M_REVERB, &[("var", 2.0), ("mix", 0.18)])], &[]),
    ("VOCAL - Dreamy Plate", &[(M_TAME, &[("k0", 0.45)]), (M_CHORUS, &[("var", 0.0), ("mix", 0.3), ("k0", 0.25)]), (M_ECHO, &[("k0", 0.625), ("k1", 0.3), ("mix", 0.22)]), (M_REVERB, &[("var", 0.0), ("k0", 0.55), ("k2", 0.3), ("k5", 0.4), ("mix", 0.35)])], &[]),
    ("VOCAL - Tape Double", &[(M_MAGNETIC, &[("var", 1.0), ("k0", 0.2), ("k3", 0.25)]), (M_CHORUS, &[("var", 1.0), ("mix", 0.35), ("k1", 0.35)]), (M_WIDTH, &[("k0", 0.65), ("k2", 0.4)])], &[]),
    ("VOCAL - Telephone Hook", &[(M_FILTER, &[("var", 3.0), ("k0", 0.66), ("k5", 0.45)]), (M_CRUSH, &[("k0", 0.4), ("k1", 0.35), ("mix", 0.5)]), (M_ECHO, &[("var", 1.0), ("k0", 0.5), ("mix", 0.2)])], &[]),
    ("VOCAL - Reverse Swell Ad-libs", &[(M_REVERB, &[("var", 4.0), ("k0", 0.6), ("mix", 0.6)]), (M_REVERSE, &[("var", 1.0), ("k0", 0.33)])], &[]),
    // DRUMS
    ("DRUMS - Dusty Break", &[(M_HEAT, &[("var", 3.0), ("k0", 0.4)]), (M_CRUSH, &[("k0", 0.3), ("k1", 0.42), ("k3", 0.4), ("mix", 0.5)]), (M_NOISE, &[("var", 0.0), ("k0", 0.45), ("k1", 0.45)]), (M_FILTER, &[("k0", 0.82), ("k1", 0.1)])], &[]),
    ("DRUMS - Gated Room Smash", &[(M_REVERB, &[("var", 2.0), ("k0", 0.35), ("k5", 0.0), ("mix", 0.45)]), (M_CHOP, &[("k0", 0.2), ("k1", 0.85), ("k2", 0.15)]), (M_HEAT, &[("var", 7.0), ("k0", 0.4)])], &[]),
    ("DRUMS - Half-Time Flip", &[(M_TIME, &[("var", 0.0), ("k0", 0.25)]), (M_HEAT, &[("var", 4.0), ("k0", 0.3)])], &[]),
    ("DRUMS - Stutter Fills", &[(M_CHOP, &[("var", 1.0), ("k0", 0.6), ("k1", 0.67), ("k4", 0.35)]), (M_HEAT, &[("var", 5.0), ("k0", 0.35)])], &[]),
    ("DRUMS - Cassette Loop", &[(M_MAGNETIC, &[("var", 0.0), ("k0", 0.55), ("k1", 0.25), ("k2", 0.5)]), (M_WOBBLE, &[("k0", 0.25), ("k2", 0.3)]), (M_NOISE, &[("var", 1.0), ("k0", 0.4)])], &[]),
    ("DRUMS - Hats Only Crunch", &[(M_CRUSH, &[("k0", 0.55), ("k1", 0.5), ("lo", 4000.0), ("mix", 0.7)]), (M_WIDTH, &[("k0", 0.7), ("lo", 4000.0)])], &[]),
    // 808 / BASS
    ("808 - Grit, Sub Stays Clean", &[(M_HEAT, &[("var", 2.0), ("k0", 0.6), ("lo", 120.0)]), (M_WIDTH, &[("k2", 0.5)])], &[]),
    ("808 - Tape Stop Drops", &[(M_TIME, &[("var", 2.0), ("k0", 0.0), ("k2", 0.35)]), (M_HEAT, &[("var", 6.0), ("k0", 0.45), ("lo", 90.0)])], &[]),
    ("BASS - Warm Cassette", &[(M_MAGNETIC, &[("var", 0.0), ("k0", 0.3), ("k2", 0.55), ("k3", 0.15)]), (M_WIDTH, &[("k2", 0.55)])], &[]),
    ("BASS - Filter Wub (synced)", &[(M_FILTER, &[("var", 0.0), ("k0", 0.45), ("k1", 0.55), ("k2", 0.55), ("k3", 0.66), ("k5", 0.3)]), (M_HEAT, &[("var", 5.0), ("k0", 0.4)])], &[]),
    // MELODY / KEYS
    ("MELODY - Lo-Fi Keys", &[(M_WOBBLE, &[("k0", 0.45), ("k2", 0.25), ("k3", 0.3)]), (M_MAGNETIC, &[("var", 0.0), ("k0", 0.5), ("k1", 0.2)]), (M_NOISE, &[("var", 0.0), ("k0", 0.5), ("k1", 0.35)]), (M_REVERB, &[("var", 1.0), ("mix", 0.25)])], &[]),
    ("MELODY - Warped Vinyl", &[(M_WOBBLE, &[("k0", 0.7), ("k1", 0.3), ("k3", 0.5)]), (M_FILTER, &[("k0", 0.78)]), (M_NOISE, &[("var", 0.0), ("k0", 0.6), ("k1", 0.6)])], &[]),
    ("MELODY - VHS Memory", &[(M_MAGNETIC, &[("var", 2.0), ("k0", 0.6), ("k1", 0.35), ("k3", 0.5)]), (M_CHORUS, &[("var", 1.0), ("mix", 0.4)]), (M_NOISE, &[("var", 1.0), ("k0", 0.45)]), (M_REVERB, &[("var", 4.0), ("mix", 0.25)])], &[]),
    ("MELODY - Spring Guitar", &[(M_HEAT, &[("var", 2.0), ("k0", 0.35)]), (M_REVERB, &[("var", 6.0), ("k0", 0.45), ("k4", 0.6), ("mix", 0.35)])], &[]),
    ("MELODY - Pluck Ping-Pong", &[(M_ECHO, &[("var", 2.0), ("k0", 0.5), ("k1", 0.45), ("mix", 0.35)]), (M_REVERB, &[("var", 0.0), ("mix", 0.2)])], &[]),
    ("MELODY - Ensemble Strings", &[(M_CHORUS, &[("var", 1.0), ("mix", 0.6), ("k0", 0.35), ("k1", 0.6)]), (M_REVERB, &[("var", 4.0), ("k0", 0.55), ("mix", 0.3)])], &[]),
    ("MELODY - Bit Toy", &[(M_CRUSH, &[("k0", 0.75), ("k1", 0.55), ("k2", 0.3), ("mix", 0.8)]), (M_ECHO, &[("var", 1.0), ("k0", 0.25), ("mix", 0.2)])], &[]),
    ("MELODY - Reverse Pad", &[(M_REVERSE, &[("var", 0.0), ("k0", 0.0)]), (M_REVERB, &[("var", 5.0), ("k0", 0.55), ("mix", 0.4)])], &[]),
    ("MELODY - Flanged Jet", &[(M_CHORUS, &[("var", 2.0), ("k0", 0.15), ("k1", 0.8), ("k2", 0.6), ("mix", 0.5)])], &[]),
    // BUS / MASTER
    ("BUS - Glue + Polish", &[(M_TAME, &[("k0", 0.35)]), (M_HEAT, &[("var", 4.0), ("k0", 0.25)]), (M_WIDTH, &[("k0", 0.58), ("k2", 0.45)])], &[]),
    ("MASTER - Subtle Patina", &[(M_HEAT, &[("var", 3.0), ("k0", 0.2)]), (M_NOISE, &[("var", 1.0), ("k0", 0.3)]), (M_WOBBLE, &[("k0", 0.12), ("k2", 0.1), ("k3", 0.1)])], &[("amount", 0.8)]),
    ("MASTER - Whole Song Lo-Fi", &[(M_WOBBLE, &[("k0", 0.3)]), (M_MAGNETIC, &[("var", 0.0), ("k0", 0.45), ("k1", 0.1)]), (M_NOISE, &[("var", 0.0), ("k0", 0.45), ("k1", 0.3)]), (M_FILTER, &[("k0", 0.86), ("k1", 0.05)])], &[]),
    // FX
    ("FX - Underwater", &[(M_FILTER, &[("k0", 0.4), ("k1", 0.4)]), (M_CHORUS, &[("var", 3.0), ("k1", 0.7), ("mix", 1.0)]), (M_REVERB, &[("var", 4.0), ("mix", 0.4)])], &[]),
    ("FX - Cathedral Wash", &[(M_REVERB, &[("var", 5.0), ("k0", 0.8), ("k1", 0.9), ("k4", 0.6), ("mix", 0.6)])], &[]),
    ("FX - Double-Time Glitch", &[(M_TIME, &[("var", 1.0), ("k0", 0.5), ("k3", 0.5)]), (M_CHOP, &[("var", 1.0), ("k0", 0.8), ("k4", 0.3)]), (M_CRUSH, &[("k0", 0.5), ("mix", 0.4)])], &[]),
    // v2: LOSS, THRU RACK, GRAINS, 1/4 + 1/8 speed, FLUX, CLASSIC PRE / PORTA / OVERLOAD, new SPACEs, MATCH
    ("LOFI - Bad Stream Vocal", &[(M_LOSS, &[("var", 1.0), ("k0", 0.7), ("k1", 0.35), ("flux", 0.4)]), (M_REVERB, &[("var", 7.0), ("mix", 0.2)])], &[("match", 1.0)]),
    ("LOFI - 64k MP3 Beat", &[(M_LOSS, &[("var", 0.0), ("k0", 0.62), ("k1", 0.08), ("k2", 0.7)]), (M_NOISE, &[("var", 1.0), ("k0", 0.3)])], &[("match", 1.0)]),
    ("LOFI - Phone Call", &[(M_LOSS, &[("var", 2.0), ("k0", 0.45), ("k1", 0.2)]), (M_HEAT, &[("var", 8.0), ("k0", 0.3)])], &[]),
    ("LOFI - Portastudio Keys", &[(M_HEAT, &[("var", 8.0), ("k0", 0.35)]), (M_WOBBLE, &[("k0", 0.35), ("flux", 0.5)]), (M_MAGNETIC, &[("var", 0.0), ("k0", 0.45), ("flux", 0.4)]), (M_NOISE, &[("var", 1.0), ("k0", 0.35)]), (M_REVERB, &[("var", 9.0), ("mix", 0.25)])], &[("match", 1.0)]),
    ("VOCAL - Classic Pre Warmth", &[(M_HEAT, &[("var", 7.0), ("k0", 0.3), ("k1", 0.3)]), (M_TAME, &[("k0", 0.35)]), (M_REVERB, &[("var", 7.0), ("mix", 0.18)])], &[("match", 1.0)]),
    ("DRUMS - Overload Smash", &[(M_HEAT, &[("var", 6.0), ("k0", 0.5), ("k3", 0.6)]), (M_REVERB, &[("var", 2.0), ("k0", 0.25), ("mix", 0.25)])], &[("match", 1.0)]),
    ("DRUMS - Vault Room", &[(M_REVERB, &[("var", 8.0), ("k0", 0.45), ("mix", 0.4)]), (M_HEAT, &[("var", 3.0), ("k0", 0.3)])], &[]),
    ("FX - Echo Thru The Rack", &[(M_ECHO, &[("var", 0.0), ("k0", 0.5), ("k1", 0.55), ("k5", 0.7), ("mix", 0.4)]), (M_WOBBLE, &[("k0", 0.3)]), (M_HEAT, &[("var", 2.0), ("k0", 0.35)]), (M_FILTER, &[("k0", 0.75)])], &[]),
    ("FX - Grain Cloud", &[(M_ECHO, &[("var", 2.0), ("k0", 0.5), ("k1", 0.5), ("k6", 0.8), ("mix", 0.5)]), (M_REVERB, &[("var", 7.0), ("k0", 0.6), ("mix", 0.4)])], &[]),
    ("FX - Quarter Speed Drop", &[(M_TIME, &[("var", 3.0), ("k0", 0.5)]), (M_MAGNETIC, &[("var", 0.0), ("k0", 0.4)])], &[]),
    ("FX - Eighth Speed Abyss", &[(M_TIME, &[("var", 4.0), ("k0", 0.75)]), (M_REVERB, &[("var", 5.0), ("mix", 0.35)])], &[]),
    ("MELODY - Flux Everything", &[(M_NOISE, &[("flux", 0.6)]), (M_WOBBLE, &[("flux", 0.8)]), (M_HEAT, &[("var", 1.0), ("flux", 0.5)]), (M_CHORUS, &[("flux", 0.6), ("mix", 0.3)]), (M_REVERB, &[("var", 7.0), ("flux", 0.6), ("mix", 0.3)]), (M_MAGNETIC, &[("flux", 0.7)])], &[("match", 1.0)]),
    ("MASTER - Ambience Glue", &[(M_HEAT, &[("var", 7.0), ("k0", 0.15)]), (M_REVERB, &[("var", 7.0), ("k0", 0.3), ("mix", 0.12)]), (M_WIDTH, &[("k0", 0.56), ("k2", 0.4)])], &[("match", 1.0)]),
    // v2.1: COMPRESS (6 models), ECHO strip + sends, CODEC / MAGNETIC knobs, global FOCUS / CUT, TONE
    ("COMPRESS - 670 Vocal Glue", &[(M_COMP, &[("var", 0.0), ("k0", 0.45), ("k1", 0.55), ("k2", 0.2), ("k3", 0.0), ("k4", 0.35)]), (M_REVERB, &[("var", 0.0), ("mix", 0.15)])], &[]),
    ("COMPRESS - 670 Slow Bus (TC5)", &[(M_COMP, &[("var", 0.0), ("k0", 0.4), ("k2", 0.8), ("k3", 0.5), ("k4", 0.25)])], &[]),
    ("COMPRESS - DS Drum Smash (parallel)", &[(M_COMP, &[("var", 1.0), ("k0", 0.7), ("k1", 0.86), ("k2", 0.3), ("k3", 0.35), ("k4", 0.3), ("mix", 0.55)])], &[]),
    ("COMPRESS - DS Nuke + Dist 3", &[(M_COMP, &[("var", 1.0), ("k0", 0.6), ("k1", 1.0), ("k2", 0.2), ("k3", 0.3), ("k4", 0.8), ("mix", 0.5)])], &[]),
    ("COMPRESS - FAT Bus Warmth", &[(M_COMP, &[("var", 2.0), ("k0", 0.4), ("k1", 0.33), ("k2", 0.35), ("k3", 0.4), ("k4", 0.35)])], &[]),
    ("COMPRESS - FAT Spank Snare", &[(M_COMP, &[("var", 2.0), ("k0", 0.55), ("k1", 0.67), ("k2", 0.2), ("k3", 0.2), ("k4", 0.5), ("mix", 0.7)])], &[]),
    ("COMPRESS - ALPHA Negative Pump", &[(M_COMP, &[("var", 3.0), ("k0", 0.55), ("k1", 0.95), ("k2", 0.4), ("k3", 0.3), ("k4", 0.3)])], &[]),
    ("COMPRESS - ALPHA Clean Vocal", &[(M_COMP, &[("var", 3.0), ("k0", 0.45), ("k1", 0.45), ("k2", 0.45), ("k3", 0.45), ("k4", 0.15)]), (M_TAME, &[("k0", 0.3)])], &[]),
    ("MASTER - SH-A Polish", &[(M_TAME, &[("k0", 0.3)]), (M_WIDTH, &[("k0", 0.56), ("k2", 0.4)])], &[("mcomp", 0.45), ("cmodel", 4.0), ("tone", 0.12)]),
    ("MASTER - SH-B Class A Glow", &[(M_COMP, &[("var", 5.0), ("k0", 0.4), ("k1", 0.4), ("k2", 0.8), ("k3", 0.45), ("k4", 0.5)])], &[]),
    ("ECHO - Dub Throw", &[(M_HEAT, &[("var", 2.0), ("k0", 0.3), ("esend", 0.7)]), (M_FILTER, &[("k0", 0.55), ("k2", 0.4), ("mix", 0.0), ("esend", 0.8)]), (M_REVERB, &[("var", 6.0), ("mix", 0.2), ("esend", 0.4)]), (M_ECHO, &[("k0", 0.75), ("k1", 0.7), ("mix", 0.45)])], &[("educk", 0.6)]),
    ("ECHO - Shimmer Up", &[(M_REVERB, &[("var", 4.0), ("mix", 0.35), ("esend", 0.7)]), (M_ECHO, &[("k0", 0.625), ("k1", 0.65), ("k6", 0.3), ("mix", 0.4)])], &[("epitch", 12.0), ("esmooth", 0.4), ("educk", 0.3)]),
    ("ECHO - Falling Tape", &[(M_MAGNETIC, &[("k0", 0.5), ("k3", 0.5), ("mix", 0.0), ("esend", 0.9)]), (M_ECHO, &[("k0", 0.75), ("k1", 0.6), ("mix", 0.4)])], &[("epitch", -5.0)]),
    ("ECHO - Grain Cloud Ping", &[(M_ECHO, &[("k0", 0.5), ("k1", 0.55), ("k6", 0.8), ("mix", 0.5)]), (M_REVERB, &[("var", 7.0), ("mix", 0.3)])], &[("eping", 1.0)]),
    ("ECHO - Crushed Repeats", &[(M_CRUSH, &[("k0", 0.6), ("k1", 0.5), ("mix", 0.0), ("esend", 1.0)]), (M_LOSS, &[("k0", 0.7), ("mix", 0.0), ("esend", 0.6)]), (M_ECHO, &[("k0", 0.375), ("k1", 0.6), ("mix", 0.45)])], &[("escrub", 0.0)]),
    ("LOFI - Phone Codec Blur", &[(M_LOSS, &[("var", 2.0), ("k0", 0.5), ("k3", 0.5), ("k4", 0.5), ("k5", 0.4)])], &[("cflut", 1.0)]),
    ("LOFI - Robot Formant", &[(M_LOSS, &[("var", 0.0), ("k0", 0.6), ("k1", 0.05), ("k5", 0.9)]), (M_REVERB, &[("var", 2.0), ("mix", 0.15)])], &[]),
    ("LOFI - Worn Tape, Filter Drops", &[(M_MAGNETIC, &[("var", 0.0), ("k0", 0.5), ("k1", 0.6), ("k4", 0.3), ("k5", 1.0), ("k6", 0.2)]), (M_NOISE, &[("var", 1.0), ("k0", 0.35)])], &[]),
    ("LOFI - All Flutter No Wear", &[(M_MAGNETIC, &[("var", 1.0), ("k3", 0.7), ("k4", 1.0)])], &[("mwear", 0.0)]),
    ("FOCUS - Mids Only Grit", &[(M_HEAT, &[("var", 6.0), ("k0", 0.6)]), (M_CRUSH, &[("k0", 0.45), ("mix", 0.5)])], &[("glo", 300.0), ("ghi", 4000.0)]),
    ("CUT - Old Radio Band", &[(M_NOISE, &[("var", 2.0), ("k0", 0.4)]), (M_HEAT, &[("var", 1.0), ("k0", 0.4)])], &[("gcut", 1.0), ("glo", 400.0), ("ghi", 4000.0), ("gshape", 4.0), ("tmode", 2.0), ("tone", -0.6)]),
    ("CUT - Hats Haze (module cut)", &[(M_REVERB, &[("var", 7.0), ("mix", 0.5), ("lo", 3000.0), ("cut", 1.0)]), (M_CHORUS, &[("mix", 0.3)])], &[]),
    ("TONE - Vintage Mic Vocal", &[(M_COMP, &[("var", 0.0), ("k0", 0.35)]), (M_REVERB, &[("var", 0.0), ("mix", 0.12)])], &[("tmode", 3.0), ("tone", 0.6)]),
    // NOISE v2: records with snap, crackle and pop
    ("LOFI - Vinyl B Snap Crackle Pop", &[(M_NOISE, &[("var", 4.0), ("k0", 0.55), ("k1", 0.55), ("k5", 0.5), ("k6", 0.45)]), (M_MAGNETIC, &[("var", 0.0), ("k0", 0.3), ("k3", 0.2)])], &[]),
    ("LOFI - Shellac 78", &[(M_NOISE, &[("var", 5.0), ("k0", 0.6), ("k1", 0.5), ("k2", 0.45), ("k3", 0.0)]), (M_FILTER, &[("var", 3.0), ("k0", 0.6), ("k5", 0.3)])], &[("tmode", 2.0), ("tone", -0.3)]),
    ("DRUMS - Dust Only Crackle", &[(M_NOISE, &[("var", 6.0), ("k0", 0.5), ("k1", 0.5), ("k5", 0.3), ("k6", 0.2), ("k3", 0.6)])], &[]),
    ("FX - Everything (SAFE holds it)", &[(M_HEAT, &[("var", 7.0), ("k0", 0.8)]), (M_CRUSH, &[("k0", 0.7)]), (M_WOBBLE, &[("k0", 0.7)]), (M_MAGNETIC, &[("k1", 0.6)]), (M_ECHO, &[("k1", 0.6)]), (M_REVERB, &[("var", 5.0)]), (M_CHOP, &[]), (M_NOISE, &[("k0", 0.7)])], &[]),
];

fn overrides(p: &Preset) -> Vec<(String, f64)> {
    let mut v = Vec::new();
    let rack: Vec<usize> = p.1.iter().map(|m| m.0).filter(|&m| m != M_ECHO).collect();
    for i in 0..SLOTS {
        v.push((format!("c{}", i + 1), rack.get(i).map(|&m| m as f64 + 1.0).unwrap_or(0.0)));
    }
    if p.1.iter().any(|m| m.0 == M_ECHO) {
        v.push(("eon".to_string(), 1.0));
    }
    for (m, fields) in p.1 {
        for (f, x) in fields.iter() {
            v.push((format!("{}_{}", MODS[*m].id, f), *x));
        }
    }
    for (g, x) in p.2.iter() {
        v.push((g.to_string(), *x));
    }
    v
}

fn build(params: &PatinaParams, over: &[(String, f64)]) -> Value {
    let map = params.param_map();
    let mut out = Map::new();
    for (id, v) in over {
        let ptr = map.iter().find(|(i, _, _)| i == id).unwrap_or_else(|| panic!("no param {id}")).1;
        let val = unsafe {
            match ptr {
                ParamPtr::FloatParam(p) => {
                    let p = &*p;
                    let n = p.preview_normalized(*v as f32);
                    assert!((p.preview_plain(n) - *v as f32).abs() < 1e-3 * (1.0 + v.abs() as f32), "{id} out of range: {v}");
                    json!({ "f32": *v as f32 })
                }
                ParamPtr::IntParam(p) => {
                    let p = &*p;
                    let n = p.preview_normalized(*v as i32);
                    assert_eq!(p.preview_plain(n), *v as i32, "{id} out of range: {v}");
                    json!({ "i32": *v as i32 })
                }
                ParamPtr::BoolParam(_) => json!({ "bool": *v != 0.0 }),
                _ => panic!("{id}: unexpected param kind"),
            }
        };
        out.insert(id.clone(), val);
    }
    json!({ "version": env!("CARGO_PKG_VERSION"), "params": out, "fields": {} })
}

fn state(p: &Preset) -> State {
    let mods: Vec<usize> = p.1.iter().map(|m| m.0).collect();
    let mut s = State::with(&mods);
    for (m, fields) in p.1 {
        let ms = &mut s.m[*m];
        for (f, x) in fields.iter() {
            let x = *x as f32;
            match *f {
                "on" => ms.on = x != 0.0,
                "mix" => ms.mix = x,
                "var" => ms.var = x as usize,
                "lo" => ms.lo = x,
                "hi" => ms.hi = x,
                "flux" => ms.flux = x,
                "cut" => ms.cut = x != 0.0,
                "esend" => ms.esend = x,
                k => ms.k[k[1..].parse::<usize>().unwrap()] = x,
            }
        }
    }
    for (g, x) in p.2.iter() {
        match *g {
            "amount" => s.amount = *x as f32,
            "in" => s.in_db = *x as f32,
            "match" => s.matching = *x != 0.0,
            "eping" => s.x.e_ping = *x != 0.0,
            "escrub" => s.x.e_scrub = *x != 0.0,
            "educk" => s.x.e_duck = *x as f32,
            "esmooth" => s.x.e_smooth = *x as f32,
            "epitch" => s.x.e_pitch = *x as f32,
            "glo" => s.x.g_lo = *x as f32,
            "ghi" => s.x.g_hi = *x as f32,
            "gcut" => s.x.g_cut = *x != 0.0,
            "gshape" => s.x.g_shape = *x as i32,
            "tone" => s.x.tone = *x as f32,
            "tmode" => s.x.tone_mode = *x as i32,
            "mcomp" => s.x.comp = *x as f32,
            "cmodel" => s.x.comp_model = *x as usize,
            "mwear" => s.x.mag_wear = *x != 0.0,
            "cflut" => s.x.cod_flut = *x != 0.0,
            _ => {}
        }
    }
    s
}

#[test]
fn factory_presets_are_valid() {
    let params = PatinaParams::default();
    let dir = std::env::var("PRESET_OUT").ok();
    for p in PRESETS {
        let v = build(&params, &overrides(p));
        let _: nice_plug::plugin::PluginState = serde_json::from_value(v.clone()).unwrap();
        // and it plays: bounded, finite
        let st = state(p);
        let mut e = Engine::new(48000.0, 1);
        let mut s = 1u32;
        let mut l: Vec<f32> = (0..96000)
            .map(|i| {
                s = s.wrapping_mul(1664525).wrapping_add(1013904223);
                0.3 * ((s >> 8) as f32 / 8388608.0 - 1.0) * if (i / 6000) % 2 == 0 { 1.0 } else { 0.1 }
            })
            .collect();
        let mut r = l.clone();
        for (a, b) in l.chunks_mut(512).zip(r.chunks_mut(512)) {
            e.process(a, b, &st, patina::Transport { bpm: Some(92.0), beat: None, playing: false });
        }
        let pk = l.iter().chain(r.iter()).fold(0.0f32, |a, v| a.max(v.abs()));
        assert!(pk.is_finite() && pk <= 1.0, "{}: peak {pk}", p.0);
        if let Some(d) = &dir {
            std::fs::create_dir_all(d).unwrap();
            std::fs::write(format!("{d}/{}.awkp", p.0), serde_json::to_string_pretty(&v).unwrap()).unwrap();
        }
    }
    assert!(PRESETS.len() >= 25);
}
