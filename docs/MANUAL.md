# PATINA manual

## Signal flow

```
in ─► global FOCUS split ─► slot 1 … slot 8 ─► ECHO ─► (FOCUS rest back in) ─► global CUT ─► TONE ─► COMPRESS ─► LOUDNESS MATCH ─► OUTPUT ─► MIX ─► SAFE
```

## Working the rack

- **Empty column:** click to add a module.
- **Module header:** click to swap or remove it, drag to reorder, and use the LED to turn it on or off.
- **Flavor:** click to step through the module's variants.
- **MAGNITUDE** (per module): its mix.
- **FOCUS** (the range strip at the bottom): drag to set where the module works, double-click for the full range. Switch to **CUT** to remove everything outside the range.
- **FLUX:** random movement on the module's main knob.
- **ECHO SEND:** how much of the echo's repeats go through this module.

## Modules

| Module | Flavors | Knobs |
|---|---|---|
| NOISE | VINYL A, TAPE, HUM, ROOM, VINYL B, SHELLAC, DUST | LEVEL, DUST, TONE, FOLLOW, SNAP, POPS |
| DIGITAL | CLEAN, DITHER | BITS, RATE, JITTER, SMOOTH |
| LOSS: MAGNETIC | CASSETTE, REEL, VHS | WEAR, DROPOUTS, SATURATE, FLUTTER, WEAR<>FLUT, … |
| LOSS: CODEC | MP3, STREAM, VOIP | LOSSY, DROPOUTS, JOINT, STRETCH, BLUR, FORMANT |
| CHOP | GATE, STUTTER | RATE, DEPTH (REPEATS), SMOOTH, SWING, CHANCE |
| WOBBLE | FREE, SYNC | WOW, RATE, FLUTTER, DRIFT, STEREO |
| SPACE | PLATE, GOLD PLATE, ROOM, CHAMBER, HALL, CATHEDRAL, SPRING, AMBIENCE, VAULT, LO-FI | DECAY, SIZE (TENSION on SPRING), … |
| CHORUS | CHORUS, ENSEMBLE, FLANGER, VIBRATO | RATE, DEPTH, FEEDBACK, TONE, WIDTH |
| VARISPEED | 1/2, 2x, TAPE STOP, 1/4, 1/8 SPEED | CYCLE, SMOOTH, LENGTH, CHANCE |
| REVERSE | SLICE, SWELL | SIZE, SMOOTH, CHANCE |
| DISTORT | 12AX7, EL34, TAPE, IRON, FET, GERM., CLIP, CLASSIC PRE, PORTA | DRIVE, COLOR, TONE, … (oversampled) |
| TAME | (none) | AMOUNT, SHARP, SPEED (KILN's de-resonator) |
| FILTER | LP, HP, BP, PHONE | CUTOFF, RES, MOVE, RATE, ENV, DRIVE |
| WIDTH | (none) | WIDTH, HAAS, MONO BASS, PAN |
| COMPRESS | 670, DS, FAT, ALPHA, SH-A, SH-B | INPUT/THRESH, RATIO, ATTACK, RELEASE, COLOR, AUTO GAIN, SC HPF (oversampled) |

## ECHO strip

The ECHO sits after the rack. Flavors: TAPE, DIGITAL, PING-PONG. Knobs: TIME, FEEDBACK, TONE, WOBBLE, WIDTH, THRU RACK, plus PING-PONG, SCRUB, PITCH, DUCK and SMOOTH. The repeats run back through every module whose ECHO SEND is up.

## Master section

- **MAGNITUDE:** scales every module's mix at once.
- **INPUT / OUTPUT:** levels in and out.
- **LOUDNESS MATCH:** the rack comes out as loud as it went in.
- **Global FOCUS / CUT:** limits the whole plugin to one frequency range. The cut slope can be 6, 12, 24 or 48 dB/oct, or resonant.
- **TONE:** TILT, MID, lo-fi SPEAKER or vintage MIC voicing.
- **COMPRESS:** a master compressor using the VISE models.

## Latency

DISTORT, COMPRESS and the master COMPRESS are oversampled, and their latency is always reported (held constant). CODEC adds one frame while it's in the rack.
