#!/usr/bin/env python3
"""PATINA: vinyl + CHOW tape A/B presets (2026-11-22). Writes into presets/."""
import json, pathlib
M = {"noise": 1, "crush": 2, "mag": 3, "chop": 4, "wob": 5, "echo": 6, "verb": 7, "chorus": 8, "time": 9, "rev": 10, "heat": 11, "tame": 12, "filt": 13, "width": 14, "loss": 15, "comp": 16}
def preset(slots, p):
    d = {f"c{i+1}": (M[s] if i < len(slots) else 0) for i, s in enumerate(slots + [None] * (8 - len(slots)))}
    d.update(p)
    return {"fields": {}, "params": {k: ({"i32": v} if isinstance(v, int) else {"f32": float(v)}) for k, v in sorted(d.items())}, "version": "0.4.0"}
P = {
 # warm, constant record bed: always-on surface + gentle crackle, CHOW reel glue, slow warp, mono lows
 "VINYL 1 - Warm Record": preset(["noise", "mag", "wob", "verb", "width"], {
    "verb_var": 10, "verb_k0": 0.35, "verb_k1": 0.4, "verb_mix": 0.25,
    "noise_var": 0, "noise_k0": 0.58, "noise_k1": 0.42, "noise_k2": 0.5, "noise_k3": 0.0, "noise_k4": 0.4, "noise_k5": 0.25, "noise_k6": 0.2,
    "mag_var": 4, "mag_k0": 0.35, "mag_k1": 0.0, "mag_k2": 0.5, "mag_k3": 0.15, "mag_k4": 0.5,
    "wob_k0": 0.22, "wob_k1": 0.25, "wob_k2": 0.08, "wob_k3": 0.25, "wob_k4": 0.15,
    "width_k0": 0.42, "width_k2": 0.35, "tone": -0.15}),
 # dustier, older pressing: VINYL B crackle up front, CHOW cassette wear, more warp, a touch of room
 "VINYL 2 - Dusty Crate": preset(["noise", "mag", "wob", "verb", "width"], {
    "noise_var": 4, "noise_k0": 0.66, "noise_k1": 0.68, "noise_k2": 0.42, "noise_k3": 0.0, "noise_k4": 0.5, "noise_k5": 0.45, "noise_k6": 0.35,
    "mag_var": 3, "mag_k0": 0.55, "mag_k1": 0.12, "mag_k2": 0.6, "mag_k3": 0.3, "mag_k4": 0.5,
    "wob_k0": 0.35, "wob_k1": 0.3, "wob_k2": 0.12, "wob_k3": 0.35, "wob_k4": 0.2,
    "verb_var": 10, "verb_k0": 0.45, "verb_k1": 0.5, "verb_mix": 0.3,
    "width_k0": 0.38, "width_k2": 0.4, "tone": -0.3}),
 # A/B pair: identical settings, classic engine vs CHOW engine
 "TAPE AB - Classic Cassette": preset(["mag"], {"mag_var": 0, "mag_k0": 0.4, "mag_k1": 0.0, "mag_k2": 0.6, "mag_k3": 0.3, "mag_k4": 0.5}),
 "TAPE AB - CHOW Cassette": preset(["mag"], {"mag_var": 3, "mag_k0": 0.4, "mag_k1": 0.0, "mag_k2": 0.6, "mag_k3": 0.3, "mag_k4": 0.5}),
 "TAPE AB - Classic Reel": preset(["mag"], {"mag_var": 1, "mag_k0": 0.2, "mag_k1": 0.0, "mag_k2": 0.7, "mag_k3": 0.15, "mag_k4": 0.5}),
 "TAPE AB - CHOW Reel": preset(["mag"], {"mag_var": 4, "mag_k0": 0.2, "mag_k1": 0.0, "mag_k2": 0.7, "mag_k3": 0.15, "mag_k4": 0.5}),
}
out = pathlib.Path(__file__).parent / "presets"
for n, d in P.items():
    (out / f"{n}.awkp").write_text(json.dumps(d, indent=2))
print(len(P), "written")
