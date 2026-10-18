#!/usr/bin/env python3
"""Grafik-Effekte für das 3,5-Zoll-Display (480x320): Feuer, Plasma, Sternenflug, Lavalampe.

Gerechnet wird in halber Auflösung (240x160) mit numpy und dann verdoppelt;
das Display schafft über SPI ohnehin nur etwa 10 Bilder pro Sekunde.

Aufruf zum Ausprobieren:  sudo python3 effekte.py [sekunden_je_effekt]
"""
import os
import sys
import time

import numpy as np

W, H = 480, 320
EW, EH = W // 2, H // 2          # Rechenauflösung
FB = "/dev/fb0"


def palette(punkte):
    """256-Farben-Verlauf aus (position 0..255, (r, g, b))-Stützpunkten."""
    pos = [p for p, _ in punkte]
    kanaele = [np.interp(np.arange(256), pos, [c[i] for _, c in punkte]) for i in range(3)]
    return np.stack(kanaele, axis=1).astype(np.uint8)


class Feuer:
    """Doom-Feuer: jede Zeile übernimmt die darunter, seitlich verweht und zufällig abgekühlt."""
    name = "Feuer"
    STUFEN = 48
    PAL = palette([(0, (0, 0, 0)), (40, (90, 0, 0)), (90, (200, 30, 0)), (140, (250, 110, 0)),
                   (190, (255, 190, 30)), (230, (255, 245, 140)), (255, (255, 255, 255))])
    ABKUEHLEN = 0.42   # Wahrscheinlichkeit je Zeile, eine Stufe kälter zu werden

    def __init__(self):
        self.hitze = np.zeros((EH, EW), dtype=np.int16)
        self.hitze[-1] = self.STUFEN
        self.rng = np.random.default_rng()
        self.spalten = np.arange(EW)

    def bild(self, t):
        h, rng = self.hitze, self.rng
        # leicht atmende Glut am Boden
        h[-1] = np.where(rng.random(EW) < 0.97, self.STUFEN, self.STUFEN - 8)
        for y in range(EH - 2, -1, -1):
            kaelter = (rng.random(EW) < self.ABKUEHLEN).astype(np.int16)
            ziel = np.clip(self.spalten - rng.integers(-1, 2, EW), 0, EW - 1)
            h[y, ziel] = np.maximum(h[y + 1] - kaelter, 0)
        index = (h * (255 / self.STUFEN)).astype(np.uint8)
        return self.PAL[index]


class Plasma:
    name = "Plasma"
    PAL = palette([(0, (10, 10, 60)), (50, (40, 30, 140)), (100, (130, 40, 170)),
                   (150, (230, 60, 140)), (200, (255, 150, 80)), (230, (80, 200, 220)),
                   (255, (10, 10, 60))])

    def __init__(self):
        y, x = np.mgrid[0:EH, 0:EW].astype(np.float32)
        self.x, self.y = x / 22.0, y / 22.0

    def bild(self, t):
        x, y = self.x, self.y
        v = (np.sin(x + t) + np.sin((y + t) / 2.0) + np.sin((x + y + t) / 2.0)
             + np.sin(np.sqrt((x - 5 + 3 * np.sin(t / 3)) ** 2 + (y - 3 + 2 * np.cos(t / 2)) ** 2 + 1) + t))
        # v liegt in -4..4; Farben laufen zusätzlich langsam durch
        index = ((v / 8.0 + 0.5) * 255 + t * 12) % 256
        return self.PAL[index.astype(np.uint8)]


class Sternenflug:
    name = "Sternenflug"
    ANZAHL = 500

    def __init__(self):
        self.rng = np.random.default_rng()
        self.x = self.rng.uniform(-1, 1, self.ANZAHL).astype(np.float32)
        self.y = self.rng.uniform(-1, 1, self.ANZAHL).astype(np.float32)
        self.z = self.rng.uniform(0.05, 1, self.ANZAHL).astype(np.float32)
        self.letzt = None

    def bild(self, t):
        dt = 0.1 if self.letzt is None else min(t - self.letzt, 0.3)
        self.letzt = t
        self.z -= dt * 0.35
        neu = self.z <= 0.02
        n = int(neu.sum())
        if n:
            self.x[neu] = self.rng.uniform(-1, 1, n)
            self.y[neu] = self.rng.uniform(-1, 1, n)
            self.z[neu] = 1.0
        sx = (self.x / self.z * EW / 2 + EW / 2).astype(int)
        sy = (self.y / self.z * EH / 2 + EH / 2).astype(int)
        sichtbar = (sx >= 0) & (sx < EW - 1) & (sy >= 0) & (sy < EH - 1)
        sx, sy, z = sx[sichtbar], sy[sichtbar], self.z[sichtbar]
        hell = np.clip((1.0 - z) * 300, 40, 255).astype(np.uint8)
        bild = np.zeros((EH, EW, 3), dtype=np.uint8)
        bild[..., 2] = 8  # ganz leichtes Nachtblau
        farbe = np.stack([hell, hell, np.minimum(hell.astype(np.uint16) + 30, 255).astype(np.uint8)], axis=1)
        bild[sy, sx] = farbe
        nah = z < 0.35  # nahe Sterne größer zeichnen
        bild[sy[nah] + 1, sx[nah]] = farbe[nah]
        bild[sy[nah], sx[nah] + 1] = farbe[nah]
        bild[sy[nah] + 1, sx[nah] + 1] = farbe[nah]
        return bild


class Lavalampe:
    name = "Lavalampe"
    PAL = palette([(0, (20, 4, 30)), (90, (60, 10, 70)), (120, (180, 30, 60)),
                   (150, (255, 90, 40)), (200, (255, 170, 60)), (255, (255, 235, 150))])

    def __init__(self):
        y, x = np.mgrid[0:EH, 0:EW].astype(np.float32)
        self.x, self.y = x, y
        rng = np.random.default_rng()
        self.phase = rng.uniform(0, 2 * np.pi, (7, 2))
        self.tempo = rng.uniform(0.15, 0.4, (7, 2))
        self.radius = rng.uniform(14, 26, 7)

    def bild(self, t):
        feld = np.zeros((EH, EW), dtype=np.float32)
        for i in range(len(self.radius)):
            bx = EW / 2 + EW * 0.38 * np.sin(t * self.tempo[i, 0] + self.phase[i, 0])
            by = EH / 2 + EH * 0.42 * np.sin(t * self.tempo[i, 1] + self.phase[i, 1])
            feld += self.radius[i] ** 2 / ((self.x - bx) ** 2 + (self.y - by) ** 2 + 1.0)
        return self.PAL[np.clip(feld * 120, 0, 255).astype(np.uint8)]


EFFEKTE = [Feuer, Plasma, Sternenflug, Lavalampe]


def nach_fb(bild_klein):
    """240x160 RGB -> 480x320 RGB565 (Pixel verdoppelt)."""
    a = bild_klein.astype(np.uint16)
    v = ((a[..., 0] >> 3) << 11) | ((a[..., 1] >> 2) << 5) | (a[..., 2] >> 3)
    v = v.repeat(2, axis=0).repeat(2, axis=1)
    return v.astype("<u2").tobytes()


def abspielen(effekt, sekunden, fd, ziel_fps=12):
    start = time.monotonic()
    bilder = 0
    while (jetzt := time.monotonic()) - start < sekunden:
        os.lseek(fd, 0, os.SEEK_SET)
        os.write(fd, nach_fb(effekt.bild(jetzt - start)))
        bilder += 1
        rest = 1 / ziel_fps - (time.monotonic() - jetzt)
        if rest > 0:
            time.sleep(rest)
    return bilder / (time.monotonic() - start)


def main():
    sekunden = float(sys.argv[1]) if len(sys.argv) > 1 else 15
    fd = os.open(FB, os.O_RDWR)
    try:
        for klasse in EFFEKTE:
            e = klasse()
            fps = abspielen(e, sekunden, fd)
            print(f"{e.name}: {fps:.1f} Bilder/s gerechnet", flush=True)
    finally:
        os.close(fd)


if __name__ == "__main__":
    main()
