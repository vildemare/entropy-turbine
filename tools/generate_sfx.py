"""Generate Entropy Turbine's small, original PCM sound set.

Run `python3 tools/generate_sfx.py` to rebuild the WAV files. Only the Python
standard library is used, and the fixed seed keeps results reproducible.
"""

from math import cos, exp, pi, sin
from pathlib import Path
from random import Random
from struct import pack
from wave import open as open_wave

RATE = 22_050
OUT = Path(__file__).resolve().parents[1] / "assets" / "audio"
RNG = Random(0xE17A)


def sweep(start_hz: float, end_hz: float, time: float, length: float) -> float:
    phase = 2 * pi * (start_hz * time + (end_hz - start_hz) * time * time / (2 * length))
    return sin(phase)


def player_shot(t: float, d: float, pitch: float = 1.0, texture: float = 1.0) -> float:
    attack = min(1.0, t / 0.003)
    body = sweep(560 * pitch, 165 * pitch, t, d) + 0.12 * sweep(1120 * pitch, 330 * pitch, t, d)
    hiss = RNG.uniform(-1, 1) * exp(-55 * t)
    return attack * (0.58 * body * exp(-18 * t) + 0.17 * texture * hiss)


def enemy_shot(t: float, d: float, pitch: float = 1.0, texture: float = 1.0) -> float:
    crack = RNG.uniform(-1, 1) * exp(-65 * t)
    body = sweep(205 * pitch, 82 * pitch, t, d) * exp(-22 * t)
    return 0.55 * texture * crack + 0.42 * body


def impact(t: float, d: float) -> float:
    click = RNG.uniform(-1, 1) * exp(-80 * t)
    ring = (sin(2 * pi * 740 * t) + 0.4 * sin(2 * pi * 1160 * t)) * exp(-34 * t)
    return 0.27 * click + 0.38 * ring


def enemy_hit(t: float, d: float) -> float:
    crunch = RNG.uniform(-1, 1) * exp(-42 * t)
    metal = sweep(470, 170, t, d) * exp(-25 * t)
    return 0.32 * crunch + 0.28 * metal


def player_hurt(t: float, d: float) -> float:
    noise = RNG.uniform(-1, 1) * exp(-18 * t)
    wobble = (sin(2 * pi * 125 * t) + 0.35 * sin(2 * pi * 183 * t)) * exp(-9 * t)
    return 0.22 * noise + 0.43 * wobble


def death(t: float, d: float) -> float:
    sputter = RNG.uniform(-1, 1) * exp(-5 * t) * (0.6 + 0.4 * sin(2 * pi * 19 * t))
    fall = sweep(310, 58, t, d) * exp(-5 * t)
    return 0.2 * sputter + 0.35 * fall


def menu_click(t: float, d: float) -> float:
    return 0.33 * (sin(2 * pi * 680 * t) + 0.4 * sin(2 * pi * 1020 * t)) * exp(-55 * t)


def ambience(t: float, d: float) -> float:
    # Every partial completes an integer number of cycles in eight seconds.
    # The start and end therefore join without a click when Bevy loops it.
    drone = 0.19 * sin(2 * pi * 48 * t) + 0.11 * sin(2 * pi * 77 * t)
    machinery = 0.05 * sin(2 * pi * 173 * t) * (0.75 + 0.25 * sin(2 * pi * 0.25 * t))
    wind = 0.08 * sin(2 * pi * 397 * t) * (0.5 + 0.5 * sin(2 * pi * 0.125 * t))
    return drone + machinery + wind


def write(name: str, duration: float, synthesis, loop: bool = False) -> None:
    samples = bytearray()
    count = round(duration * RATE)
    for index in range(count):
        time = index / RATE
        value = synthesis(time, duration)
        if not loop:
            value *= min(1.0, time / 0.004, (duration - time) / 0.012)
        samples.extend(pack("<h", round(max(-1.0, min(1.0, value)) * 32767)))
    with open_wave(str(OUT / name), "wb") as wav:
        wav.setnchannels(1)
        wav.setsampwidth(2)
        wav.setframerate(RATE)
        wav.writeframes(samples)


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    write("player_shot.wav", 0.18, player_shot)
    write("enemy_shot.wav", 0.16, enemy_shot)
    write("impact.wav", 0.15, impact)
    write("enemy_hit.wav", 0.20, enemy_hit)
    write("player_hurt.wav", 0.32, player_hurt)
    write("death.wav", 0.62, death)
    write("menu_click.wav", 0.11, menu_click)
    write("ambience.wav", 8.0, ambience, loop=True)
    write("player_shot_2.wav", 0.18, lambda t, d: player_shot(t, d, 0.82, 1.10))
    write("player_shot_3.wav", 0.18, lambda t, d: player_shot(t, d, 0.92, 0.85))
    write("enemy_shot_2.wav", 0.16, lambda t, d: enemy_shot(t, d, 0.82, 1.14))
    write("enemy_shot_3.wav", 0.16, lambda t, d: enemy_shot(t, d, 0.93, 0.90))


if __name__ == "__main__":
    main()
