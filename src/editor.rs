//! Patina editor. Dark by default; LIGHT in the footer flips the body (saved with
//! the session, not in presets). Header and footer are dark bands in both.
//!
//! Top: the SCREEN (in / out spectrum, every module's FOCUS range, latency) and the
//! MASTER section (MAGNITUDE, INPUT, OUTPUT, LOUDNESS MATCH, meters).
//! Middle: the rack, 8 columns in signal order. Each column: header (click = swap /
//! remove, drag = reorder, LED = on / off), flavor, MAGNITUDE, its knobs, FLUX and
//! FOCUS (the frequency range it works on). Empty columns: click to add.

use crate::analyzer::Analyzer;
use crate::awk_ui::*;
use crate::params::*;
use crate::presets::PresetBar;
use crate::{get, Shared};
use nice_plug::prelude::*;
use nice_plug_egui::create_egui_editor;
use egui::{pos2, vec2, Align2, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Painter, Pos2, Rect, Response, Sense, Shape, Stroke, StrokeKind};
use std::f32::consts::{PI, TAU};
use std::sync::atomic::Ordering;
use std::sync::Arc;

pub const WIDTH: u32 = 1120;
pub const HEIGHT: u32 = 814;

const COLW: f32 = 129.0;
const COLG: f32 = 8.0;
const COLX: f32 = 16.0;
const COLY: f32 = 192.0;
const COLH: f32 = 412.0;
const BOTY: f32 = 632.0;
const BOTH: f32 = 128.0;
const ACCENT: Color32 = rgb(0xff, 0x5c, 0xbb);

#[derive(Clone, Copy)]
struct Th {
    bg: Color32,
    bg2: Color32,
    panel: Color32,
    raised: Color32,
    edge: Color32,
    text: Color32,
    dim: Color32,
    faint: Color32,
    knob: Color32,
    knob_hi: Color32,
    shadow: f32,
    light: bool,
}
const DARK: Th = Th {
    bg: rgb(0x0e, 0x0f, 0x12),
    bg2: rgb(0x15, 0x16, 0x1a),
    panel: rgb(0x1a, 0x1c, 0x21),
    raised: rgb(0x24, 0x27, 0x2e),
    edge: rgb(0x2e, 0x32, 0x3a),
    text: rgb(0xe9, 0xec, 0xf1),
    dim: rgb(0x8d, 0x93, 0x9e),
    faint: rgb(0x33, 0x37, 0x40),
    knob: rgb(0x23, 0x26, 0x2c),
    knob_hi: rgb(0x46, 0x4a, 0x54),
    shadow: 0.8,
    light: false,
};
const LIGHT: Th = Th {
    bg: rgb(0xd5, 0xd8, 0xdd),
    bg2: rgb(0xe2, 0xe4, 0xe8),
    panel: rgb(0xec, 0xee, 0xf1),
    raised: rgb(0xf8, 0xf9, 0xfa),
    edge: rgb(0xc0, 0xc5, 0xcc),
    text: rgb(0x1a, 0x1d, 0x23),
    dim: rgb(0x5a, 0x61, 0x6c),
    faint: rgb(0xcf, 0xd3, 0xd9),
    knob: rgb(0xe6, 0xe8, 0xeb),
    knob_hi: rgb(0xff, 0xff, 0xff),
    shadow: 0.35,
    light: true,
};

/// One colour per module.
const MCOL: [Color32; N_MODS] = [
    rgb(0xe8, 0xc5, 0x47), // NOISE
    rgb(0x4f, 0xd3, 0xe8), // DIGITAL
    rgb(0xee, 0x7d, 0x4f), // MAGNETIC
    rgb(0xf0, 0x62, 0x9a), // CHOP
    rgb(0x9d, 0x7c, 0xff), // WOBBLE
    rgb(0x3e, 0xc9, 0xa0), // ECHO
    rgb(0x5b, 0x8d, 0xff), // SPACE
    rgb(0xc2, 0x8b, 0xff), // CHORUS
    rgb(0xff, 0x94, 0x68), // VARISPEED
    rgb(0xff, 0xb6, 0x48), // REVERSE
    rgb(0xff, 0x5a, 0x5a), // DISTORT
    rgb(0x7f, 0xcf, 0x86), // TAME
    rgb(0x4c, 0xc3, 0xd9), // FILTER
    rgb(0x9f, 0xb3, 0xbf), // WIDTH
    rgb(0xee, 0x7d, 0x4f), // CODEC (LOSS)
    rgb(0xe6, 0xdc, 0xc4), // COMPRESS
];
const ON_COL: Color32 = rgb(0x12, 0x13, 0x16);

struct Drag {
    from: usize,
    grab: f32,
    moved: bool,
}

struct St {
    presets: PresetBar,
    skin: Skin,
    an_in: Analyzer,
    an_out: Analyzer,
    /// module picker open for this slot
    pick: Option<usize>,
    /// flavor list open for this module
    flav: Option<usize>,
    drag: Option<Drag>,
}

fn new_state(name: &'static str) -> St {
    let mut skin = Skin::new(Look::Riso, ACCENT, "PATINA", env!("CARGO_PKG_VERSION"), "");
    skin.name_font = Some(head(24.0));
    St { presets: PresetBar::new(name), skin, an_in: Analyzer::default(), an_out: Analyzer::default(), pick: None, flav: None, drag: None }
}

pub(crate) fn create(params: Arc<PatinaParams>, shared: Arc<Shared>) -> Option<crate::nice_app::AwkEditor> {
    let state = params.editor_state.clone();
    let st = new_state("Patina");
    create_egui_editor(state, nice_plug_egui::RepaintNotifier::new(), nice_plug_egui::EguiNiceSettings::default(), crate::nice_app::AwkApp::new(fonts, {
        let mut __st = st;
        Box::new(move |ctx: &mut egui::Ui, setter: &ParamSetter| {
            let st = &mut __st;
            frame(ctx, setter, st, params.as_ref(), shared.as_ref())
        })
    }))
}

fn fonts(ctx: &egui::Context) {
    install_fonts(ctx);
    let mut f = FontDefinitions::default();
    let faces: [(&str, &'static [u8]); 6] = [
        ("awk-d", include_bytes!("../assets/Michroma-Regular.ttf")),
        ("awk-l", include_bytes!("../assets/BarlowCondensed-SemiBold.ttf")),
        ("awk-b", include_bytes!("../assets/BarlowCondensed-Bold.ttf")),
        ("awk-m", include_bytes!("../assets/ShareTechMono-Regular.ttf")),
        ("pat-h", include_bytes!("../assets/ArchivoBlack-Regular.ttf")),
        ("pat-m", include_bytes!("../assets/IBMPlexMono-Medium.ttf")),
    ];
    let fallbacks: Vec<String> = ["Ubuntu-Light", "NotoEmoji-Regular", "emoji-icon-font"].iter().filter(|n| f.font_data.contains_key(**n)).map(|s| s.to_string()).collect();
    for (name, bytes) in faces {
        f.font_data.insert(name.into(), Arc::new(FontData::from_static(bytes)));
        let mut fam = vec![name.to_string()];
        fam.extend(fallbacks.iter().cloned());
        f.families.insert(FontFamily::Name(name.into()), fam);
    }
    for old in ["block", "marker"] {
        let mut fam = vec!["awk-b".to_string()];
        fam.extend(fallbacks.iter().cloned());
        f.families.insert(FontFamily::Name(old.into()), fam);
    }
    if let Some(p) = f.families.get_mut(&FontFamily::Proportional) {
        p.insert(0, "awk-l".into());
    }
    if let Some(m) = f.families.get_mut(&FontFamily::Monospace) {
        m.insert(0, "awk-m".into());
    }
    ctx.set_fonts(f);
}
fn head(s: f32) -> FontId {
    FontId::new(s, FontFamily::Name("pat-h".into()))
}
fn pm(s: f32) -> FontId {
    FontId::new(s, FontFamily::Name("pat-m".into()))
}



// ------------------------------------------------------------------ helpers

fn set_int(setter: &ParamSetter, p: &IntParam, v: i32) {
    setter.begin_set_parameter(p);
    setter.set_parameter(p, v);
    setter.end_set_parameter(p);
}
fn set_bool(setter: &ParamSetter, p: &BoolParam, v: bool) {
    setter.begin_set_parameter(p);
    setter.set_parameter(p, v);
    setter.end_set_parameter(p);
}
fn set_float(setter: &ParamSetter, p: &FloatParam, v: f32) {
    setter.begin_set_parameter(p);
    setter.set_parameter(p, v);
    setter.end_set_parameter(p);
}
fn to_default<P: Param>(setter: &ParamSetter, p: &P) {
    setter.begin_set_parameter(p);
    setter.set_parameter_normalized(p, p.default_normalized_value());
    setter.end_set_parameter(p);
}
fn reset_module(setter: &ParamSetter, mp: &ModParams) {
    to_default(setter, &mp.on);
    to_default(setter, &mp.mix);
    to_default(setter, &mp.var);
    to_default(setter, &mp.lo);
    to_default(setter, &mp.hi);
    for k in mp.knobs() {
        to_default(setter, k);
    }
    to_default(setter, &mp.flux);
    to_default(setter, &mp.cut);
    to_default(setter, &mp.esend);
}
/// The modules in the rack, in slot order.
fn chain_list(p: &PatinaParams) -> Vec<usize> {
    let mut v = Vec::new();
    for c in p.slots() {
        let x = c.value();
        if x > 0 && x - 1 != M_ECHO as i32 && !v.contains(&((x - 1) as usize)) {
            v.push((x - 1) as usize);
        }
    }
    v
}
fn write_chain(setter: &ParamSetter, p: &PatinaParams, list: &[usize]) {
    // old presets keep ECHO in a slot (= ECHO on): moving it to the strip keeps it on
    if p.slots().iter().any(|c| c.value() == M_ECHO as i32 + 1) {
        set_bool(setter, &p.e_on, true);
    }
    for (i, c) in p.slots().iter().enumerate() {
        let v = list.get(i).map(|m| *m as i32 + 1).unwrap_or(0);
        if c.value() != v {
            set_int(setter, c, v);
        }
    }
}


/// Knob names short enough for three knobs in a row.
fn short_name(s: &'static str) -> &'static str {
    match s {
        "PACKET LOSS" => "PACKETS",
        "PRE-DELAY" => "PRE-DLY",
        "THRU RACK" => "THRU",
        "MONO BASS" => "MONO LO",
        "SATURATE" => "SATUR.",
        "DROPOUTS" => "DROPS",
        "VOL<>FILT" => "VOL/FILT",
        _ => s,
    }
}
fn hzs(f: f32) -> String {
    if f >= 1000.0 {
        let k = f / 1000.0;
        if k >= 10.0 { format!("{:.0}k", k) } else { format!("{:.1}k", k) }
    } else {
        format!("{:.0}", f)
    }
}
fn rr(r: f32) -> CornerRadius {
    CornerRadius::same(r as u8)
}
fn slot_rect(o: egui::Vec2, i: usize) -> Rect {
    Rect::from_min_size(pos2(COLX + i as f32 * (COLW + COLG), COLY) + o, vec2(COLW, COLH))
}

/// The knob: soft-shadowed cap, coloured value arc, pointer, label and value.
#[allow(clippy::too_many_arguments)]
fn knob(ui: &mut egui::Ui, setter: &ParamSetter, th: &Th, c: Pos2, r: f32, param: &FloatParam, col: Color32, label: &str, value: &str, id: impl std::hash::Hash + std::fmt::Debug, dim: bool) -> Response {
    let rect = Rect::from_center_size(c, vec2(r * 2.0 + 14.0, r * 2.0 + 14.0));
    let uid = ui.scope_id().with(("kn", id));
    let resp = ui.interact(rect, uid, Sense::click_and_drag());
    knob_input(ui, setter, &resp, uid.with("d"), param);
    let pa = ui.painter();
    let v = param.unmodulated_normalized_value();
    let hot = resp.hovered() || resp.dragged();
    let col = if dim { al(col, 0.35) } else { col };
    let tw = if r > 20.0 { 4.0 } else { 3.0 };
    let ar = r + tw + 2.0;
    pa.add(Shape::line(arc_pts(c, ar, ang(0.0), ang(1.0), 48), Stroke::new(tw, th.faint)));
    if v > 0.003 {
        if !th.light && !dim {
            pa.add(Shape::line(arc_pts(c, ar, ang(0.0), ang(v), 48), Stroke::new(tw + 4.0, al(col, 0.16))));
        }
        pa.add(Shape::line(arc_pts(c, ar, ang(0.0), ang(v), 48), Stroke::new(tw, col)));
    }
    disc_shadow(pa, c, r, vec2(0.0, 2.5), th.shadow);
    disc_radial(pa, c, r, th.knob_hi, th.knob, vec2(-r * 0.3, -r * 0.4));
    pa.circle_stroke(c, r, Stroke::new(1.0, if th.light { th.edge } else { Color32::from_black_alpha(160) }));
    pa.circle_stroke(c, r - 1.5, Stroke::new(1.0, Color32::from_white_alpha(if th.light { 120 } else { 14 })));
    let a = ang(v);
    pa.line_segment([polar(c, a, r * 0.28), polar(c, a, r * 0.84)], Stroke::new(if r > 20.0 { 3.0 } else { 2.2 }, if dim { th.dim } else { th.text }));
    if !label.is_empty() {
        txt(pa, c + vec2(0.0, r + 13.0), Align2::CENTER_CENTER, label, labb(if r > 20.0 { 11.5 } else { 9.5 }), if dim { th.faint } else { th.dim }, if r > 20.0 { 0.8 } else { 0.3 });
    }
    txt(pa, c + vec2(0.0, r + if r > 20.0 { 27.0 } else { 24.0 }), Align2::CENTER_CENTER, value, mono(if r > 20.0 { 11.0 } else { 9.5 }), if hot { col } else if dim { th.faint } else { th.text }, 0.0);
    resp
}

// ------------------------------------------------------------------ frame

fn frame(root: &mut egui::Ui, setter: &ParamSetter, st: &mut St, p: &PatinaParams, sh: &Shared) {
    let ctx_owned = root.ctx().clone();
    let ctx = &ctx_owned;
    ctx.request_repaint_after(std::time::Duration::from_millis(33));
    st.skin.begin(ctx);
    st.an_in.update(&sh.spec_in);
    st.an_out.update(&sh.spec_out);
    let th = if p.paper.load(Ordering::Relaxed) { LIGHT } else { DARK };
    let t = ctx.input(|i| i.time) as f32;
    egui::CentralPanel::default().frame(egui::Frame::NONE.fill(th.bg)).show(root, |ui| {
        let whole = ui.max_rect();
        let o = whole.min.to_vec2();
        let at = |x: f32, y: f32| pos2(x, y) + o;
        let pa = ui.painter().clone();
        vgrad(&pa, whole, th.bg2, th.bg);
        let list = chain_list(p);
        screen(ui, st, p, sh, &th, &list, Rect::from_min_size(at(16.0, 54.0), vec2(800.0, 128.0)), t);
        master(ui, setter, st, p, sh, &th, Rect::from_min_size(at(828.0, 54.0), vec2(276.0, 128.0)));
        echo_strip(ui, setter, st, p, sh, &th, Rect::from_min_size(at(16.0, BOTY), vec2(720.0, BOTH)), &list);
        finish(ui, setter, p, sh, &th, Rect::from_min_size(at(744.0, BOTY), vec2(360.0, BOTH)));
        for i in 0..SLOTS {
            match list.get(i) {
                Some(&m) => column(ui, setter, st, p, sh, &th, &list, i, m, o, t),
                None => empty_column(ui, st, &th, i, i == list.len(), o),
            }
        }
        drag_overlay(ui, setter, st, p, &th, &list, o);
        footer(ui, setter, st, p, sh, whole);
        if let Some(m) = st.flav {
            flavor_menu(ui, setter, st, p, &th, &list, m, o);
        }
        if let Some(slot) = st.pick {
            picker(ui, setter, st, p, &th, &list, slot, o, t);
        }
        // header last so its preset list draws over everything
        let _ = st.presets.strip(ui, setter, p, whole, &st.skin, &p.mix, &p.bypass);
    });
}

// ------------------------------------------------------------------ screen

#[allow(clippy::too_many_arguments)]
fn screen(ui: &mut egui::Ui, st: &St, p: &PatinaParams, sh: &Shared, th: &Th, list: &[usize], r: Rect, t: f32) {
    let pa = ui.painter().clone();
    let scr = rgb(0x09, 0x0a, 0x0d);
    pa.rect_filled(r.translate(vec2(0.0, 2.0)), rr(8.0), Color32::from_black_alpha(if th.light { 40 } else { 120 }));
    pa.rect_filled(r, rr(8.0), scr);
    pa.rect_stroke(r, rr(8.0), Stroke::new(1.0, if th.light { rgb(0x2a, 0x2d, 0x33) } else { th.edge }), StrokeKind::Inside);
    let inner = r.shrink2(vec2(14.0, 10.0));
    let (x0, x1) = (inner.left() + 24.0, inner.right());
    let bars = inner.bottom() - list.len().max(1) as f32 * 5.0 + 2.0;
    let top = inner.top() + 16.0;
    let bot = bars - 16.0;
    let mapx = |u: f32| x0 + u * (x1 - x0);
    let mapy = |db: f32| bot - ((db + 78.0) / 78.0).clamp(0.0, 1.0) * (bot - top);
    let grid = rgb(0x1b, 0x1e, 0x24);
    for f in [50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0] {
        let x = mapx(fu(f));
        line(&pa, pos2(x, top), pos2(x, bot), grid, 1.0);
        txt(&pa, pos2(x, bot + 7.0), Align2::CENTER_CENTER, hzs(f), mono(8.0), rgb(0x5b, 0x61, 0x6b), 0.0);
    }
    for d in [-18.0, -42.0, -66.0] {
        let y = mapy(d);
        line(&pa, pos2(x0, y), pos2(x1, y), grid, 1.0);
        txt(&pa, pos2(x0 - 4.0, y), Align2::RIGHT_CENTER, format!("{}", d as i32), mono(8.0), rgb(0x4b, 0x51, 0x5b), 0.0);
    }
    let n = 180;
    let pts = |an: &Analyzer| -> Vec<Pos2> { (0..=n).map(|i| i as f32 / n as f32).map(|u| pos2(mapx(u), mapy(an.at(u)))).collect() };
    let pin = pts(&st.an_in);
    fill_under(&pa, &pin, bot, Color32::from_white_alpha(14));
    polyline(&pa, pin, Color32::from_white_alpha(60), 1.0, false);
    let pout = pts(&st.an_out);
    fill_under(&pa, &pout, bot, al(ACCENT, 0.10));
    polyline(&pa, pout, ACCENT, 1.6, true);
    // global FOCUS / CUT range
    let (glo, ghi) = (p.g_lo.value(), p.g_hi.value());
    if glo > 20.5 || ghi < 19900.0 {
        let (a, b) = (mapx(fu(glo)), mapx(fu(ghi)));
        let shade = Color32::from_black_alpha(if p.g_cut.value() { 170 } else { 110 });
        pa.rect_filled(Rect::from_min_max(pos2(x0, top), pos2(a, bot)), 0.0, shade);
        pa.rect_filled(Rect::from_min_max(pos2(b, top), pos2(x1, bot)), 0.0, shade);
        for x in [a, b] {
            line(&pa, pos2(x, top), pos2(x, bot), al(ACCENT, 0.8), 1.0);
        }
        txt(&pa, pos2((a + b) * 0.5, top + 6.0), Align2::CENTER_CENTER, if p.g_cut.value() { "CUT" } else { "FOCUS" }, mono(8.5), ACCENT, 1.0);
    }
    // every module's FOCUS range, in its colour, in rack order
    for (j, &m) in list.iter().enumerate() {
        let mp = p.m(m);
        let y = bars + j as f32 * 5.0;
        let (a, b) = (mapx(fu(mp.lo.value())), mapx(fu(mp.hi.value())));
        let fl = get(&sh.flux[m]);
        let col = if mp.on.value() { MCOL[m] } else { al(MCOL[m], 0.25) };
        pa.rect_filled(Rect::from_min_max(pos2(x0, y), pos2(x1, y + 3.0)), 0.0, rgb(0x16, 0x18, 0x1d));
        pa.rect_filled(Rect::from_min_max(pos2(a, y), pos2(b.max(a + 2.0), y + 3.0)), 1.0, col);
        if mp.flux.value() > 0.001 {
            let fx = a + (b - a) * (0.5 + 0.5 * fl.clamp(-1.0, 1.0));
            pa.circle_filled(pos2(fx, y + 1.5), 2.5, Color32::WHITE);
        }
    }
    let lat = sh.lat.load(Ordering::Relaxed) as f32;
    let sr = get(&sh.spec_in.sr).max(1.0);
    let head_s = format!("{} MODULES  ·  {:.0} BPM  ·  LATENCY {:.1} ms", list.len(), get(&sh.bpm), lat / sr * 1000.0);
    txt(&pa, pos2(inner.left(), inner.top() + 4.0), Align2::LEFT_CENTER, head_s, mono(9.5), rgb(0x8d, 0x93, 0x9e), 1.0);
    txt(&pa, pos2(inner.right() - 4.0, inner.top() + 4.0), Align2::RIGHT_CENTER, "IN", mono(9.5), Color32::from_white_alpha(110), 1.0);
    txt(&pa, pos2(inner.right() - 30.0, inner.top() + 4.0), Align2::RIGHT_CENTER, "OUT", mono(9.5), ACCENT, 1.0);
    let _ = t;
}

// ------------------------------------------------------------------ master

#[allow(clippy::too_many_arguments)]
fn master(ui: &mut egui::Ui, setter: &ParamSetter, st: &mut St, p: &PatinaParams, sh: &Shared, th: &Th, r: Rect) {
    let pa = ui.painter().clone();
    pa.rect_filled(r.translate(vec2(0.0, 2.0)), rr(8.0), Color32::from_black_alpha(if th.light { 30 } else { 90 }));
    pa.rect_filled(r, rr(8.0), th.panel);
    pa.rect_stroke(r, rr(8.0), Stroke::new(1.0, th.edge), StrokeKind::Inside);
    let at = |x: f32, y: f32| r.min + vec2(x, y);
    txt(&pa, at(14.0, 14.0), Align2::LEFT_CENTER, "MASTER", labb(12.0), th.dim, 2.0);
    let a = p.amount.value();
    knob(ui, setter, th, at(62.0, 62.0), 28.0, &p.amount, ACCENT, "MAGNITUDE", &format!("{:.0}%", a * 100.0), "mag", false);
    knob(ui, setter, th, at(150.0, 40.0), 14.0, &p.input, th.text, "INPUT", &format!("{:+.1} dB", p.input.value()), "in", false);
    knob(ui, setter, th, at(210.0, 40.0), 14.0, &p.out, th.text, "OUTPUT", &format!("{:+.1} dB", p.out.value()), "out", false);
    let on = p.matching.value();
    let mr = Rect::from_min_size(at(124.0, 94.0), vec2(112.0, 22.0));
    let resp = ui.interact(mr, ui.scope_id().with("match"), Sense::click()).on_hover_text("Loudness match: the rack comes out as loud as it went in");
    if resp.clicked() {
        set_bool(setter, &p.matching, !on);
    }
    pa.rect_filled(mr, rr(5.0), if on { ACCENT } else { th.raised });
    pa.rect_stroke(mr, rr(5.0), Stroke::new(1.0, if on { ACCENT } else { th.edge }), StrokeKind::Inside);
    txt(&pa, mr.center(), Align2::CENTER_CENTER, "LOUDNESS MATCH", labb(11.0), if on { ON_COL } else { th.dim }, 1.0);
    // meters
    let m = &sh.meters;
    for (k, (lab, v)) in [("I", m.db(0).max(m.db(1))), ("O", m.db(2).max(m.db(3)))].iter().enumerate() {
        let x = r.right() - 30.0 + k as f32 * 12.0;
        let (y0, y1) = (r.top() + 14.0, r.bottom() - 24.0);
        let mr = Rect::from_min_max(pos2(x, y0), pos2(x + 7.0, y1));
        pa.rect_filled(mr, rr(2.0), if th.light { th.faint } else { rgb(0x0c, 0x0d, 0x10) });
        let f = ((v + 60.0) / 60.0).clamp(0.0, 1.0);
        if f > 0.0 {
            let fr = Rect::from_min_max(pos2(x, y1 - f * (y1 - y0)), pos2(x + 7.0, y1));
            vgrad(&pa, fr, if *v > -3.0 { ACCENT } else { rgb(0xa8, 0xe6, 0x8a) }, rgb(0x3e, 0xc9, 0xa0));
        }
        txt(&pa, pos2(x + 3.5, y1 + 9.0), Align2::CENTER_CENTER, *lab, mono(9.0), th.dim, 0.0);
    }
    let _ = st;
}

// ------------------------------------------------------------------ columns

#[allow(clippy::too_many_arguments)]
fn column(ui: &mut egui::Ui, setter: &ParamSetter, st: &mut St, p: &PatinaParams, sh: &Shared, th: &Th, list: &[usize], i: usize, m: usize, o: egui::Vec2, _t: f32) {
    let r = slot_rect(o, i);
    let pa = ui.painter().clone();
    let mp = p.m(m);
    let d = &MODS[m];
    let col = MCOL[m];
    let on = mp.on.value();
    let is_loss = m == M_MAGNETIC || m == M_LOSS;
    let dragging_me = st.drag.as_ref().map(|d| d.from == i && d.moved).unwrap_or(false);
    // body
    pa.rect_filled(r.translate(vec2(0.0, 3.0)), rr(8.0), Color32::from_black_alpha(if th.light { 28 } else { 110 }));
    pa.rect_filled(r, rr(8.0), th.panel);
    pa.rect_stroke(r, rr(8.0), Stroke::new(1.0, if dragging_me { col } else { th.edge }), StrokeKind::Inside);
    // header
    let hr = Rect::from_min_size(r.min, vec2(COLW, 30.0));
    let hresp = ui.interact(hr, ui.scope_id().with(("hdr", i)), Sense::click_and_drag()).on_hover_text("Click: swap or remove · drag: move it in the rack");
    let hcol = if on { col } else { mix_col(col, th.panel, 0.72) };
    pa.rect_filled(hr, CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 }, hcol);
    if !th.light && on {
        vgrad(&pa, Rect::from_min_max(hr.min + vec2(1.0, 1.0), pos2(hr.right() - 1.0, hr.top() + 12.0)), Color32::from_white_alpha(28), Color32::TRANSPARENT);
    }
    txt(&pa, hr.center() + vec2(4.0, 0.0), Align2::CENTER_CENTER, ui_name(m), labb(if ui_name(m).len() > 7 { 13.5 } else { 15.0 }), if on { ON_COL } else { th.dim }, 1.4);
    for gy in 0..3 {
        for gx in 0..2 {
            pa.circle_filled(pos2(hr.right() - 14.0 + gx as f32 * 4.0, hr.center().y - 4.0 + gy as f32 * 4.0), 1.1, if on { al(ON_COL, 0.55) } else { th.faint });
        }
    }
    // LED (power)
    let lc = pos2(hr.left() + 14.0, hr.center().y);
    let lresp = ui.interact(Rect::from_center_size(lc, vec2(20.0, 22.0)), ui.scope_id().with(("led", i)), Sense::click()).on_hover_text(if on { "Turn off" } else { "Turn on" });
    pa.circle_filled(lc, 6.0, if on { ON_COL } else { th.raised });
    pa.circle_filled(lc, 3.5, if on { rgb(0xf4, 0xff, 0xf0) } else { th.faint });
    if lresp.clicked() {
        set_bool(setter, &mp.on, !on);
    } else if hresp.clicked() {
        st.pick = Some(i);
        st.flav = None;
    }
    if hresp.drag_started() {
        let px = ui.ctx().pointer_interact_pos().map(|q| q.x).unwrap_or(hr.left());
        st.drag = Some(Drag { from: i, grab: px - hr.left(), moved: false });
    }
    if hresp.dragged() {
        if let Some(dg) = st.drag.as_mut() {
            if hresp.drag_delta().length() > 0.0 {
                dg.moved = true;
            }
        }
    }
    if hresp.drag_stopped() {
        if let Some(dg) = st.drag.take() {
            if dg.moved {
                let to = drop_index(ui, &dg, o, list.len());
                if to != dg.from {
                    let mut l = list.to_vec();
                    let x = l.remove(dg.from);
                    l.insert(to.min(l.len()), x);
                    write_chain(setter, p, &l);
                }
            }
        }
    }
    let at = |y: f32| r.top() + y;
    let cx = r.center().x;
    let dimk = !on;
    // flavor (LOSS: MAGNETIC / CODEC switch first)
    let mut fr = Rect::from_min_size(pos2(r.left() + 8.0, at(38.0)), vec2(COLW - 16.0, 22.0));
    if is_loss {
        let mr = Rect::from_min_size(fr.min, vec2(52.0, 22.0));
        let lbl = if m == M_MAGNETIC { "MAGNETIC" } else { "CODEC" };
        if chip(ui, &pa, th, mr, lbl, true, col, ("lmode", i)).on_hover_text("LOSS mode: MAGNETIC (tape) or CODEC (digital)").clicked() {
            let other = if m == M_MAGNETIC { M_LOSS } else { M_MAGNETIC };
            let mut l = list.to_vec();
            l.retain(|&x| x != other || x == m);
            if let Some(j) = l.iter().position(|&x| x == m) {
                l[j] = other;
            }
            set_bool(setter, &p.m(other).on, on);
            write_chain(setter, p, &l);
        }
        fr = Rect::from_min_max(pos2(mr.right() + 3.0, fr.top()), fr.max);
    }
    if !d.vars.is_empty() {
        let var = mp.var.value().clamp(0, d.vars.len() as i32 - 1);
        pa.rect_filled(fr, rr(5.0), th.raised);
        pa.rect_stroke(fr, rr(5.0), Stroke::new(1.0, th.edge), StrokeKind::Inside);
        let nv = d.vars.len() as i32;
        let aw = if is_loss { 0.0 } else { 20.0 };
        let arrows: Vec<(Rect, &str)> = if is_loss { Vec::new() } else { vec![(Rect::from_min_size(fr.min, vec2(aw, 22.0)), "‹"), (Rect::from_min_size(pos2(fr.right() - aw, fr.top()), vec2(aw, 22.0)), "›")] };
        for (k, (ar, s)) in arrows.iter().enumerate() {
            let resp = ui.interact(*ar, ui.scope_id().with(("fv", i, k)), Sense::click());
            txt(&pa, ar.center() + vec2(0.0, -1.0), Align2::CENTER_CENTER, *s, labb(17.0), if resp.hovered() { col } else { th.dim }, 0.0);
            if resp.clicked() {
                let nvv = if k == 0 { (var - 1 + nv) % nv } else { (var + 1) % nv };
                set_int(setter, &mp.var, nvv);
            }
        }
        let mid = Rect::from_min_max(pos2(fr.left() + aw, fr.top()), pos2(fr.right() - aw, fr.bottom()));
        let resp = ui.interact(mid, ui.scope_id().with(("fvm", i)), Sense::click()).on_hover_text("Pick the flavor");
        let name = d.vars[var as usize];
        txt(&pa, mid.center(), Align2::CENTER_CENTER, name, labb(if name.len() > 9 || is_loss && name.len() > 6 { 10.5 } else { 12.0 }), if resp.hovered() { col } else { th.text }, 0.6);
        if resp.clicked() {
            st.flav = if st.flav == Some(m) { None } else { Some(m) };
            st.pick = None;
        }
    } else {
        txt(&pa, fr.center(), Align2::CENTER_CENTER, "· · ·", labb(12.0), th.faint, 1.0);
    }
    // MAGNITUDE
    let mix = mp.mix.value();
    let mag_val = if m == M_COMP && on {
        let gr = get(&sh.comp_gr);
        if gr > 0.1 { format!("{:.0}% · -{:.1}", mix * 100.0, gr) } else { format!("{:.0}%", mix * 100.0) }
    } else {
        format!("{:.0}%", mix * 100.0)
    };
    knob(ui, setter, th, pos2(cx, at(98.0)), 26.0, &mp.mix, col, "MAGNITUDE", &mag_val, ("mix", m), dimk);
    // knobs, 3 per row (LOSS: one of them is a balance bar)
    let var = mp.var.value().max(0) as usize;
    let ks = mp.knobs();
    let bar = match m {
        M_MAGNETIC => Some((4usize, "WEAR", "FLUTTER")),
        M_LOSS => Some((5usize, "LOSSY", "FORMANT")),
        _ => None,
    };
    let idx: Vec<usize> = (0..d.knobs.len()).filter(|&k| bar.map(|b| b.0 != k).unwrap_or(true)).collect();
    let nk = idx.len();
    for (n, &k) in idx.iter().enumerate() {
        let row = n / 3;
        let in_row = (nk - row * 3).min(3);
        let j = n % 3;
        let x = cx + (j as f32 - (in_row as f32 - 1.0) / 2.0) * 41.0;
        let y = at(176.0 + row as f32 * 58.0);
        let v = ks[k].value();
        knob(ui, setter, th, pos2(x, y), 12.0, ks[k], col, short_name(knob_name(m, k, var)), &knob_text(m, k, v, var), ("k", m, k), dimk);
    }
    if let Some((k, a, b)) = bar {
        let y = at(292.0);
        txt(&pa, pos2(r.left() + 10.0, y - 10.0), Align2::LEFT_CENTER, a, labb(9.5), if dimk { th.faint } else { th.dim }, 0.6);
        txt(&pa, pos2(r.right() - 10.0, y - 10.0), Align2::RIGHT_CENTER, b, labb(9.5), if dimk { th.faint } else { th.dim }, 0.6);
        let tr = Rect::from_min_max(pos2(r.left() + 12.0, y - 3.0), pos2(r.right() - 12.0, y + 3.0));
        let resp = ui.interact(tr.expand2(vec2(6.0, 8.0)), ui.scope_id().with(("bar", m)), Sense::click_and_drag()).on_hover_text(knob_text(m, k, ks[k].value(), var));
        hslider(ui, setter, &resp, ks[k], tr);
        let v = ks[k].value();
        pa.rect_filled(tr, rr(3.0), th.faint);
        let cxm = tr.center().x;
        let hx = tr.left() + v * tr.width();
        pa.rect_filled(Rect::from_min_max(pos2(cxm.min(hx), tr.top()), pos2(cxm.max(hx), tr.bottom())), rr(3.0), al(col, if dimk { 0.4 } else { 1.0 }));
        line(&pa, pos2(cxm, tr.top() - 3.0), pos2(cxm, tr.bottom() + 3.0), th.dim, 1.0);
        pa.circle_filled(pos2(hx, tr.center().y), 6.0, th.knob_hi);
        pa.circle_stroke(pos2(hx, tr.center().y), 6.0, Stroke::new(1.0, th.edge));
        if resp.hovered() || resp.dragged() {
            txt(&pa, pos2(cxm, y + 12.0), Align2::CENTER_CENTER, knob_text(m, k, v, var), mono(9.0), col, 0.0);
        }
        // WEAR / FLUTTER on / off
        let (w, f) = if m == M_MAGNETIC { (&p.mag_wear, &p.mag_flut) } else { (&p.cod_wear, &p.cod_flut) };
        for (n, (prm, lbl)) in [(w, "WEAR"), (f, "FLUTTER")].into_iter().enumerate() {
            let cr = Rect::from_min_size(pos2(r.left() + 10.0 + n as f32 * 56.0, at(318.0)), vec2(53.0, 18.0));
            if chip(ui, &pa, th, cr, lbl, prm.value(), col, ("tg", m, n)).clicked() {
                set_bool(setter, prm, !prm.value());
            }
        }
    }
    // FLUX
    let fy = at(352.0);
    txt(&pa, pos2(r.left() + 10.0, fy), Align2::LEFT_CENTER, "FLUX", labb(10.0), if dimk { th.faint } else { th.dim }, 1.0);
    let tr = Rect::from_min_max(pos2(r.left() + 42.0, fy - 3.0), pos2(r.right() - 10.0, fy + 3.0));
    let fresp = ui.interact(tr.expand2(vec2(4.0, 7.0)), ui.scope_id().with(("flux", m)), Sense::click_and_drag()).on_hover_text("FLUX: the module's main knob drifts around by itself");
    hslider(ui, setter, &fresp, &mp.flux, tr);
    let fv = mp.flux.value();
    pa.rect_filled(tr, rr(3.0), th.faint);
    if fv > 0.002 {
        pa.rect_filled(Rect::from_min_max(tr.min, pos2(tr.left() + fv * tr.width(), tr.bottom())), rr(3.0), al(col, if dimk { 0.4 } else { 1.0 }));
        let w = get(&sh.flux[m]).clamp(-1.0, 1.0);
        pa.circle_filled(pos2(tr.left() + fv * tr.width() * (0.5 + 0.5 * w), tr.center().y), 2.0, if th.light { ON_COL } else { Color32::WHITE });
    }
    pa.circle_filled(pos2(tr.left() + fv * tr.width(), tr.center().y), 5.0, th.knob_hi);
    pa.circle_stroke(pos2(tr.left() + fv * tr.width(), tr.center().y), 5.0, Stroke::new(1.0, th.edge));
    if fresp.hovered() || fresp.dragged() {
        txt(&pa, pos2(tr.right(), fy - 12.0), Align2::RIGHT_CENTER, format!("{:.0}%", fv * 100.0), mono(9.0), col, 0.0);
    }
    // FOCUS / CUT: same range, the chip says what happens outside it
    let gy = at(374.0);
    let (lo, hi) = (mp.lo.value(), mp.hi.value());
    let full = lo <= 20.5 && hi >= 19900.0;
    let cut = mp.cut.value();
    let cr = Rect::from_min_size(pos2(r.left() + 8.0, gy - 8.0), vec2(42.0, 16.0));
    if chip(ui, &pa, th, cr, if cut { "CUT" } else { "FOCUS" }, cut, col, ("fc", m)).on_hover_text("FOCUS: the module only works inside the range · CUT: outside the range is removed").clicked() {
        set_bool(setter, &mp.cut, !cut);
    }
    txt(&pa, pos2(r.right() - 10.0, gy), Align2::RIGHT_CENTER, if full { "FULL".to_string() } else { format!("{}–{}", hzs(lo), hzs(hi)) }, mono(9.0), if full { th.dim } else { col }, 0.0);
    let br = Rect::from_min_max(pos2(r.left() + 10.0, gy + 10.0), pos2(r.right() - 10.0, gy + 24.0));
    focus_strip(ui, setter, th, &pa, &mp.lo, &mp.hi, br, col, ("m", m), dimk, cut);
    if !on && !th.light {
        pa.rect_filled(Rect::from_min_max(pos2(r.left() + 1.0, hr.bottom()), r.max - vec2(1.0, 1.0)), CornerRadius { nw: 0, ne: 0, sw: 8, se: 8 }, al(th.panel, 0.35));
    }
    // ECHO send, just under the module: how much of the repeats go through it
    let sy = r.bottom() + 13.0;
    let es = mp.esend.value();
    let ecol = MCOL[M_ECHO];
    let echo_on = p.e_on.value() || p.slots().iter().any(|c| c.value() == M_ECHO as i32 + 1);
    txt(&pa, pos2(r.left() + 4.0, sy), Align2::LEFT_CENTER, "ECHO", labb(9.0), if es > 0.0 && echo_on { ecol } else { th.dim }, 0.8);
    let tr = Rect::from_min_max(pos2(r.left() + 36.0, sy - 2.5), pos2(r.right() - 6.0, sy + 2.5));
    let resp = ui.interact(tr.expand2(vec2(4.0, 7.0)), ui.scope_id().with(("es", m)), Sense::click_and_drag()).on_hover_text("ECHO send: every repeat goes back through this module");
    hslider(ui, setter, &resp, &mp.esend, tr);
    pa.rect_filled(tr, rr(2.5), th.faint);
    if es > 0.002 {
        pa.rect_filled(Rect::from_min_max(tr.min, pos2(tr.left() + es * tr.width(), tr.bottom())), rr(2.5), if echo_on { ecol } else { al(ecol, 0.4) });
    }
    pa.circle_filled(pos2(tr.left() + es * tr.width(), tr.center().y), 4.5, th.knob_hi);
    pa.circle_stroke(pos2(tr.left() + es * tr.width(), tr.center().y), 4.5, Stroke::new(1.0, if es > 0.0 { ecol } else { th.edge }));
    if resp.hovered() || resp.dragged() {
        txt(&pa, pos2(tr.right(), sy - 11.0), Align2::RIGHT_CENTER, format!("{:.0}%", es * 100.0), mono(9.0), ecol, 0.0);
    }
}

/// Small toggle chip.
fn chip(ui: &mut egui::Ui, pa: &Painter, th: &Th, r: Rect, label: &str, on: bool, col: Color32, id: impl std::hash::Hash + std::fmt::Debug) -> Response {
    let resp = ui.interact(r, ui.scope_id().with(("chip", id)), Sense::click());
    let hot = resp.hovered();
    pa.rect_filled(r, rr(4.0), if on { col } else if hot { th.faint } else { th.raised });
    pa.rect_stroke(r, rr(4.0), Stroke::new(1.0, if on { col } else { th.edge }), StrokeKind::Inside);
    txt(pa, r.center(), Align2::CENTER_CENTER, label, labb(if r.width() < 48.0 { 9.0 } else { 10.0 }), if on { ON_COL } else { th.dim }, 0.5);
    resp
}

// ------------------------------------------------------------------ ECHO strip

#[allow(clippy::too_many_arguments)]
fn echo_strip(ui: &mut egui::Ui, setter: &ParamSetter, st: &mut St, p: &PatinaParams, sh: &Shared, th: &Th, r: Rect, list: &[usize]) {
    let pa = ui.painter().clone();
    let mp = p.m(M_ECHO);
    let col = MCOL[M_ECHO];
    let legacy = p.slots().iter().any(|c| c.value() == M_ECHO as i32 + 1);
    let on = (p.e_on.value() || legacy) && mp.on.value();
    let dim = !on;
    pa.rect_filled(r.translate(vec2(0.0, 3.0)), rr(8.0), Color32::from_black_alpha(if th.light { 28 } else { 110 }));
    pa.rect_filled(r, rr(8.0), th.panel);
    pa.rect_stroke(r, rr(8.0), Stroke::new(1.0, if on { al(col, 0.6) } else { th.edge }), StrokeKind::Inside);
    let at = |x: f32, y: f32| r.min + vec2(x, y);
    // header block
    let hr = Rect::from_min_size(r.min, vec2(116.0, 28.0));
    pa.rect_filled(hr, CornerRadius { nw: 8, ne: 0, sw: 0, se: 8 }, if on { col } else { mix_col(col, th.panel, 0.72) });
    txt(&pa, hr.center() + vec2(8.0, 0.0), Align2::CENTER_CENTER, "ECHO", labb(15.0), if on { ON_COL } else { th.dim }, 2.0);
    let lc = pos2(hr.left() + 14.0, hr.center().y);
    let lresp = ui.interact(hr, ui.scope_id().with("e-led"), Sense::click()).on_hover_text(if on { "Turn ECHO off" } else { "Turn ECHO on" });
    pa.circle_filled(lc, 6.0, if on { ON_COL } else { th.raised });
    pa.circle_filled(lc, 3.5, if on { rgb(0xf4, 0xff, 0xf0) } else { th.faint });
    if lresp.clicked() {
        if on {
            set_bool(setter, &p.e_on, false);
            if legacy {
                let l: Vec<usize> = list.to_vec();
                for (i, c) in p.slots().iter().enumerate() {
                    set_int(setter, c, l.get(i).map(|m| *m as i32 + 1).unwrap_or(0));
                }
            }
        } else {
            set_bool(setter, &p.e_on, true);
            set_bool(setter, &mp.on, true);
        }
    }
    // flavor
    let fr = Rect::from_min_size(at(8.0, 36.0), vec2(100.0, 20.0));
    let vars = MODS[M_ECHO].vars;
    let var = mp.var.value().clamp(0, vars.len() as i32 - 1);
    pa.rect_filled(fr, rr(5.0), th.raised);
    pa.rect_stroke(fr, rr(5.0), Stroke::new(1.0, th.edge), StrokeKind::Inside);
    let resp = ui.interact(fr, ui.scope_id().with("e-var"), Sense::click()).on_hover_text("Click: next flavor");
    txt(&pa, fr.center(), Align2::CENTER_CENTER, vars[var as usize], labb(11.0), if resp.hovered() { col } else { th.text }, 0.8);
    if resp.clicked() {
        set_int(setter, &mp.var, (var + 1) % vars.len() as i32);
    }
    // SCRUB / PING
    for (n, (prm, lbl, tip)) in [(&p.e_scrub, "SCRUB", "SCRUB: TIME changes slide like tape (pitch bends) instead of jumping"), (&p.e_ping, "PING", "Ping-pong: repeats bounce left / right")].into_iter().enumerate() {
        let cr = Rect::from_min_size(at(8.0 + n as f32 * 51.0, 62.0), vec2(49.0, 18.0));
        if chip(ui, &pa, th, cr, lbl, prm.value(), col, ("e-tg", n)).on_hover_text(tip).clicked() {
            set_bool(setter, prm, !prm.value());
        }
    }
    // which modules the repeats go through
    txt(&pa, at(8.0, 98.0), Align2::LEFT_CENTER, "THRU", labb(9.0), th.dim, 0.8);
    let mut x = 40.0;
    for &m in list {
        let s = p.m(m).esend.value().max(mp.k5.value());
        if s > 0.001 {
            let c = pos2(r.left() + x, r.top() + 98.0);
            pa.circle_filled(c, 4.5, al(MCOL[m], (0.35 + 0.65 * s) * if dim { 0.4 } else { 1.0 }));
            x += 11.0;
        }
    }
    if x == 40.0 {
        txt(&pa, at(40.0, 98.0), Align2::LEFT_CENTER, "none", mono(9.0), th.faint, 0.0);
    }
    // MAGNITUDE + knobs
    let mix = mp.mix.value();
    knob(ui, setter, th, at(154.0, 46.0), 20.0, &mp.mix, col, "MAGNITUDE", &format!("{:.0}%", mix * 100.0), "e-mix", dim);
    let ks = mp.knobs();
    let v = var as usize;
    let pitch = p.e_pitch.value();
    let items: [(&FloatParam, &str, String); 10] = [
        (ks[0], "TIME", knob_text(M_ECHO, 0, ks[0].value(), v)),
        (ks[1], "FEEDBACK", knob_text(M_ECHO, 1, ks[1].value(), v)),
        (ks[2], "TONE", knob_text(M_ECHO, 2, ks[2].value(), v)),
        (ks[4], "WIDTH", knob_text(M_ECHO, 4, ks[4].value(), v)),
        (&p.e_duck, "DUCK", format!("{:.0}%", p.e_duck.value() * 100.0)),
        (&p.e_smooth, "SMOOTH", format!("{:.0}%", p.e_smooth.value() * 100.0)),
        (ks[6], "GRAINS", knob_text(M_ECHO, 6, ks[6].value(), v)),
        (&p.e_pitch, "PITCH", if pitch.abs() < 0.5 { "0 st".into() } else { format!("{:+.0} st", pitch) }),
        (ks[3], "WOBBLE", knob_text(M_ECHO, 3, ks[3].value(), v)),
        (ks[5], "ALL THRU", knob_text(M_ECHO, 5, ks[5].value(), v)),
    ];
    for (n, (prm, lbl, val)) in items.iter().enumerate() {
        knob(ui, setter, th, at(210.0 + n as f32 * 47.0, 40.0), 12.0, prm, col, lbl, val, ("e-k", n), dim);
    }
    // FLUX
    let fy = 104.0;
    txt(&pa, at(136.0, fy), Align2::LEFT_CENTER, "FLUX", labb(10.0), if dim { th.faint } else { th.dim }, 1.0);
    let tr = Rect::from_min_max(at(170.0, fy - 3.0), at(310.0, fy + 3.0));
    let fresp = ui.interact(tr.expand2(vec2(4.0, 7.0)), ui.scope_id().with("e-flux"), Sense::click_and_drag()).on_hover_text("FLUX: the repeats drift around by themselves");
    hslider(ui, setter, &fresp, &mp.flux, tr);
    let fv = mp.flux.value();
    pa.rect_filled(tr, rr(3.0), th.faint);
    if fv > 0.002 {
        pa.rect_filled(Rect::from_min_max(tr.min, pos2(tr.left() + fv * tr.width(), tr.bottom())), rr(3.0), al(col, if dim { 0.4 } else { 1.0 }));
    }
    pa.circle_filled(pos2(tr.left() + fv * tr.width(), tr.center().y), 5.0, th.knob_hi);
    pa.circle_stroke(pos2(tr.left() + fv * tr.width(), tr.center().y), 5.0, Stroke::new(1.0, th.edge));
    // CUT inside the loop
    txt(&pa, at(330.0, fy), Align2::LEFT_CENTER, "CUT", labb(10.0), if dim { th.faint } else { th.dim }, 1.0);
    let (lo, hi) = (mp.lo.value(), mp.hi.value());
    let full = lo <= 20.5 && hi >= 19900.0;
    let br = Rect::from_min_max(at(358.0, fy - 7.0), at(620.0, fy + 7.0));
    focus_strip(ui, setter, th, &pa, &mp.lo, &mp.hi, br, col, "e", dim, true);
    txt(&pa, at(630.0, fy), Align2::LEFT_CENTER, if full { "FULL".to_string() } else { format!("{}–{}", hzs(lo), hzs(hi)) }, mono(9.0), if full { th.dim } else { col }, 0.0);
    // VU
    let vr = Rect::from_min_max(at(r.width() - 22.0, 12.0), at(r.width() - 12.0, 84.0));
    pa.rect_filled(vr, rr(2.0), if th.light { th.faint } else { rgb(0x0c, 0x0d, 0x10) });
    let vdb = 20.0 * get(&sh.e_vu).max(1e-6).log10();
    let f = ((vdb + 48.0) / 48.0).clamp(0.0, 1.0);
    if f > 0.0 {
        vgrad(&pa, Rect::from_min_max(pos2(vr.left(), vr.bottom() - f * vr.height()), vr.max), if vdb > -6.0 { ACCENT } else { col }, al(col, 0.6));
    }
    txt(&pa, pos2(vr.center().x, vr.bottom() + 9.0), Align2::CENTER_CENTER, "VU", mono(8.5), th.dim, 0.0);
    let _ = st;
}

// ------------------------------------------------------------------ finishing: FOCUS / CUT, TONE, COMPRESS

fn finish(ui: &mut egui::Ui, setter: &ParamSetter, p: &PatinaParams, sh: &Shared, th: &Th, r: Rect) {
    let pa = ui.painter().clone();
    pa.rect_filled(r.translate(vec2(0.0, 3.0)), rr(8.0), Color32::from_black_alpha(if th.light { 28 } else { 110 }));
    pa.rect_filled(r, rr(8.0), th.panel);
    pa.rect_stroke(r, rr(8.0), Stroke::new(1.0, th.edge), StrokeKind::Inside);
    let at = |x: f32, y: f32| r.min + vec2(x, y);
    // global range
    let cut = p.g_cut.value();
    let cr = Rect::from_min_size(at(12.0, 10.0), vec2(58.0, 18.0));
    if chip(ui, &pa, th, cr, if cut { "CUT" } else { "FOCUS" }, true, ACCENT, "g-fc").on_hover_text("Whole plugin. FOCUS: only this range gets the effects, the rest passes clean · CUT: outside the range is removed").clicked() {
        set_bool(setter, &p.g_cut, !cut);
    }
    let (lo, hi) = (p.g_lo.value(), p.g_hi.value());
    let full = lo <= 20.5 && hi >= 19900.0;
    txt(&pa, at(212.0, 19.0), Align2::RIGHT_CENTER, if full { "FULL RANGE".to_string() } else { format!("{} – {} Hz", hzs(lo), hzs(hi)) }, mono(9.5), if full { th.dim } else { ACCENT }, 0.0);
    let br = Rect::from_min_max(at(12.0, 36.0), at(212.0, 52.0));
    focus_strip(ui, setter, th, &pa, &p.g_lo, &p.g_hi, br, ACCENT, "g", false, cut);
    // shapes (the CUT filters' slope)
    txt(&pa, at(12.0, 66.0), Align2::LEFT_CENTER, "SHAPE", labb(9.5), if cut { th.dim } else { th.faint }, 0.8);
    let sh_v = p.g_shape.value();
    for (n, s) in SHAPES.iter().enumerate() {
        let cr = Rect::from_min_size(at(12.0 + n as f32 * 40.5, 74.0), vec2(38.5, 18.0));
        let resp = chip(ui, &pa, th, cr, s, sh_v == n as i32, if cut { ACCENT } else { al(ACCENT, 0.45) }, ("shape", n)).on_hover_text("CUT filter shape: 6 / 12 / 24 / 48 dB per octave, or resonant");
        if resp.clicked() {
            set_int(setter, &p.g_shape, n as i32);
            if !cut {
                set_bool(setter, &p.g_cut, true);
            }
        }
    }
    txt(&pa, at(12.0, 110.0), Align2::LEFT_CENTER, if cut { "outside the range is removed" } else { "effects only inside the range" }, mono(8.5), th.dim, 0.0);
    // TONE
    let t = p.tone.value();
    let mode = p.tone_mode.value().clamp(0, 3) as usize;
    let tval = match mode {
        0 | 1 => {
            if t.abs() < 0.01 { "FLAT".to_string() } else { format!("{:+.1} dB", t * if mode == 0 { 6.0 } else { 9.0 }) }
        }
        2 => if t.abs() < 0.01 { "OFF".into() } else { format!("{} {:.0}%", if t < 0.0 { "SMALL" } else { "RADIO" }, t.abs() * 100.0) },
        _ => if t.abs() < 0.01 { "OFF".into() } else { format!("{} {:.0}%", if t < 0.0 { "RIBBON" } else { "COND." }, t.abs() * 100.0) },
    };
    knob(ui, setter, th, at(262.0, 46.0), 16.0, &p.tone, th.text, "TONE", &tval, "tone", false);
    let mr = Rect::from_min_size(at(232.0, 98.0), vec2(60.0, 18.0));
    if chip(ui, &pa, th, mr, TONE_MODES[mode], false, ACCENT, "tmode").on_hover_text("TONE voicing: TILT, MID, lo-fi SPEAKER, vintage MIC (click for the next)").clicked() {
        set_int(setter, &p.tone_mode, (mode as i32 + 1) % 4);
    }
    // COMPRESS
    let c = p.m_comp.value();
    let model = p.comp_model.value().clamp(0, 5) as usize;
    let gr = get(&sh.mcomp_gr);
    let cval = if c < 0.005 { "OFF".to_string() } else if gr > 0.1 { format!("-{:.1} dB", gr) } else { format!("{:.0}%", c * 100.0) };
    knob(ui, setter, th, at(326.0, 46.0), 16.0, &p.m_comp, ACCENT, "COMPRESS", &cval, "mcomp", false);
    let mr = Rect::from_min_size(at(298.0, 98.0), vec2(56.0, 18.0));
    if chip(ui, &pa, th, mr, crate::comp::MODELS[model], false, ACCENT, "cmodel").on_hover_text("Compressor model (click for the next): 670 vari-mu, DS, FAT, ALPHA, SH-A, SH-B").clicked() {
        set_int(setter, &p.comp_model, (model as i32 + 1) % 6);
    }
}

/// Horizontal slider behaviour (drag / click to set, double-click = default).
fn hslider(ui: &egui::Ui, setter: &ParamSetter, resp: &Response, prm: &FloatParam, tr: Rect) {
    if resp.double_clicked() {
        to_default(setter, prm);
        return;
    }
    if resp.drag_started() || resp.clicked() {
        setter.begin_set_parameter(prm);
    }
    if resp.dragged() || resp.clicked() {
        if let Some(q) = ui.ctx().pointer_interact_pos() {
            setter.set_parameter_normalized(prm, ((q.x - tr.left()) / tr.width()).clamp(0.0, 1.0));
        }
    }
    if resp.drag_stopped() || resp.clicked() {
        setter.end_set_parameter(prm);
    }
}

/// FOCUS: the frequency range the module works on (the rest passes clean).
#[allow(clippy::too_many_arguments)]
fn focus_strip(ui: &mut egui::Ui, setter: &ParamSetter, th: &Th, pa: &Painter, plo: &FloatParam, phi: &FloatParam, br: Rect, col: Color32, m: impl std::hash::Hash + Copy + std::fmt::Debug, dim: bool, cut: bool) {
    let mapx = |f: f32| br.left() + fu(f) * br.width();
    let (lo, hi) = (plo.value(), phi.value());
    pa.rect_filled(br, rr(4.0), if th.light { th.faint } else { rgb(0x11, 0x12, 0x16) });
    for f in [100.0, 1000.0, 10000.0] {
        let x = mapx(f);
        line(pa, pos2(x, br.top() + 3.0), pos2(x, br.bottom() - 3.0), th.edge, 1.0);
    }
    let (xl, xh) = (mapx(lo), mapx(hi));
    let sel = Rect::from_min_max(pos2(xl, br.top()), pos2(xh.max(xl + 2.0), br.bottom()));
    pa.rect_filled(sel, rr(3.0), al(col, if dim { 0.15 } else { 0.35 }));
    if cut {
        // CUT: hatch what gets removed
        for (a, b) in [(br.left(), xl), (xh, br.right())] {
            let mut x = a + 3.0;
            while x < b - 1.0 {
                line(pa, pos2(x, br.bottom() - 2.0), pos2((x + 6.0).min(b), br.top() + 2.0), al(col, if dim { 0.15 } else { 0.4 }), 1.0);
                x += 5.0;
            }
        }
    }
    pa.rect_stroke(sel, rr(3.0), Stroke::new(1.0, al(col, if dim { 0.3 } else { 0.9 })), StrokeKind::Inside);
    for (w, x, prm) in [(0, xl, plo), (1, xh, phi)] {
        let hr = Rect::from_center_size(pos2(x, br.center().y), vec2(12.0, br.height() + 8.0));
        let resp = ui.interact(hr, ui.scope_id().with(("foc", m, w)), Sense::click_and_drag()).on_hover_text("Drag: where this module works · double-click: full range");
        if resp.double_clicked() {
            to_default(setter, plo);
            to_default(setter, phi);
        } else {
            if resp.drag_started() {
                setter.begin_set_parameter(prm);
            }
            if resp.dragged() {
                if let Some(q) = ui.ctx().pointer_interact_pos() {
                    let u = ((q.x - br.left()) / br.width()).clamp(0.0, 1.0);
                    let f = 20.0 * 1000f32.powf(u);
                    let f = if w == 0 { f.min(hi * 0.8) } else { f.max(lo * 1.25) };
                    setter.set_parameter(prm, f.clamp(20.0, 20000.0));
                }
            }
            if resp.drag_stopped() {
                setter.end_set_parameter(prm);
            }
        }
        let hot = resp.hovered() || resp.dragged();
        pa.rect_filled(Rect::from_center_size(pos2(x, br.center().y), vec2(if hot { 5.0 } else { 3.0 }, br.height() + 4.0)), rr(1.5), if dim { th.dim } else { col });
        if hot {
            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
        }
    }
}

fn empty_column(ui: &mut egui::Ui, st: &mut St, th: &Th, i: usize, first: bool, o: egui::Vec2) {
    let r = slot_rect(o, i);
    let pa = ui.painter().clone();
    let resp = ui.interact(r, ui.scope_id().with(("empty", i)), Sense::click());
    let hot = resp.hovered();
    pa.rect_filled(r, rr(8.0), if hot { al(th.panel, 0.8) } else { al(th.panel, 0.35) });
    pa.rect_stroke(r, rr(8.0), Stroke::new(1.0, if hot { th.dim } else { th.edge }), StrokeKind::Inside);
    if first || hot {
        let c = r.center();
        pa.circle_stroke(c, 20.0, Stroke::new(1.5, if hot { ACCENT } else { th.dim }));
        line(&pa, c - vec2(8.0, 0.0), c + vec2(8.0, 0.0), if hot { ACCENT } else { th.dim }, 2.0);
        line(&pa, c - vec2(0.0, 8.0), c + vec2(0.0, 8.0), if hot { ACCENT } else { th.dim }, 2.0);
        txt(&pa, c + vec2(0.0, 36.0), Align2::CENTER_CENTER, "ADD MODULE", labb(11.0), if hot { th.text } else { th.dim }, 1.5);
    }
    if resp.clicked() {
        st.pick = Some(i);
        st.flav = None;
    }
}

fn drop_index(ui: &egui::Ui, dg: &Drag, o: egui::Vec2, n: usize) -> usize {
    let px = ui.ctx().pointer_interact_pos().map(|q| q.x).unwrap_or(0.0) - dg.grab - o.x;
    let i = ((px - COLX + (COLW + COLG) * 0.5) / (COLW + COLG)).floor();
    (i.max(0.0) as usize).min(n.saturating_sub(1))
}

/// While a column is being dragged: where it will land, and its header under the mouse.
fn drag_overlay(ui: &mut egui::Ui, _setter: &ParamSetter, st: &St, p: &PatinaParams, th: &Th, list: &[usize], o: egui::Vec2) {
    let Some(dg) = st.drag.as_ref() else { return };
    if !dg.moved || dg.from >= list.len() {
        return;
    }
    let m = list[dg.from];
    let to = drop_index(ui, dg, o, list.len());
    let pa = ui.painter().clone();
    let tr = slot_rect(o, to);
    pa.rect_stroke(tr.expand(3.0), rr(10.0), Stroke::new(2.0, MCOL[m]), StrokeKind::Outside);
    if let Some(q) = ui.ctx().pointer_interact_pos() {
        let hr = Rect::from_min_size(pos2(q.x - dg.grab, tr.top() - 4.0), vec2(COLW, 30.0));
        pa.rect_filled(hr.translate(vec2(0.0, 4.0)), rr(8.0), Color32::from_black_alpha(120));
        pa.rect_filled(hr, rr(8.0), MCOL[m]);
        txt(&pa, hr.center(), Align2::CENTER_CENTER, ui_name(m), labb(15.0), ON_COL, 1.6);
    }
    let _ = (p, th);
}

// ------------------------------------------------------------------ popups

#[allow(clippy::too_many_arguments)]
fn flavor_menu(ui: &mut egui::Ui, setter: &ParamSetter, st: &mut St, p: &PatinaParams, th: &Th, list: &[usize], m: usize, o: egui::Vec2) {
    let Some(i) = list.iter().position(|&x| x == m) else {
        st.flav = None;
        return;
    };
    let d = &MODS[m];
    let mp = p.m(m);
    let r = slot_rect(o, i);
    let w = COLW + 30.0;
    let x = (r.center().x - w / 2.0).clamp(o.x + 8.0, o.x + WIDTH as f32 - w - 8.0);
    let mr = Rect::from_min_size(pos2(x, r.top() + 62.0), vec2(w, d.vars.len() as f32 * 24.0 + 8.0));
    let bg = ui.interact(Rect::from_min_size(pos2(o.x, o.y + 44.0), vec2(WIDTH as f32, HEIGHT as f32 - 44.0)), ui.scope_id().with("flav-bg"), Sense::click());
    let pa = ui.painter().clone();
    pa.rect_filled(mr.translate(vec2(0.0, 4.0)), rr(8.0), Color32::from_black_alpha(140));
    pa.rect_filled(mr, rr(8.0), th.raised);
    pa.rect_stroke(mr, rr(8.0), Stroke::new(1.0, MCOL[m]), StrokeKind::Inside);
    let cur = mp.var.value();
    let mut picked = false;
    for (k, v) in d.vars.iter().enumerate() {
        let ir = Rect::from_min_size(mr.min + vec2(4.0, 4.0 + k as f32 * 24.0), vec2(w - 8.0, 22.0));
        let resp = ui.interact(ir, ui.scope_id().with(("flv", k)), Sense::click());
        let sel = cur == k as i32;
        if sel || resp.hovered() {
            pa.rect_filled(ir, rr(5.0), if sel { MCOL[m] } else { th.faint });
        }
        txt(&pa, ir.left_center() + vec2(10.0, 0.0), Align2::LEFT_CENTER, *v, labb(12.5), if sel { ON_COL } else { th.text }, 1.0);
        if resp.clicked() {
            set_int(setter, &mp.var, k as i32);
            picked = true;
        }
    }
    if picked || (bg.clicked() && !ui.ctx().pointer_interact_pos().map(|q| mr.contains(q)).unwrap_or(false)) {
        st.flav = None;
    }
}

#[allow(clippy::too_many_arguments)]
fn picker(ui: &mut egui::Ui, setter: &ParamSetter, st: &mut St, p: &PatinaParams, th: &Th, list: &[usize], slot: usize, o: egui::Vec2, t: f32) {
    let whole = Rect::from_min_size(pos2(o.x, o.y + 44.0), vec2(WIDTH as f32, HEIGHT as f32 - 44.0));
    let bg = ui.interact(whole, ui.scope_id().with("pick-bg"), Sense::click());
    let pa = ui.painter().clone();
    pa.rect_filled(whole, 0.0, al(th.bg, 0.86));
    let sheet = Rect::from_center_size(whole.center(), vec2(856.0, 500.0));
    pa.rect_filled(sheet.translate(vec2(0.0, 6.0)), rr(12.0), Color32::from_black_alpha(150));
    pa.rect_filled(sheet, rr(12.0), th.panel);
    pa.rect_stroke(sheet, rr(12.0), Stroke::new(1.0, th.edge), StrokeKind::Inside);
    let here = list.get(slot).copied();
    let title = match here {
        Some(m) => format!("SLOT {} · {}", slot + 1, ui_name(m)),
        None => "ADD A MODULE".to_string(),
    };
    txt(&pa, sheet.left_top() + vec2(24.0, 30.0), Align2::LEFT_CENTER, title, labb(20.0), th.text, 2.0);
    let mut close = false;
    if let Some(m) = here {
        for (k, s) in ["REMOVE", "RESET"].iter().enumerate() {
            let b = Rect::from_min_size(sheet.right_top() + vec2(-220.0 + k as f32 * 100.0, 18.0), vec2(92.0, 24.0));
            let resp = ui.interact(b, ui.scope_id().with(("pk-act", k)), Sense::click());
            pa.rect_filled(b, rr(6.0), if resp.hovered() { th.faint } else { th.raised });
            pa.rect_stroke(b, rr(6.0), Stroke::new(1.0, th.edge), StrokeKind::Inside);
            txt(&pa, b.center(), Align2::CENTER_CENTER, *s, labb(12.0), th.text, 1.5);
            if resp.clicked() {
                if k == 0 {
                    let mut l = list.to_vec();
                    l.remove(slot);
                    write_chain(setter, p, &l);
                } else {
                    reset_module(setter, p.m(m));
                    set_bool(setter, &p.m(m).on, true);
                }
                close = true;
            }
        }
    }
    let cw = (sheet.width() - 48.0 - 4.0 * 10.0) / 5.0;
    // ECHO has its own strip; LOSS is one card (MAGNETIC / CODEC inside)
    let cards = [M_NOISE, M_CRUSH, M_MAGNETIC, M_CHOP, M_WOBBLE, M_REVERB, M_CHORUS, M_TIME, M_REVERSE, M_HEAT, M_TAME, M_FILTER, M_WIDTH, M_COMP];
    let here = here.map(|h| if h == M_LOSS { M_MAGNETIC } else { h });
    let list_c: Vec<usize> = list.iter().map(|&x| if x == M_LOSS { M_MAGNETIC } else { x }).collect();
    let real: Vec<usize> = list.to_vec();
    for (ci, &m) in cards.iter().enumerate() {
        let (gx, gy) = (ci % 5, ci / 5);
        let list = &list_c;
        let cr = Rect::from_min_size(sheet.min + vec2(24.0 + gx as f32 * (cw + 10.0), 58.0 + gy as f32 * 142.0), vec2(cw, 130.0));
        let used_elsewhere = list.contains(&m) && here != Some(m);
        let resp = ui.interact(cr, ui.scope_id().with(("pk", m)), Sense::click()).on_hover_text(if used_elsewhere { "Already in the rack: picking it swaps the two" } else { "" });
        let col = MCOL[m];
        let hot = resp.hovered();
        pa.rect_filled(cr, rr(8.0), if hot { th.raised } else { th.bg2 });
        pa.rect_stroke(cr, rr(8.0), Stroke::new(if here == Some(m) { 2.0 } else { 1.0 }, if hot || here == Some(m) { col } else { th.edge }), StrokeKind::Inside);
        pa.rect_filled(Rect::from_min_size(cr.min, vec2(cw, 5.0)), CornerRadius { nw: 8, ne: 8, sw: 0, se: 0 }, col);
        glyph(&pa, m, cr.min + vec2(cw / 2.0, 40.0), 44.0, if used_elsewhere { al(col, 0.4) } else { col }, t);
        txt(&pa, cr.min + vec2(cw / 2.0, 76.0), Align2::CENTER_CENTER, ui_name(m), labb(15.0), if used_elsewhere { th.dim } else { th.text }, 1.5);
        let blurb = if m == M_MAGNETIC { "worn tape (wear, dropouts, flutter) or CODEC damage (lossy, dropouts, stretch, blur)" } else { MODS[m].blurb };
        let g = pa.layout(blurb.to_string(), mono(8.5), th.dim, cw - 18.0);
        pa.galley(cr.min + vec2(9.0, 88.0), g, th.dim);
        if used_elsewhere {
            txt(&pa, cr.right_top() + vec2(-8.0, 14.0), Align2::RIGHT_CENTER, "IN RACK", mono(8.5), col, 0.5);
        }
        if resp.clicked() {
            let mut l = real.clone();
            if let Some(j) = l.iter().position(|&x| x == m || (m == M_MAGNETIC && x == M_LOSS)) {
                // already in: swap places with this slot's module (or move it here)
                if slot < l.len() {
                    l.swap(j, slot);
                } else {
                    let x = l.remove(j);
                    l.push(x);
                }
            } else if slot < l.len() {
                l[slot] = m;
                set_bool(setter, &p.m(m).on, true);
            } else {
                l.push(m);
                set_bool(setter, &p.m(m).on, true);
            }
            write_chain(setter, p, &l);
            close = true;
        }
    }
    if close || (bg.clicked() && !ui.ctx().pointer_interact_pos().map(|q| sheet.contains(q)).unwrap_or(false)) {
        st.pick = None;
    }
}

// ------------------------------------------------------------------ footer

fn footer(ui: &mut egui::Ui, setter: &ParamSetter, st: &mut St, p: &PatinaParams, sh: &Shared, whole: Rect) {
    let r = Rect::from_min_max(pos2(whole.left(), whole.bottom() - 46.0), whole.right_bottom());
    let pa = ui.painter().clone();
    pa.rect_filled(r, 0.0, INK);
    line(&pa, r.left_top(), r.right_top(), Color32::from_black_alpha(160), 1.0);
    let cy = r.center().y;
    let soft_t = rgb(0xbd, 0xb3, 0xa3);
    txt(&pa, pos2(r.left() + 16.0, cy), Align2::LEFT_CENTER, "OVERSAMPLING", pm(9.5), soft_t, 1.5);
    for (i, s) in OS_NAMES.iter().enumerate() {
        let cr = Rect::from_min_size(pos2(r.left() + 120.0 + i as f32 * 40.0, cy - 10.0), vec2(36.0, 20.0));
        if st.skin.chip(ui, cr, s, p.os.value() == i as i32, ("os", i)).on_hover_text("Oversampling for DISTORT").clicked() {
            set_int(setter, &p.os, i as i32);
        }
    }
    let gr = -sh.meters.db(4);
    let safe = p.safe.value();
    let sr = Rect::from_min_size(pos2(r.left() + 300.0, cy - 10.0), vec2(108.0, 20.0));
    let lbl = if safe { if gr > 0.1 { format!("SAFE  -{:.1}", gr) } else { "SAFE 0 dB".to_string() } } else { "SAFE OFF".to_string() };
    if st.skin.chip(ui, sr, &lbl, safe, "safe").clicked() {
        set_bool(setter, &p.safe, !safe);
    }
    txt(&pa, pos2(r.left() + 520.0, cy), Align2::CENTER_CENTER, format!("{:.0} BPM · SYNC", get(&sh.bpm)), pm(10.0), soft_t, 2.0);
    let light = p.paper.load(Ordering::Relaxed);
    for (i, s) in ["DARK", "LIGHT"].iter().enumerate() {
        let cr = Rect::from_min_size(pos2(r.left() + 660.0 + i as f32 * 58.0, cy - 10.0), vec2(54.0, 20.0));
        if st.skin.chip(ui, cr, s, light == (i == 1), ("theme", i)).clicked() {
            p.paper.store(i == 1, Ordering::Relaxed);
        }
    }
    txt(&pa, pos2(r.right() - 16.0, cy), Align2::RIGHT_CENTER, format!("AWK STUDIOS · PATINA v{}", env!("CARGO_PKG_VERSION")), pm(9.5), rgb(0x8c, 0x84, 0x78), 1.0);
}

/// Little drawing on each card.
fn glyph(pa: &Painter, m: usize, c: Pos2, s: f32, col: Color32, t: f32) {
    let st = Stroke::new(2.2, col);
    let wave = |f: &dyn Fn(f32) -> f32| -> Vec<Pos2> { (0..=40).map(|i| i as f32 / 40.0).map(|u| pos2(c.x + (u - 0.5) * s, c.y - f(u) * s)).collect() };
    match m {
        M_NOISE => {
            for i in 0..14 {
                let x = c.x + (i as f32 / 13.0 - 0.5) * s;
                let h = ((i as f32 * 12.9 + (t * 8.0).floor()).sin() * 43758.5).fract() - 0.5;
                pa.line_segment([pos2(x, c.y), pos2(x, c.y + h * s * 0.55)], st);
            }
        }
        M_CRUSH => {
            let mut pts = Vec::new();
            for i in 0..=8 {
                let u = i as f32 / 8.0;
                let y = c.y - ((u * TAU).sin() * 2.0).round() / 2.0 * s * 0.35;
                let x0 = c.x + (u - 0.5) * s;
                if let Some(&last) = pts.last() {
                    let last: Pos2 = last;
                    pts.push(pos2(x0, last.y));
                }
                pts.push(pos2(x0, y));
            }
            pa.add(Shape::line(pts, st));
        }
        M_MAGNETIC => {
            for k in [-0.25f32, 0.25] {
                let cc = c + vec2(k * s, 0.0);
                pa.circle_stroke(cc, s * 0.2, st);
                let a = t * 2.0;
                pa.line_segment([cc + vec2(a.cos(), a.sin()) * s * 0.1, cc - vec2(a.cos(), a.sin()) * s * 0.1], st);
            }
            pa.line_segment([c + vec2(-0.25 * s, 0.2 * s), c + vec2(0.25 * s, 0.2 * s)], st);
        }
        M_CHOP => {
            for i in 0..4 {
                let on = (t * 4.0).floor() as i32 % 4 == i;
                let r = Rect::from_min_size(pos2(c.x + (i as f32 / 4.0 - 0.5) * s + 2.0, c.y - s * 0.2), vec2(s / 4.0 - 4.0, s * 0.4));
                pa.rect_filled(r, 0.0, if on { col } else { al(col, 0.35) });
            }
        }
        M_WOBBLE => {
            pa.add(Shape::line(wave(&|u| (u * TAU * 1.5 + t * 3.0).sin() * 0.25), st));
        }
        M_ECHO => {
            pa.add(Shape::line(wave(&|u| (u * TAU * 3.0 + t * 3.0).sin() * 0.28 * (-u * 2.6).exp()), st));
        }
        M_CHORUS => {
            pa.add(Shape::line(wave(&|u| (u * TAU * 1.5 + t * 2.0).sin() * 0.22), st));
            pa.add(Shape::line(wave(&|u| (u * TAU * 1.5 + t * 2.0 + 0.9).sin() * 0.22), Stroke::new(2.2, al(col, 0.5))));
        }
        M_TIME => {
            pa.add(Shape::line(wave(&|u| (u.powf(0.6) * TAU * 3.0 + t).sin() * 0.25), st));
        }
        M_REVERSE => {
            pa.add(Shape::line(wave(&|u| ((1.0 - u) * TAU * 2.0).sin() * 0.2 * u), st));
            let a = c + vec2(-0.42 * s, -0.33 * s);
            pa.line_segment([a, a + vec2(0.5 * s, 0.0)], st);
            pa.line_segment([a, a + vec2(0.1 * s, -0.08 * s)], st);
            pa.line_segment([a, a + vec2(0.1 * s, 0.08 * s)], st);
        }
        M_REVERB => {
            for i in 0..5 {
                let rr = s * (0.12 + i as f32 * 0.12);
                pa.add(Shape::line(arc_pts(c + vec2(-0.35 * s, 0.0), rr, -0.9, 0.9, 16), Stroke::new(2.2, al(col, 1.0 - i as f32 * 0.17))));
            }
        }
        M_HEAT => {
            pa.add(Shape::line(wave(&|u| ((u * 2.0 - 1.0) * 3.0).tanh() * 0.32), st));
        }
        M_TAME => {
            pa.add(Shape::line(wave(&|u| -0.3 * (-((u - 0.62) / 0.06).powi(2)).exp() + 0.08), st));
        }
        M_FILTER => {
            pa.add(Shape::line(wave(&|u| if u < 0.55 { 0.15 } else { 0.15 - (u - 0.55) * 1.1 }), st));
        }
        M_COMP => {
            // transfer curve with a knee
            let pts: Vec<Pos2> = (0..=24).map(|i| {
                let u = i as f32 / 24.0;
                let y = if u < 0.5 { u } else { 0.5 + (u - 0.5) * 0.25 };
                pos2(c.x - s * 0.45 + u * s * 0.9, c.y + s * 0.35 - y * s * 0.7)
            }).collect();
            pa.add(Shape::line(pts, st));
            let g = 0.5 + 0.5 * (t * 2.0).sin();
            pa.line_segment([pos2(c.x + s * 0.48, c.y - s * 0.3), pos2(c.x + s * 0.48, c.y - s * 0.3 + g * s * 0.4)], Stroke::new(3.0, col));
        }
        M_LOSS => {
            for i in 0..10 {
                let x = c.x + (i as f32 / 9.0 - 0.5) * s;
                let gone = ((i as f32 * 7.3 + (t * 3.0).floor()).sin() * 9731.7).fract().abs() < 0.3;
                let h = (0.12 + 0.25 * ((i as f32 * 1.7).sin() * 0.5 + 0.5)) * s;
                if !gone {
                    pa.line_segment([pos2(x, c.y + h * 0.5), pos2(x, c.y - h * 0.5)], st);
                }
            }
        }
        _ => {
            for sg in [-1.0f32, 1.0] {
                let tip = c + vec2(sg * 0.42 * s, 0.0);
                pa.line_segment([c, tip], st);
                pa.line_segment([tip, tip + vec2(-sg * 0.14 * s, -0.13 * s)], st);
                pa.line_segment([tip, tip + vec2(-sg * 0.14 * s, 0.13 * s)], st);
            }
        }
    }
}


#[allow(dead_code)]
const _UNUSED: (f32, f32) = (PI, TAU);

#[cfg(test)]
mod tests {
    use super::*;
    use nice_plug::context::gui::GuiContextInner;
    use nice_plug::plugin::PluginState;
    use egui::{epaint, Event, ImageData, Modifiers, PointerButton, RawInput, TextureId, TextureWrapMode, Vec2};
    use std::collections::HashMap;

    struct Mock;
    impl GuiContextInner for Mock {
        fn plugin_api(&self) -> PluginApi {
            PluginApi::Vst3
        }
        fn request_restart(&self) {}
        unsafe fn raw_begin_set_parameter(&self, _p: ParamPtr) {}
        unsafe fn raw_set_parameter_normalized(&self, _p: ParamPtr, _n: f32) {}
        unsafe fn raw_end_set_parameter(&self, _p: ParamPtr) {}
        fn get_state(&self) -> PluginState {
            PluginState { version: String::new(), params: Default::default(), fields: Default::default() }
        }
        fn set_state(&self, _s: PluginState) {}
    }

    /// Tiny software rasteriser for egui meshes, so the real window can be checked as a PNG.
    struct Tex {
        w: usize,
        h: usize,
        px: Vec<[f32; 4]>,
        repeat: bool,
    }
    fn apply_delta(texs: &mut HashMap<TextureId, Tex>, out: &egui::FullOutput) {
        for (id, d) in out.textures_delta.set.iter().flat_map(|(id, ds)| ds.iter().map(move |d| (id, d))) {
            let (w, h, px): (usize, usize, Vec<[f32; 4]>) = match &d.image {
                ImageData::Color(c) => (c.size[0], c.size[1], c.pixels.iter().map(|p| [p.r() as f32 / 255.0, p.g() as f32 / 255.0, p.b() as f32 / 255.0, p.a() as f32 / 255.0]).collect()),
            };
            let repeat = d.options.wrap_mode == TextureWrapMode::Repeat;
            if let Some(pos) = d.pos {
                let t = texs.get_mut(id).unwrap();
                for y in 0..h {
                    for x in 0..w {
                        t.px[(pos[1] + y) * t.w + pos[0] + x] = px[y * w + x];
                    }
                }
            } else {
                texs.insert(*id, Tex { w, h, px, repeat });
            }
        }
    }
    fn render(ctx: &egui::Context, out: egui::FullOutput, texs: &mut HashMap<TextureId, Tex>, path: &str) {
        apply_delta(texs, &out);
        let (w, h) = (WIDTH as usize, HEIGHT as usize);
        let mut img = vec![[0.0f32; 3]; w * h];
        let prims = ctx.tessellate(out.shapes, 1.0);
        for cp in prims {
            let epaint::Primitive::Mesh(mesh) = cp.primitive else { continue };
            let clip = cp.clip_rect;
            let tex = texs.get(&mesh.texture_id);
            for tri in mesh.indices.chunks(3) {
                let v = [mesh.vertices[tri[0] as usize], mesh.vertices[tri[1] as usize], mesh.vertices[tri[2] as usize]];
                let (x0, x1) = (v.iter().map(|q| q.pos.x).fold(f32::MAX, f32::min).max(clip.left()).max(0.0), v.iter().map(|q| q.pos.x).fold(f32::MIN, f32::max).min(clip.right()).min(w as f32));
                let (y0, y1) = (v.iter().map(|q| q.pos.y).fold(f32::MAX, f32::min).max(clip.top()).max(0.0), v.iter().map(|q| q.pos.y).fold(f32::MIN, f32::max).min(clip.bottom()).min(h as f32));
                let area = (v[1].pos.x - v[0].pos.x) * (v[2].pos.y - v[0].pos.y) - (v[2].pos.x - v[0].pos.x) * (v[1].pos.y - v[0].pos.y);
                if area.abs() < 1e-6 {
                    continue;
                }
                for py in (y0.floor() as usize)..(y1.ceil() as usize).min(h) {
                    for px in (x0.floor() as usize)..(x1.ceil() as usize).min(w) {
                        let (fx, fy) = (px as f32 + 0.5, py as f32 + 0.5);
                        let e = |a: &epaint::Vertex, b: &epaint::Vertex| (b.pos.x - a.pos.x) * (fy - a.pos.y) - (fx - a.pos.x) * (b.pos.y - a.pos.y);
                        let (w0, w1, w2) = (e(&v[1], &v[2]) / area, e(&v[2], &v[0]) / area, e(&v[0], &v[1]) / area);
                        if w0 < -1e-4 || w1 < -1e-4 || w2 < -1e-4 {
                            continue;
                        }
                        let mut c = [0.0f32; 4];
                        for i in 0..4 {
                            c[i] = (w0 * v[0].color.to_array()[i] as f32 + w1 * v[1].color.to_array()[i] as f32 + w2 * v[2].color.to_array()[i] as f32) / 255.0;
                        }
                        if let Some(t) = tex {
                            let mut u = w0 * v[0].uv.x + w1 * v[1].uv.x + w2 * v[2].uv.x;
                            let mut vv = w0 * v[0].uv.y + w1 * v[1].uv.y + w2 * v[2].uv.y;
                            if t.repeat {
                                u = u.rem_euclid(1.0);
                                vv = vv.rem_euclid(1.0);
                            }
                            let tx = ((u * t.w as f32) as usize).min(t.w - 1);
                            let ty = ((vv * t.h as f32) as usize).min(t.h - 1);
                            let s = t.px[ty * t.w + tx];
                            for i in 0..4 {
                                c[i] *= s[i];
                            }
                        }
                        // premultiplied (gamma-space) blend
                        let d = &mut img[py * w + px];
                        for i in 0..3 {
                            d[i] = c[i] + d[i] * (1.0 - c[3]);
                        }
                    }
                }
            }
        }
        let bytes: Vec<u8> = img.iter().flat_map(|p| p.iter().map(|v| (v.clamp(0.0, 1.0) * 255.0) as u8).collect::<Vec<_>>()).collect();
        std::fs::write(path, bytes).unwrap();
    }

    fn raw(i: usize, ev: Vec<Event>) -> RawInput {
        RawInput { screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(WIDTH as f32, HEIGHT as f32))), time: Some(i as f64 * 0.04), events: ev, ..Default::default() }
    }

    /// Draws, clicks all over (both themes, with and without the pickers open):
    /// nothing panics.
    #[test]
    fn draws_and_clicks() {
        let ctx = egui::Context::default();
        fonts(&ctx);
        let p = PatinaParams::default();
        let sh = Shared::default();
        let mock = Mock;
        let setter = ParamSetter::new(&mock);
        let mut st = new_state("PatinaTest");
        let mut i = 0;
        for light in [false, true] {
            p.paper.store(light, Ordering::Relaxed);
            for mode in 0..3 {
                for y in (50..800).step_by(37) {
                    for x in (10..1110).step_by(53) {
                        st.pick = if mode == 1 { Some(2) } else { None };
                        st.flav = if mode == 2 { Some(M_REVERB) } else { None };
                        let q = Pos2::new(x as f32, y as f32);
                        let _ = ctx.run_ui(raw(i, vec![Event::PointerMoved(q), Event::PointerButton { pos: q, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE }]), |c| frame(c, &setter, &mut st, &p, &sh));
                        let _ = ctx.run_ui(raw(i + 1, vec![Event::PointerButton { pos: q, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE }]), |c| frame(c, &setter, &mut st, &p, &sh));
                        i += 2;
                    }
                }
            }
        }
    }

    /// Renders the window to raw RGB files when SHOT_DIR is set:
    /// 0 dark, 1 light, 2 module picker, 3 flavor list, 4 a full rack with FLUX.
    #[test]
    fn screenshots() {
        let Ok(dir) = std::env::var("SHOT_DIR") else { return };
        let ctx = egui::Context::default();
        fonts(&ctx);
        let p = PatinaParams::default();
        // a fuller rack for shot 4 (values can't be set in tests: rebuild params)
        let mut q = PatinaParams::default();
        q.c4 = IntParam::new("Slot 4", (M_COMP + 1) as i32, IntRange::Linear { min: 0, max: N_MODS as i32 });
        q.c7 = IntParam::new("Slot 7", (M_LOSS + 1) as i32, IntRange::Linear { min: 0, max: N_MODS as i32 });
        q.e_on = BoolParam::new("e", true);
        q.wob.esend = FloatParam::new("s", 0.6, FloatRange::Linear { min: 0.0, max: 1.0 });
        q.heat.esend = FloatParam::new("s", 0.4, FloatRange::Linear { min: 0.0, max: 1.0 });
        q.heat.cut = BoolParam::new("c", true);
        q.g_lo = FloatParam::new("glo", 120.0, FloatRange::Linear { min: 20.0, max: 20000.0 });
        q.g_hi = FloatParam::new("ghi", 9000.0, FloatRange::Linear { min: 20.0, max: 20000.0 });
        q.tone = FloatParam::new("t", -0.4, FloatRange::Linear { min: -1.0, max: 1.0 });
        q.m_comp = FloatParam::new("c", 0.5, FloatRange::Linear { min: 0.0, max: 1.0 });
        q.c8 = IntParam::new("Slot 8", (M_CHOP + 1) as i32, IntRange::Linear { min: 0, max: N_MODS as i32 });
        q.wob.flux = FloatParam::new("f", 0.6, FloatRange::Linear { min: 0.0, max: 1.0 });
        q.heat.lo = FloatParam::new("lo", 150.0, FloatRange::Linear { min: 20.0, max: 20000.0 });
        q.heat.hi = FloatParam::new("hi", 6000.0, FloatRange::Linear { min: 20.0, max: 20000.0 });
        q.chop.on = BoolParam::new("on", false);
        let sh = Shared::default();
        let mock = Mock;
        let setter = ParamSetter::new(&mock);
        let mut st = new_state("PatinaShot");
        let mut texs = HashMap::new();
        for (n, (light, pick, flav, full)) in [(false, None, None, false), (true, None, None, false), (false, Some(6usize), None, false), (false, None, Some(M_REVERB), false), (false, None, None, true)].iter().enumerate() {
            let pp = if *full { &q } else { &p };
            pp.paper.store(*light, Ordering::Relaxed);
            st.pick = *pick;
            st.flav = *flav;
            let mut last = None;
            for f in 0..3 {
                last = Some(ctx.run_ui(raw(n * 10 + f, vec![]), |c| frame(c, &setter, &mut st, pp, &sh)));
                if f < 2 {
                    apply_delta(&mut texs, last.as_ref().unwrap());
                }
            }
            render(&ctx, last.unwrap(), &mut texs, &format!("{}/shot{}.rgb", dir, n));
        }
    }
}
