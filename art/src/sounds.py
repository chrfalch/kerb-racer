"""The recorded foley, taken from infinite-driver's sample banks.

    python3 art/src/sounds.py [path/to/infinite-driver/public/audio]

Writes the tyre, ground and body-hit sounds to art/sounds/*.wav (16-bit mono,
32 kHz), as they are; the bake turns them into assets/*.sound. The engines are
synthesized by art/src/engines.py.
"""
import os
import subprocess
import sys

import numpy as np
from scipy.io import wavfile

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.join(HERE, "..", "sounds")
SRC = sys.argv[1] if len(sys.argv) > 1 else os.path.expanduser("~/Developer/infinite-driver/public/audio")
SR = 32000


def decode(path):
    """A file as mono float at SR (ffmpeg does the decoding and resampling)."""
    args = ["ffmpeg", "-v", "error", "-i", path, "-ac", "1", "-ar", str(SR), "-f", "f32le", "-"]
    data = np.frombuffer(subprocess.run(args, check=True, capture_output=True).stdout, dtype=np.float32)
    return data.astype(np.float64)


def write(name, x):
    x = np.clip(x, -1.0, 1.0)
    wavfile.write(os.path.join(OUT, f"{name}.wav"), SR, (x * 32767).round().astype(np.int16))


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
    foley()
