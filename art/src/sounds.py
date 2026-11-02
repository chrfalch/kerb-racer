"""The recorded sounds, taken from infinite-driver's sample banks.

    python3 art/src/sounds.py [path/to/infinite-driver/public/audio]

Writes art/sounds/*.wav (16-bit mono, 32 kHz), which the bake turns into
assets/*.sound, and prints ENGINE_RPMS for logic/src/sound.rs.

The engine is infinite-driver's recorded Petrol V8: loops of a real engine held
at steady rpm, on load (pulling) and off load (coasting), each seamless. Exact
plays a voice at one fixed rate, so instead of bending a loop's pitch as it
plays (infinite-driver's worklet does), we re-pitch the recordings here onto a
grid of rpms close enough together (7%) that crossfading two neighbours by the
engine's rpm sounds like one engine revving. Each loop is resampled whole with
an FFT, which treats it as periodic, so its seam stays seamless.

The foley (tyres, ground, body hits) is copied over as it is.
"""
import json
import os
import subprocess
import sys

import numpy as np
from scipy.io import wavfile
from scipy.signal import resample

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "sounds")
SRC = sys.argv[1] if len(sys.argv) > 1 else os.path.expanduser("~/Developer/infinite-driver/public/audio")
SR = 32000

# Idle, then a 7% grid from the lowest working rpm to the limiter.
IDLE = [800, 1000, 1250]
GRID = IDLE + [int(round(1500 * 1.07**k / 10) * 10) for k in range(0, 23) if 1500 * 1.07**k <= 6300] + [6600]


def decode(path, start=0.0, end=None):
    """A file as mono float at SR (ffmpeg does the decoding and resampling)."""
    args = ["ffmpeg", "-v", "error", "-i", path, "-ac", "1", "-ar", str(SR), "-f", "f32le", "-"]
    data = np.frombuffer(subprocess.run(args, check=True, capture_output=True).stdout, dtype=np.float32)
    a = int(round(start * SR))
    b = len(data) if end is None else int(round(end * SR))
    return data[a:b].astype(np.float64)


def write(name, x, gain=1.0):
    x = np.clip(x * gain, -1.0, 1.0)
    wavfile.write(os.path.join(OUT, f"{name}.wav"), SR, (x * 32767).round().astype(np.int16))


def engine():
    bank = os.path.join(SRC, "engine-v8")
    manifest = json.load(open(os.path.join(bank, "manifest.json")))
    for load in ["on", "off"]:
        loops = [l for l in manifest["loops"] if l["load"] == load]
        loops.sort(key=lambda l: l["rpm"])
        recorded = {l["rpm"]: decode(os.path.join(bank, l["file"]), l.get("loopStart", 0), l["loopEnd"]) for l in loops}
        peak = 0.0
        out = {}
        for rpm in GRID:
            # The nearest recording (by ratio), sped up or slowed down to this rpm.
            src = min(recorded, key=lambda r: abs(np.log(rpm / r)))
            x = recorded[src]
            n = int(round(len(x) * src / rpm))
            y = resample(x, n) if n != len(x) else x
            out[rpm] = y
            peak = max(peak, np.abs(y).max())
        for rpm, y in out.items():
            # One level for the whole set, so the loops keep their relative loudness.
            write(f"engine-{load}-{rpm}", y, manifest.get("gain", 1.0) * 0.6 / peak)


FOLEY = {
    # Loops.
    "squeal": "squeal-0", "slide": "slide-0", "roll-gravel": "roll-gravel-0",
    "roll-dirt": "roll-dirt-0", "scrape": "scrape-0",
    # Car-to-car and fence hits, light to heavy.
    "hit-0": "chassis-hit-1", "hit-1": "chassis-hit-5", "hit-2": "chassis-hit-9",
    "hit-3": "chassis-hit-11", "hit-4": "chassis-hit-0", "hit-5": "chassis-hit-6",
    # Kerb blocks under a wheel: the light bump-stop knocks.
    "kerb-0": "bump-4", "kerb-1": "bump-5", "kerb-2": "bump-6", "kerb-3": "bump-7",
}


def foley():
    for name, take in FOLEY.items():
        write(name, decode(os.path.join(SRC, "foley", f"{take}.flac")))


if __name__ == "__main__":
    os.makedirs(OUT, exist_ok=True)
    engine()
    foley()
    print(f"pub const ENGINE_RPMS: [u32; {len(GRID)}] = {GRID};")
