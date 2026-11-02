"""The soundtrack, composed here and rendered offline.

    python3 art/src/music.py

Writes art/sounds/music-race.wav (the race: 150 BPM surf-rock with a chiptune
lead, A minor, 24 bars, seamless when looped), which the bake turns into
assets/music-race.sound (with the countdown, lap and flag stings beside it),
and assets/music/menu.m4a (the menu: a 112 BPM
electric-piano shuffle in C, 16 bars), which app.contract's `audio` plays.

Everything is synthesized from the notes below: no samples, no libraries past
numpy and scipy. Each piece is rendered twice end to end and the second pass is
kept, so reverb, echo and long notes ringing past the last bar land on the
first one: the loop has no seam.
"""
import os
import subprocess

import numpy as np
from scipy.io import wavfile
from scipy.signal import butter, fftconvolve, sosfilt

HERE = os.path.dirname(os.path.abspath(__file__))
TOP = os.path.join(HERE, "..", "..")
SR = 32000
rng = np.random.default_rng(7)


def hz(midi):
    return 440.0 * 2 ** ((midi - 69) / 12)


NOTE = {"C": 0, "C#": 1, "D": 2, "D#": 3, "E": 4, "F": 5, "F#": 6, "G": 7, "G#": 8, "A": 9, "A#": 10, "B": 11}


def m(name):
    """'A4' -> 69."""
    return 12 * (int(name[-1]) + 1) + NOTE[name[:-1]]


def lowpass(x, cutoff, order=2):
    cutoff = min(cutoff, SR * 0.45)
    return sosfilt(butter(order, cutoff, "low", fs=SR, output="sos"), x)


def highpass(x, cutoff, order=2):
    return sosfilt(butter(order, cutoff, "high", fs=SR, output="sos"), x)


def bandpass(x, lo, hi, order=2):
    return sosfilt(butter(order, [lo, hi], "band", fs=SR, output="sos"), x)


def env(n, attack, decay, sustain=0.0, release=0.02, length=None):
    """An ADSR over n samples; the note is held for `length` seconds (default: all of it)."""
    t = np.arange(n) / SR
    held = n / SR - release if length is None else length
    a = np.clip(t / max(attack, 1e-4), 0, 1)
    d = sustain + (1 - sustain) * np.exp(-np.maximum(t - attack, 0) / max(decay, 1e-4))
    r = np.clip(1 - (t - held) / max(release, 1e-4), 0, 1)
    return a * d * r


def saw(f, n, phase=0.0):
    # Band-limited enough at these pitches: a sum of harmonics under Nyquist.
    t = np.arange(n) / SR
    out = np.zeros(n)
    for k in range(1, int(SR * 0.45 / f) + 1):
        out += np.sin(2 * np.pi * k * f * t + phase * k) / k
    return out * 0.6


def pulse(f, n, width=0.25, vibrato=0.0, vib_hz=5.5, vib_delay=0.15):
    t = np.arange(n) / SR
    depth = vibrato * np.clip((t - vib_delay) / 0.2, 0, 1)
    inst = f * 2 ** (depth * np.sin(2 * np.pi * vib_hz * t) / 12)
    ph = np.cumsum(inst) / SR
    out = np.zeros(n)
    for k in range(1, int(SR * 0.45 / f) + 1):
        out += np.sin(np.pi * k * width) / k * np.cos(2 * np.pi * k * ph)
    return out * 0.9


def tri(f, n):
    t = np.arange(n) / SR
    return 2 / np.pi * np.arcsin(np.sin(2 * np.pi * f * t))


class Track:
    def __init__(self, bpm, bars):
        self.bpm = bpm
        self.beat = 60.0 / bpm
        self.bars = bars
        self.length = int(round(bars * 4 * self.beat * SR))
        # Two passes end to end and a tail; the second pass is the loop.
        n = self.length * 2 + SR * 4
        self.buses = {}
        self.n = n

    def bus(self, name):
        if name not in self.buses:
            self.buses[name] = np.zeros((2, self.n))
        return self.buses[name]

    def add(self, name, at_beats, x, pan=0.0, gain=1.0):
        """Add mono x at beat `at_beats` (of the first pass) to both passes."""
        b = self.bus(name)
        l, r = np.cos((pan + 1) * np.pi / 4) * np.sqrt(2), np.sin((pan + 1) * np.pi / 4) * np.sqrt(2)
        for p in range(2):
            i = int(round((at_beats * self.beat) * SR)) + p * self.length
            j = min(i + len(x), self.n)
            if i < 0 or i >= self.n:
                continue
            b[0, i:j] += x[: j - i] * l * gain
            b[1, i:j] += x[: j - i] * r * gain

    def mixdown(self, sends):
        out = np.zeros((2, self.n))
        for name, b in self.buses.items():
            out += b * sends.get(name, 1.0)
        return out

    def loop(self, mix):
        return mix[:, self.length : self.length * 2]


def reverb(x, seconds=1.2, damp=3500):
    n = int(seconds * SR)
    t = np.arange(n) / SR
    out = np.zeros_like(x)
    for c in range(2):
        ir = rng.standard_normal(n) * np.exp(-t * 6.9 / seconds)
        ir = lowpass(ir, damp)
        ir[: int(0.012 * SR)] = 0  # pre-delay
        out[c] = fftconvolve(x[c], ir)[: x.shape[1]] * 0.045
    return out


def echo(x, delay, feedback=0.35, taps=4):
    out = np.zeros_like(x)
    d = int(delay * SR)
    g = 1.0
    for k in range(1, taps + 1):
        g *= feedback
        # Ping-pong: odd taps on the right, even on the left.
        c = k % 2
        src = x[0] + x[1]
        out[c, d * k :] += src[: -d * k] * g * 0.5
    return out


def duck(track, x, depth=0.45, release=0.18):
    """Pump x against the kick: down on every beat, back up over `release`."""
    n = x.shape[1]
    t = (np.arange(n) / SR) % track.beat
    g = 1 - depth * np.exp(-t / release)
    return x * g


def master(x, drive=1.4):
    # Level first, so the soft clip only rounds the loudest peaks off.
    x = x / np.abs(x).max()
    x = np.tanh(x * drive) / np.tanh(drive)
    peak = np.abs(x).max()
    return x * (0.89 / peak)


# ---------------------------------------------------------------- instruments


def kick(gain=1.0):
    n = int(0.32 * SR)
    t = np.arange(n) / SR
    f = 45 + 110 * np.exp(-t / 0.035)
    body = np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-t / 0.16)
    click = highpass(rng.standard_normal(n), 2000) * np.exp(-t / 0.004) * 0.3
    return (body + click) * gain


def snare(gain=1.0):
    n = int(0.25 * SR)
    t = np.arange(n) / SR
    tone = np.sin(2 * np.pi * 190 * t) * np.exp(-t / 0.05) * 0.5
    noise = bandpass(rng.standard_normal(n), 1200, 9000) * np.exp(-t / 0.075)
    return (tone + noise) * 0.8 * gain


def rim(gain=1.0):
    n = int(0.08 * SR)
    t = np.arange(n) / SR
    return (np.sin(2 * np.pi * 820 * t) + 0.6 * bandpass(rng.standard_normal(n), 2000, 7000)) * np.exp(-t / 0.012) * 0.5 * gain


def hat(open_=False, gain=1.0):
    n = int((0.22 if open_ else 0.05) * SR)
    t = np.arange(n) / SR
    x = highpass(rng.standard_normal(n), 7500, 4)
    return x * np.exp(-t / (0.07 if open_ else 0.012)) * 0.35 * gain


def shaker(gain=1.0):
    n = int(0.06 * SR)
    t = np.arange(n) / SR
    x = bandpass(rng.standard_normal(n), 5000, 12000)
    return x * np.sin(np.pi * t / t[-1]) ** 2 * 0.22 * gain


def crash(gain=1.0):
    n = int(1.8 * SR)
    t = np.arange(n) / SR
    x = highpass(rng.standard_normal(n), 4000, 2) + 0.3 * bandpass(rng.standard_normal(n), 3000, 6000)
    return x * np.exp(-t / 0.55) * 0.3 * gain


def tom(midi, gain=1.0):
    n = int(0.3 * SR)
    t = np.arange(n) / SR
    f = hz(midi) * (1 + 0.5 * np.exp(-t / 0.03))
    return np.sin(2 * np.pi * np.cumsum(f) / SR) * np.exp(-t / 0.12) * 0.8 * gain


def bass(midi, seconds, gain=1.0):
    """A picked electric bass: a saw through a lowpass that closes after the pluck."""
    n = int((seconds + 0.05) * SR)
    f = hz(midi)
    x = saw(f, n) + 0.5 * np.sin(2 * np.pi * f * np.arange(n) / SR)
    bright = lowpass(x, 2200)
    dark = lowpass(x, 500)
    t = np.arange(n) / SR
    pluck = np.exp(-t / 0.06)
    y = dark + (bright - dark) * pluck
    return y * env(n, 0.003, 0.4, 0.55, 0.04, seconds) * gain


def stab(midis, seconds, gain=1.0, cutoff=3200):
    """A chord stab: detuned saws, bright then closing."""
    n = int((seconds + 0.06) * SR)
    x = np.zeros(n)
    for k, note in enumerate(midis):
        for det in (-0.08, 0.08):
            x += saw(hz(note + det), n, phase=k + det * 10)
    x = lowpass(x, cutoff) * 0.18
    return x * env(n, 0.004, 0.12, 0.25, 0.05, seconds) * gain


def lead(midi, seconds, gain=1.0):
    """The chiptune lead: a 25% pulse with delayed vibrato, a touch of saw."""
    n = int((seconds + 0.06) * SR)
    f = hz(midi)
    x = pulse(f, n, 0.25, vibrato=0.25 if seconds > 0.3 else 0.0) + 0.25 * saw(f * 1.003, n)
    x = lowpass(x, 6000)
    return x * env(n, 0.006, 0.25, 0.7, 0.05, seconds) * 0.32 * gain


def arp(midi, seconds, gain=1.0):
    n = int((seconds + 0.03) * SR)
    x = pulse(hz(midi), n, 0.5) * 0.6 + tri(hz(midi * 1.0 + 12), n) * 0.2
    return lowpass(x, 4500) * env(n, 0.002, 0.08, 0.0, 0.02, seconds) * 0.22 * gain


def epiano(midi, seconds, gain=1.0):
    """A Rhodes-ish tine: two-operator FM whose brightness falls away, with tremolo."""
    n = int((seconds + 0.5) * SR)
    t = np.arange(n) / SR
    f = hz(midi)
    index = 2.2 * np.exp(-t / 0.25) + 0.25
    x = np.sin(2 * np.pi * f * t + index * np.sin(2 * np.pi * f * t))
    x += 0.15 * np.sin(2 * np.pi * 7 * f * t) * np.exp(-t / 0.03)  # the tine's tick
    trem = 1 - 0.15 * (1 + np.sin(2 * np.pi * 4.5 * t)) / 2
    return x * trem * env(n, 0.003, 1.1, 0.25, 0.25, seconds) * 0.16 * gain


def bell(midi, seconds, gain=1.0):
    n = int((seconds + 0.8) * SR)
    t = np.arange(n) / SR
    f = hz(midi)
    x = np.sin(2 * np.pi * f * t) + 0.35 * np.sin(2 * np.pi * 2.0 * f * t) * np.exp(-t / 0.35)
    x += 0.2 * np.sin(2 * np.pi * 3.01 * f * t) * np.exp(-t / 0.12)
    return x * env(n, 0.002, 0.7, 0.0, 0.3, seconds + 0.6) * 0.22 * gain


def round_bass(midi, seconds, gain=1.0):
    n = int((seconds + 0.05) * SR)
    t = np.arange(n) / SR
    f = hz(midi)
    x = np.sin(2 * np.pi * f * t) + 0.3 * np.sin(4 * np.pi * f * t) * np.exp(-t / 0.08) + 0.15 * tri(f, n)
    return x * env(n, 0.004, 0.5, 0.6, 0.05, seconds) * 0.55 * gain


# ---------------------------------------------------------------- the race

CHORDS = {
    "Am": ["A3", "C4", "E4"], "F": ["F3", "A3", "C4"], "C": ["G3", "C4", "E4"],
    "G": ["G3", "B3", "D4"], "Em": ["G3", "B3", "E4"], "E": ["G#3", "B3", "E4"],
    "Dm": ["A3", "D4", "F4"],
}
ROOT = {"Am": "A2", "F": "F2", "C": "C3", "G": "G2", "Em": "E2", "E": "E2", "Dm": "D2"}

# 24 bars: a four-bar groove, the hook twice, the lift, the hook again with a turnaround.
RACE_BARS = (
    ["Am", "F", "C", "G"]
    + ["Am", "F", "C", "G"] * 2
    + ["F", "G", "Em", "Am", "F", "G", "E", "E"]
    + ["Am", "F", "C", "G", "Dm", "F", "G", "G"]
)[:24]

# The lead in eighths: (start, length, note) per bar; None is a bar of rest.
HOOK = [
    [(0, 1, "E5"), (1, 1, "G5"), (2, 2, "A5"), (4, 1, "G5"), (5, 1, "E5"), (6, 2, "D5")],
    [(0, 3, "C5"), (3, 1, "D5"), (4, 2, "E5"), (6, 2, "C5")],
    [(0, 1, "G5"), (1, 1, "G5"), (2, 1, "E5"), (3, 1, "G5"), (4, 2, "C6"), (6, 1, "B5"), (7, 1, "G5")],
    [(0, 2, "B5"), (2, 1, "A5"), (3, 1, "G5"), (4, 4, "D5")],
]
HOOK2 = [
    HOOK[0],
    [(0, 2, "C6"), (2, 2, "A5"), (4, 1, "F5"), (5, 1, "G5"), (6, 2, "A5")],
    [(0, 1, "G5"), (1, 1, "E5"), (2, 2, "C5"), (4, 1, "D5"), (5, 1, "E5"), (6, 2, "G5")],
    [(0, 2, "F5"), (2, 2, "D5"), (4, 2, "B4"), (6, 2, "D5")],
]
LIFT = [
    [(0, 4, "A5"), (4, 2, "C6"), (6, 2, "A5")],
    [(0, 4, "B5"), (4, 2, "D6"), (6, 2, "B5")],
    [(0, 2, "G5"), (2, 2, "E5"), (4, 2, "B5"), (6, 2, "G5")],
    [(0, 6, "A5"), (6, 1, "G5"), (7, 1, "A5")],
    [(0, 2, "C6"), (2, 2, "A5"), (4, 2, "F5"), (6, 2, "A5")],
    [(0, 2, "D6"), (2, 2, "B5"), (4, 2, "G5"), (6, 2, "B5")],
    [(0, 3, "G#5"), (3, 1, "A5"), (4, 2, "B5"), (6, 2, "D6")],
    [(0, 4, "E6"), (4, 1, "D6"), (5, 1, "C6"), (6, 1, "B5"), (7, 1, "G#5")],
]
TURN = [
    [(0, 2, "F5"), (2, 2, "A5"), (4, 2, "D6"), (6, 2, "A5")],
    [(0, 2, "C6"), (2, 2, "A5"), (4, 2, "F5"), (6, 2, "C6")],
    [(0, 7, "B5")],
    None,
]
RACE_LEAD = [None] * 4 + HOOK + HOOK2 + LIFT + HOOK + TURN


def race():
    tr = Track(150, len(RACE_BARS))
    e = 0.5  # an eighth, in beats
    for bar, chord in enumerate(RACE_BARS):
        b0 = bar * 4
        last = bar == len(RACE_BARS) - 1
        section_start = bar in (0, 4, 12, 20)
        # Drums: kick on 1, the "and" of 2 and on 3; snare on 2 and 4; eighth hats.
        if section_start:
            tr.add("drums", b0, crash(), pan=-0.3)
        for k in [0, 1.5, 2] + ([3.5] if bar % 2 else []):
            if not (last and k >= 2):
                tr.add("kick", b0 + k, kick())
        for k in (1, 3):
            if not (last and k == 3):
                tr.add("drums", b0 + k, snare(), pan=0.05)
        for k in range(8):
            if last and k >= 4:
                break
            tr.add("drums", b0 + k * e, hat(open_=(k % 2 == 1 and k in (3, 7)), gain=0.8 if k % 2 else 1.0), pan=0.35)
        if last:
            # The fill back to the top: sixteenth snares rising, then two toms.
            for k in range(6):
                tr.add("drums", b0 + 2 + k * 0.25, snare(0.45 + 0.1 * k), pan=0.05)
            tr.add("drums", b0 + 3.5, tom(m("A3")), pan=-0.4)
            tr.add("drums", b0 + 3.75, tom(m("E3")), pan=0.4)
        # Bass: driving eighths with an octave hop.
        root = m(ROOT[chord])
        pattern = [0, 0, 12, 0, 0, 0, 12, 7]
        for k, step in enumerate(pattern):
            if last and k >= 4:
                tr.add("bass", b0 + 2, bass(root + 7, e * tr.beat * 3.5))
                break
            tr.add("bass", b0 + k * e, bass(root + step, e * tr.beat * 0.85))
        # Stabs on the offbeats (the surf "skank"), wider in the lift.
        voicing = [m(n) for n in CHORDS[chord]]
        lift = 12 <= bar < 20
        for k in (1, 3, 5, 7):
            if last and k > 3:
                break
            tr.add("stabs", b0 + k * e, stab(voicing + ([voicing[0] + 12] if lift else []), e * tr.beat * 0.7), pan=-0.45 if k % 4 == 1 else 0.45)
        # A sixteenth arpeggio under the lift and the last hook.
        if lift or bar >= 20 and not last:
            notes = voicing + [voicing[0] + 12]
            for k in range(16):
                n = notes[[0, 1, 2, 3, 2, 1, 2, 3][k % 8]] + 12
                tr.add("arp", b0 + k * 0.25, arp(n, 0.25 * tr.beat * 0.8), pan=0.6 * np.sin(k * 0.7))
        # The lead.
        phrase = RACE_LEAD[bar]
        if phrase:
            for start, length, note in phrase:
                tr.add("lead", b0 + start * e, lead(m(note), length * e * tr.beat * 0.95), pan=-0.1)
                # Doubled an octave down, quietly, for weight.
                tr.add("lead", b0 + start * e, lead(m(note) - 12, length * e * tr.beat * 0.95, gain=0.35), pan=0.15)
    stabs = duck(tr, tr.bus("stabs"))
    arpb = duck(tr, tr.bus("arp"), depth=0.3)
    bass_ = duck(tr, tr.bus("bass"), depth=0.25, release=0.1)
    lead_ = tr.bus("lead")
    dry = tr.bus("kick") * 0.95 + tr.bus("drums") * 0.9 + bass_ * 0.85 + stabs * 0.7 + arpb * 0.6 + lead_ * 0.9
    wet = reverb(stabs * 0.6 + lead_ * 0.7 + tr.bus("drums") * 0.3 + arpb * 0.5, 1.1)
    wet += echo(lead_, tr.beat * 0.75, 0.35) * 0.5
    mix = highpass(dry + wet, 30)
    return master(tr.loop(mix))


# ---------------------------------------------------------------- the menu

MENU_BARS = ["Cmaj7", "Am7", "Fmaj7", "G7"] * 2 + ["Em7", "Am7", "Dm7", "G7", "Fmaj7", "Em7", "Dm7", "G7"]
MENU_CHORDS = {
    "Cmaj7": ["E4", "G4", "B4", "C5"], "Am7": ["E4", "G4", "A4", "C5"], "Fmaj7": ["E4", "F4", "A4", "C5"],
    "G7": ["D4", "F4", "G4", "B4"], "Em7": ["D4", "E4", "G4", "B4"], "Dm7": ["C4", "D4", "F4", "A4"],
}
MENU_ROOT = {"Cmaj7": "C2", "Am7": "A1", "Fmaj7": "F2", "G7": "G2", "Em7": "E2", "Dm7": "D2"}
# The bell tune, in eighths per bar.
MENU_TUNE = [
    None,
    None,
    [(0, 1, "E5"), (1, 1, "G5"), (2, 2, "A5"), (5, 1, "G5"), (6, 2, "E5")],
    [(0, 2, "D5"), (2, 1, "G4"), (3, 1, "B4"), (4, 4, "D5")],
    [(0, 1, "E5"), (1, 1, "G5"), (2, 2, "C6"), (5, 1, "B5"), (6, 2, "A5")],
    [(0, 2, "G5"), (2, 2, "E5"), (4, 1, "C5"), (5, 1, "D5"), (6, 2, "E5")],
    [(0, 2, "A5"), (2, 1, "G5"), (3, 1, "F5"), (4, 2, "E5"), (6, 2, "C5")],
    [(0, 3, "D5"), (4, 1, "B4"), (5, 1, "C5"), (6, 2, "D5")],
    [(0, 3, "B4"), (3, 1, "D5"), (4, 2, "G5"), (6, 2, "F#5")],
    [(0, 3, "E5"), (3, 1, "C5"), (4, 4, "A4")],
    [(0, 2, "F5"), (2, 1, "E5"), (3, 1, "D5"), (4, 2, "A4"), (6, 2, "C5")],
    [(0, 4, "B4"), (4, 1, "G4"), (5, 1, "A4"), (6, 2, "B4")],
    [(0, 2, "C5"), (2, 2, "E5"), (4, 2, "A5"), (6, 2, "G5")],
    [(0, 3, "G5"), (3, 1, "E5"), (4, 4, "B4")],
    [(0, 2, "A4"), (2, 2, "D5"), (4, 2, "F5"), (6, 2, "A5")],
    [(0, 1, "G5"), (1, 1, "F5"), (2, 1, "D5"), (3, 1, "B4"), (4, 2, "G4"), (6, 2, "D5")],
]


def menu():
    tr = Track(112, len(MENU_BARS))
    e = 0.5
    swing = 0.08  # the off-eighths lean late
    off = lambda k: k * e + (swing if k % 2 else 0)
    for bar, chord in enumerate(MENU_BARS):
        b0 = bar * 4
        last = bar == len(MENU_BARS) - 1
        for k in (0, 2.5) if bar % 2 == 0 else (0, 1.75, 2.5):
            tr.add("kick", b0 + k, kick(0.7))
        for k in (1, 3):
            tr.add("drums", b0 + k, rim(0.9) if not (last and k == 3) else snare(0.6), pan=-0.1)
        for k in range(8):
            tr.add("drums", b0 + off(k), shaker(1.0 if k % 2 else 0.6), pan=0.4)
        if bar % 4 == 0:
            tr.add("drums", b0, crash(0.35), pan=-0.4)
        # Electric piano: the chord on 1 and a push on the "and" of 2.
        notes = [m(n) for n in MENU_CHORDS[chord]]
        for at, length, g in ((0, 1.4, 1.0), (off(3), 1.2, 0.7), (off(7) if bar % 2 else 3, 0.5, 0.55)):
            for k, note in enumerate(notes):
                tr.add("keys", b0 + at + k * 0.008, epiano(note, length * tr.beat, g), pan=-0.3 + 0.2 * k)
        # A walking-ish bass.
        root = m(MENU_ROOT[chord])
        walk = [(0, 0, 1.5), (off(3), 7, 0.5), (2, 12, 1.0), (3, 10 if "7" in chord else 7, 0.5), (off(7), 7, 0.4)]
        for at, step, length in walk:
            tr.add("bass", b0 + at, round_bass(root + step, length * tr.beat))
        phrase = MENU_TUNE[bar]
        if phrase:
            for start, length, note in phrase:
                tr.add("bell", b0 + off(start), bell(m(note), length * e * tr.beat), pan=0.2)
    keys = tr.bus("keys")
    bell_ = tr.bus("bell")
    dry = tr.bus("kick") * 0.8 + tr.bus("drums") * 0.75 + tr.bus("bass") * 0.9 + keys * 0.8 + bell_ * 0.8
    wet = reverb(keys * 0.6 + bell_ * 0.9 + tr.bus("drums") * 0.2, 1.6, 3000)
    wet += echo(bell_, tr.beat * 0.75, 0.3) * 0.4
    mix = highpass(dry + wet, 30)
    return master(tr.loop(mix), 1.1)


# ---------------------------------------------------------------- stings


def sting(notes, seconds, tail=0.6):
    """Short cues in the lead's voice: (start s, length s, note, gain)."""
    n = int((seconds + tail) * SR)
    out = np.zeros(n)
    for start, length, note, g in notes:
        a, b = lead(m(note), length, g), bell(m(note), length, 0.6 * g)
        x = np.zeros(max(len(a), len(b)))
        x[: len(a)] += a
        x[: len(b)] += b
        i = int(start * SR)
        j = min(n, i + len(x))
        out[i:j] += x[: j - i]
    out /= np.abs(out).max()
    return out * 0.7


STINGS = {
    # The countdown: a beep for each light, and a higher, longer one for go.
    "count": [(0, 0.16, "A5", 1.0)],
    "go": [(0, 0.5, "A6", 1.0), (0, 0.5, "E6", 0.6), (0, 0.5, "A5", 0.5)],
    # A lap: a quick rising arpeggio.
    "lap": [(0, 0.07, "C6", 0.8), (0.07, 0.07, "E6", 0.9), (0.14, 0.3, "G6", 1.0)],
    # The flag: a little fanfare.
    "finish": [
        (0.0, 0.12, "G5", 0.8), (0.13, 0.12, "C6", 0.8), (0.26, 0.12, "E6", 0.9),
        (0.39, 0.3, "G6", 1.0), (0.72, 0.12, "E6", 0.8), (0.85, 1.0, "C7", 1.0),
        (0.85, 1.0, "G6", 0.6), (0.85, 1.0, "E6", 0.5),
    ],
}


def write_wav(path, x):
    wavfile.write(path, SR, (np.clip(x.T, -1, 1) * 32767).round().astype(np.int16))


if __name__ == "__main__":
    sounds = os.path.join(TOP, "art", "sounds")
    os.makedirs(sounds, exist_ok=True)
    write_wav(os.path.join(sounds, "music-race.wav"), race())
    print("art/sounds/music-race.wav")
    for name, notes in STINGS.items():
        seconds = max(start + length for start, length, _, _ in notes)
        x = sting(notes, seconds)
        write_wav(os.path.join(sounds, f"sting-{name}.wav"), x[None, :])
        print(f"art/sounds/sting-{name}.wav")
    music = os.path.join(TOP, "assets", "music")
    os.makedirs(music, exist_ok=True)
    tmp = os.path.join(music, "menu.wav")
    write_wav(tmp, menu())
    subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", tmp, "-c:a", "aac", "-b:a", "128k", "-ar", "44100", os.path.join(music, "menu.m4a")], check=True)
    os.remove(tmp)
    print("assets/music/menu.m4a")
