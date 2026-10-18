#!/usr/bin/env python3
"""Pixel-Welt: Info-Anzeige als kleines Jump-'n'-Run auf dem 3,5-Zoll-Display.

Grafiken: "Pixel Adventure 1" von Pixel Frog (CC0), https://pixelfrog-assets.itch.io/pixel-adventure-1

Was die Szene zeigt:
  - Uhrzeit groß in der Pixel-Schrift, darunter Datum, links Temperatur
  - Tag/Nacht und Regen/Schnee nach dem echten Wetter (Open-Meteo)
  - jede von Pi-hole geblockte Anfrage wird zu einer Frucht, die Figur sammelt sie ein;
    oben rechts steht, wie viele heute geblockt wurden
  - Internet weg: Stacheln, die Figur fällt hin, rote Zeile "OFFLINE SEIT ..."
  - jede Stunde eine andere Figur; Antippen des Displays lässt sie springen

Gerechnet wird in 240x160 und verdoppelt. Ans Display geht nur der Zeilenbereich,
der sich geändert hat - das SPI-Display schafft für Vollbilder nur etwa 10 Bilder/s.
"""
import os
import random
import struct
import time
from datetime import datetime

import numpy as np
from PIL import Image, ImageDraw

import anzeige as daten  # Wetter, Pi-hole, Online-Prüfung, Speedtest-Datei

ASSETS = "/opt/pi-anzeige/assets/Free"
FB = "/dev/fb0"
LW, LH = 240, 160            # logische Auflösung, auf dem Display verdoppelt
BODEN = 128                  # Oberkante des Grasbodens (logisch)
FPS = 15

FIGUREN = ["Ninja Frog", "Mask Dude", "Pink Man", "Virtual Guy"]
FRUECHTE = ["Apple", "Bananas", "Cherries", "Kiwi", "Melon", "Orange", "Pineapple", "Strawberry"]
WOCHENTAG = ["MO", "DI", "MI", "DO", "FR", "SA", "SO"]
MONAT = ["JAN", "FEB", "MAR", "APR", "MAI", "JUN", "JUL", "AUG", "SEP", "OKT", "NOV", "DEZ"]
WETTER_WORT = {0: "KLAR", 1: "HEITER", 2: "WOLKIG", 3: "BEDECKT", 45: "NEBEL", 48: "NEBEL",
               51: "NIESEL", 53: "NIESEL", 55: "NIESEL", 56: "NIESEL", 57: "NIESEL",
               61: "REGEN", 63: "REGEN", 65: "STARKREGEN", 66: "EISREGEN", 67: "EISREGEN",
               71: "SCHNEE", 73: "SCHNEE", 75: "SCHNEE", 77: "SCHNEE",
               80: "SCHAUER", 81: "SCHAUER", 82: "SCHAUER", 85: "SCHNEE", 86: "SCHNEE",
               95: "GEWITTER", 96: "GEWITTER", 99: "GEWITTER"}
REGEN_CODES = set(range(51, 68)) | {80, 81, 82, 95, 96, 99}
SCHNEE_CODES = {71, 73, 75, 77, 85, 86}

WEISS = (255, 255, 255, 255)
SCHATTEN = (20, 16, 32, 255)
ROT = (255, 90, 90, 255)
GRUEN = (120, 230, 120, 255)


def lade(pfad):
    return Image.open(os.path.join(ASSETS, pfad)).convert("RGBA")


def streifen(pfad):
    """Waagerechtes Sprite-Blatt in quadratische Einzelbilder schneiden."""
    bild = lade(pfad)
    h = bild.height
    return [bild.crop((i * h, 0, (i + 1) * h, h)) for i in range(bild.width // h)]


def gespiegelt(bilder):
    return [b.transpose(Image.Transpose.FLIP_LEFT_RIGHT) for b in bilder]


class Schrift:
    """Pixel-Schrift aus dem Paket: 8x10 je Zeichen, A-Z, 0-9 und . , : ? ! ( ) + -"""
    REIHEN = ["ABCDEFGHIJ", "KLMNOPQRST", "UVWXYZ", "0123456789", ".,:?!()+-"]

    def __init__(self):
        blatt = lade("Menu/Text/Text (White) (8x10).png")
        self.zeichen = {}
        for r, reihe in enumerate(self.REIHEN):
            for c, z in enumerate(reihe):
                self.zeichen[z] = blatt.crop((c * 8, r * 10, c * 8 + 8, r * 10 + 10)).getchannel("A")
        self.zeichen["/"] = self._schraeg()
        # Satzzeichen nur so breit wie ihre Pixel; Buchstaben und Ziffern bleiben gleich breit,
        # damit die Uhr beim Ziffernwechsel nicht hin- und herspringt.
        # je 1 Pixel Luft links und rechts neben dem Satzzeichen (plus die übliche 1 Pixel Lücke)
        self.links = {z: self.zeichen[z].getbbox()[0] - 1 for z in ".,:!"}
        self.vorschub = {z: self.zeichen[z].getbbox()[2] - self.links[z] + 2 for z in ".,:!"}
        self.vorschub.update({" ": 5, "°": 4})

    @staticmethod
    def _schraeg():
        maske = Image.new("L", (8, 10), 0)
        d = ImageDraw.Draw(maske)
        d.line((6, 1, 1, 8), fill=255, width=2)
        return maske

    def breite(self, text, skala=1):
        return sum(self.vorschub.get(z, 9) for z in text.upper()) * skala - skala

    def schreiben(self, bild, xy, text, farbe=WEISS, skala=1, schatten=True):
        x, y = xy
        text = text.upper().replace("Ä", "AE").replace("Ö", "OE").replace("Ü", "UE")
        for z in text:
            maske = self.zeichen.get(z)
            if maske is not None:
                if skala != 1:
                    maske = maske.resize((8 * skala, 10 * skala), Image.Resampling.NEAREST)
                gx = x - self.links.get(z, 0) * skala
                if schatten:
                    bild.paste(SCHATTEN, (gx + skala, y + skala), maske)
                bild.paste(farbe, (gx, y), maske)
            elif z == "°":
                d = ImageDraw.Draw(bild)
                d.rectangle((x, y, x + 2 * skala, y + 2 * skala), outline=farbe, width=1)
            x += self.vorschub.get(z, 9) * skala
        return x


class Figur:
    def __init__(self, name):
        pfad = f"Main Characters/{name}/"
        self.anim = {}
        for zustand, datei in (("rennen", "Run"), ("stehen", "Idle"), ("springen", "Jump"),
                               ("fallen", "Fall"), ("treffer", "Hit")):
            rechts = streifen(pfad + f"{datei} (32x32).png")
            self.anim[zustand] = (rechts, gespiegelt(rechts))
        self.x = random.uniform(20, LW - 52)
        self.y = BODEN - 32
        self.vy = 0.0
        self.richtung = 1
        self.zustand = "stehen"
        self.pause_bis = 0.0
        self.naechste_pause = time.monotonic() + random.uniform(5, 10)

    @property
    def am_boden(self):
        return self.y >= BODEN - 32 - 0.5

    def springen(self):
        if self.am_boden:
            self.vy = -175.0

    def schritt(self, dt, jetzt, ziel_x, tempo, offline):
        if offline:
            self.zustand = "treffer"
            return
        # Schwerkraft und Sprung
        if not self.am_boden or self.vy < 0:
            self.vy += 420.0 * dt
            self.y = min(self.y + self.vy * dt, BODEN - 32)
            if self.y >= BODEN - 32:
                self.vy = 0.0
        if jetzt < self.pause_bis and ziel_x is None:
            self.zustand = "stehen"
        else:
            if ziel_x is None:
                if self.x <= 6:
                    self.richtung = 1
                elif self.x >= LW - 38:
                    self.richtung = -1
                if jetzt > self.naechste_pause:
                    self.pause_bis = jetzt + random.uniform(1.5, 4)
                    self.naechste_pause = self.pause_bis + random.uniform(6, 14)
                    if random.random() < 0.5:
                        self.richtung *= -1
            else:
                abstand = ziel_x - (self.x + 16)
                if abs(abstand) > 2:
                    self.richtung = 1 if abstand > 0 else -1
            self.x = min(max(self.x + self.richtung * tempo * dt, 4), LW - 36)
            self.zustand = "rennen"
        if not self.am_boden:
            self.zustand = "springen" if self.vy < 0 else "fallen"

    def bild(self, t):
        rechts, links = self.anim[self.zustand]
        bilder = rechts if self.richtung > 0 else links
        return bilder[int(t * 20) % len(bilder)]

    def mitte(self):
        return self.x + 16, self.y + 20


class Frucht:
    def __init__(self, bilder, x, y):
        self.bilder, self.x, self.y = bilder, x, y
        self.versatz = random.random() * 5

    def bild(self, t):
        return self.bilder[int((t + self.versatz) * 20) % len(self.bilder)]


class Beruehrung:
    """Liest den Touchscreen (ADS7846) ohne zu blockieren; meldet jedes Antippen."""
    FORMAT = "llHHi"
    GROESSE = struct.calcsize(FORMAT)

    def __init__(self):
        self.fd = None
        try:
            with open("/proc/bus/input/devices") as f:
                bloecke = f.read().split("\n\n")
            for block in bloecke:
                if "ADS7846" in block:
                    for teil in block.split():
                        if teil.startswith("event"):
                            self.fd = os.open(f"/dev/input/{teil}", os.O_RDONLY | os.O_NONBLOCK)
        except OSError:
            self.fd = None

    def angetippt(self):
        if self.fd is None:
            return False
        getippt = False
        try:
            while True:
                daten_ = os.read(self.fd, self.GROESSE * 16)
                if not daten_:
                    break
                for i in range(0, len(daten_) - self.GROESSE + 1, self.GROESSE):
                    _, _, typ, code, wert = struct.unpack_from(self.FORMAT, daten_, i)
                    if typ == 1 and code == 0x14A and wert == 1:  # EV_KEY, BTN_TOUCH gedrückt
                        getippt = True
        except BlockingIOError:
            pass
        except OSError:
            self.fd = None
        return getippt


class Welt:
    def __init__(self):
        self.schrift = Schrift()
        self.fruchtbilder = {f: streifen(f"Items/Fruits/{f}.png") for f in FRUECHTE}
        self.eingesammelt = streifen("Items/Fruits/Collected.png")
        self.stachel = lade("Traps/Spikes/Idle.png")
        self.erdbeere = self.fruchtbilder["Strawberry"][0]
        gelaende = lade("Terrain/Terrain (16x16).png")
        self.gras = gelaende.crop((112, 0, 128, 16))
        self.erde = gelaende.crop((112, 16, 128, 32))
        self.muster_tag = lade("Background/Blue.png")
        self.muster_nacht = lade("Background/Purple.png")
        self.sterne = [(random.randrange(LW), random.randrange(BODEN - 40)) for _ in range(40)]
        self.figur_index = datetime.now().hour % len(FIGUREN)
        self.figur = Figur(FIGUREN[self.figur_index])
        self.fruechte = []
        self.effekte = []         # (x, y, startzeit) der Einsammel-Wolken
        self.tropfen = [[random.uniform(0, LW), random.uniform(0, BODEN)] for _ in range(45)]
        self.statisch = None
        self.statisch_schluessel = None
        self.bonus_naechst = time.monotonic() + 40

    # --- unveränderlicher Teil des Bildes -------------------------------------------
    def hintergrund(self, jetzt, wetter, geblockt, netz):
        nacht = bool(wetter) and not wetter["current"].get("is_day", 1)
        bild = Image.new("RGBA", (LW, LH))
        muster = self.muster_nacht if nacht else self.muster_tag
        for y in range(0, BODEN, 64):
            for x in range(0, LW, 64):
                bild.paste(muster, (x, y))
        if nacht:
            dunkel = Image.new("RGBA", (LW, BODEN), (10, 8, 40, 150))
            bild.alpha_composite(dunkel, (0, 0))
            d = ImageDraw.Draw(bild)
            for sx, sy in self.sterne:
                d.point((sx, sy), fill=(255, 255, 220, 255))
        for x in range(0, LW, 16):
            bild.paste(self.gras, (x, BODEN))
            bild.paste(self.erde, (x, BODEN + 16))
        if nacht:
            bild.alpha_composite(Image.new("RGBA", (LW, LH - BODEN), (10, 8, 40, 90)), (0, BODEN))

        s = self.schrift
        uhr = jetzt.strftime("%H:%M")
        s.schreiben(bild, ((LW - s.breite(uhr, 4)) // 2, 4), uhr, skala=4)
        datum = f"{WOCHENTAG[jetzt.weekday()]} {jetzt.day} {MONAT[jetzt.month - 1]}"
        s.schreiben(bild, ((LW - s.breite(datum)) // 2, 50), datum)
        if wetter:
            temp = f"{round(wetter['current']['temperature_2m'])}°"
            s.schreiben(bild, (6, 50), temp)
        # Pi-hole-Zähler oben rechts: Erdbeere + Zahl
        zahl = "-" if geblockt is None else str(geblockt)
        x_zahl = LW - 6 - s.breite(zahl)
        bild.alpha_composite(self.erdbeere, (x_zahl - 26, 39))
        s.schreiben(bild, (x_zahl, 50), zahl)

        # zwei Zeilen im Boden: Wetter und Internet
        if wetter:
            c, t = wetter["current"], wetter["daily"]
            zeile1 = (f"{WETTER_WORT.get(c['weather_code'], 'WETTER')} "
                      f"{round(t['temperature_2m_min'][0])}-{round(t['temperature_2m_max'][0])}°")
        else:
            zeile1 = "WETTER ?"
        s.schreiben(bild, (6, BODEN + 4), zeile1)
        ms, offline_seit, speed = netz if netz else (None, None, None)
        if ms is not None:
            zeile2 = f"ONLINE {ms:.0f}MS"
            if speed and "down_mbit" in speed:
                zeile2 += f" {speed['down_mbit']:.0f}/{speed['up_mbit']:.0f} MBIT"
            s.schreiben(bild, (6, BODEN + 18), zeile2, farbe=GRUEN)
        elif offline_seit:
            s.schreiben(bild, (6, BODEN + 18), f"OFFLINE SEIT {offline_seit:%H:%M}", farbe=ROT)
            for x in range(70, 190, 16):
                bild.alpha_composite(self.stachel, (x, BODEN - 16))
        else:
            s.schreiben(bild, (6, BODEN + 18), "PRUEFE INTERNET")
        return bild

    # --- Spiellogik ---------------------------------------------------------------
    def fruechte_dazu(self, anzahl):
        for _ in range(min(anzahl, 6 - len(self.fruechte))):
            art = random.choice(FRUECHTE)
            hoch = random.random() < 0.6
            y = BODEN - 52 if hoch else BODEN - 26
            self.fruechte.append(Frucht(self.fruchtbilder[art], random.uniform(10, LW - 42), y))

    def figur_wechseln(self, stunde):
        neu = stunde % len(FIGUREN)
        if neu != self.figur_index:
            alt = self.figur
            self.figur_index = neu
            self.figur = Figur(FIGUREN[neu])
            self.figur.x, self.figur.richtung = alt.x, alt.richtung

    def schritt(self, dt, mono, offline, tempo, getippt):
        f = self.figur
        if getippt:
            f.springen()
        ziel = None
        if self.fruechte and not offline:
            fx, _ = f.mitte()
            naechste = min(self.fruechte, key=lambda fr: abs(fr.x + 16 - fx))
            ziel = naechste.x + 16
            if naechste.y < BODEN - 40 and abs(ziel - fx) < 10:
                f.springen()
        f.schritt(dt, mono, ziel, tempo, offline)
        fx, fy = f.mitte()
        for fr in list(self.fruechte):
            if abs(fr.x + 16 - fx) < 12 and abs(fr.y + 16 - fy) < 18:
                self.fruechte.remove(fr)
                self.effekte.append((fr.x, fr.y, mono))
        self.effekte = [e for e in self.effekte if mono - e[2] < 0.3]
        if not self.fruechte and mono > self.bonus_naechst:
            # gelegentlich eine Frucht, damit die Szene lebt, auch wenn Pi-hole nichts blockt
            self.fruechte_dazu(1)
            self.bonus_naechst = mono + random.uniform(30, 60)

    def zeichnen(self, mono, wetter):
        bild = self.statisch.copy()
        for fr in self.fruechte:
            bild.alpha_composite(fr.bild(mono), (int(fr.x), int(fr.y)))
        for ex, ey, start in self.effekte:
            i = min(int((mono - start) * 20), len(self.eingesammelt) - 1)
            bild.alpha_composite(self.eingesammelt[i], (int(ex), int(ey)))
        bild.alpha_composite(self.figur.bild(mono), (int(self.figur.x), int(self.figur.y)))
        if wetter:
            code = wetter["current"]["weather_code"]
            if code in REGEN_CODES or code in SCHNEE_CODES:
                schnee = code in SCHNEE_CODES
                d = ImageDraw.Draw(bild)
                for tr in self.tropfen:
                    if schnee:
                        d.point((tr[0], tr[1]), fill=(255, 255, 255, 255))
                    else:
                        d.line((tr[0], tr[1], tr[0] - 1, tr[1] + 3), fill=(170, 200, 255, 255))
        return bild

    def wetter_bewegen(self, dt, wetter):
        if not wetter:
            return
        code = wetter["current"]["weather_code"]
        tempo = 40 if code in SCHNEE_CODES else 160
        for tr in self.tropfen:
            tr[1] += tempo * dt
            tr[0] += (-10 if code not in SCHNEE_CODES else random.uniform(-8, 8)) * dt
            if tr[1] > BODEN:
                tr[0], tr[1] = random.uniform(0, LW + 20), random.uniform(-20, 0)


def nach_rgb565(bild_logisch):
    a = np.asarray(bild_logisch.convert("RGB"), dtype=np.uint16)
    v = ((a[..., 0] >> 3) << 11) | ((a[..., 1] >> 2) << 5) | (a[..., 2] >> 3)
    return v.repeat(2, axis=0).repeat(2, axis=1).astype("<u2")


def main():
    welt = Welt()
    tippen = Beruehrung()
    wetter, wetter_zeit = None, 0.0
    geblockt, pihole_zeit = None, 0.0
    netz, netz_zeit, offline_seit = None, 0.0, None
    vorher = None
    fd = os.open(FB, os.O_RDWR)
    letzt = time.monotonic()
    while True:
        start = time.monotonic()
        jetzt = datetime.now()
        dt = min(start - letzt, 0.2)
        letzt = start

        # Daten in größeren Abständen holen
        if not wetter_zeit or start - wetter_zeit >= daten.WETTER_ALLE:
            try:
                wetter, wetter_zeit = daten.wetter_holen(), start
            except Exception:
                wetter_zeit = start - daten.WETTER_ALLE + 120
        if not pihole_zeit or start - pihole_zeit >= daten.PIHOLE_ALLE:
            try:
                neu = daten.pihole_heute()[1]
                if geblockt is not None and neu > geblockt:
                    welt.fruechte_dazu(neu - geblockt)
                geblockt = neu
            except Exception:
                pass
            pihole_zeit = start
        if not netz_zeit or start - netz_zeit >= daten.NETZ_ALLE:
            ms = daten.online_pruefen()
            offline_seit = None if ms is not None else (offline_seit or jetzt)
            netz, netz_zeit = (ms, offline_seit, daten.speed_lesen()), start

        welt.figur_wechseln(jetzt.hour)
        schluessel = (jetzt.strftime("%Y%m%d%H%M"), id(wetter), geblockt,
                      None if netz is None else (netz[0] is None, netz[1], str(netz[2])))
        if schluessel != welt.statisch_schluessel:
            welt.statisch = welt.hintergrund(jetzt, wetter, geblockt, netz)
            welt.statisch_schluessel = schluessel

        offline = netz is not None and netz[0] is None
        speed = netz[2] if netz else None
        tempo = 45.0
        if speed and "down_mbit" in speed:
            tempo = min(max(25.0 + speed["down_mbit"] / 8, 30.0), 75.0)
        welt.schritt(dt, start, offline, tempo, tippen.angetippt())
        welt.wetter_bewegen(dt, wetter)

        neu = nach_rgb565(welt.zeichnen(start, wetter))
        if vorher is None:
            zeilen = np.array([0, neu.shape[0] - 1])
        else:
            zeilen = np.flatnonzero((neu != vorher).any(axis=1))
        if zeilen.size:
            oben, unten = zeilen[0], zeilen[-1] + 1
            os.lseek(fd, int(oben) * neu.shape[1] * 2, os.SEEK_SET)
            os.write(fd, neu[oben:unten].tobytes())
        vorher = neu

        rest = 1 / FPS - (time.monotonic() - start)
        if rest > 0:
            time.sleep(rest)


if __name__ == "__main__":
    os.nice(5)
    main()
