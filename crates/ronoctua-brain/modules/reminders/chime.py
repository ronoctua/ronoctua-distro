#!/usr/bin/env python3
"""The reminders' chime: two notes of a small bell, written as a WAV file.

Made here, at build time, from arithmetic — nothing is downloaded and no sound
file is kept in the repository. The same input gives the same bytes, so the
store path does not move between builds.

A bell and not a beep: each note is a fundamental with the inharmonic partials
a struck bar has (2.76, 5.40 and 8.93 times the fundamental), each dying away
faster than the one below it, over a soft attack so the speaker does not
click. The second note is a fourth below the first — the "ding-dong" a person
turns towards without being startled by.

Usage: chime.py <out.wav>
"""

from __future__ import annotations

import math
import struct
import sys
import wave

RATE = 48_000
# (start in seconds, fundamental in Hz)
NOTES = ((0.0, 1318.51), (0.42, 987.77))
LENGTH = 1.6
PARTIALS = ((1.0, 1.0, 2.2), (2.76, 0.42, 4.5), (5.40, 0.20, 8.0), (8.93, 0.08, 12.0))
ATTACK = 0.004
LOUDNESS = 0.45


def sample(t: float) -> float:
    total = 0.0
    for start, f in NOTES:
        s = t - start
        if s < 0:
            continue
        attack = min(1.0, s / ATTACK)
        for ratio, weight, decay in PARTIALS:
            total += weight * attack * math.exp(-decay * s) * math.sin(2 * math.pi * f * ratio * s)
    return total


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__, file=sys.stderr)
        return 2
    frames = int(RATE * LENGTH)
    raw = [sample(n / RATE) for n in range(frames)]
    peak = max(abs(v) for v in raw) or 1.0
    # A FADE over the last tenth of a second: the tail ends at zero, not on a
    # step the speaker would click on.
    fade = int(RATE * 0.1)
    data = bytearray()
    for n, v in enumerate(raw):
        tail = min(1.0, (frames - n) / fade)
        data += struct.pack("<h", int(round(v / peak * LOUDNESS * tail * 32767)))
    with wave.open(sys.argv[1], "wb") as out:
        out.setnchannels(1)
        out.setsampwidth(2)
        out.setframerate(RATE)
        out.writeframes(bytes(data))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
