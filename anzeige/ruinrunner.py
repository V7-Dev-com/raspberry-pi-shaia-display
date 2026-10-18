#!/usr/bin/env python3
"""Ruin-Runner-Szene für das 3,5-Zoll-Display: Shaia im Dungeon als Info-Anzeige.

Grafiken: "Shaia" aus "Ruin Runners" von picopico256 (https://picopico256.itch.io/shaia-ruin-runnerscharacter).
Nutzung privat und kommerziell erlaubt, WEITERGABE DER GRAFIKEN VERBOTEN - die Dateien
liegen deshalb nur auf dem Pi und lokal, nie in einem öffentlichen Repo.

Was die Szene zeigt:
  - oben: Uhrzeit, Datum, Temperatur/Wetter/UV, Luftfeuchte/Böen und Wetter-Warnungen
    (Sturmböen, UV, Hitze, Gewitter, Starkregen, Schnee) als farbige Schilder
  - unten drei Zeilen: Wetter-Details, Internet, System (Pi-hole, CPU, Laufzeit)
  - jede von Pi-hole geblockte Anfrage schickt ein Skelett (3-6 Lebenspunkte); Shaia kämpft
    mit wechselnden Angriffen, das Skelett schlägt zurück, sie blockt, wird getroffen oder
    umgeworfen; erst bei 0 Lebenspunkten zerfällt das Skelett
  - ohne Gegner: Alltag (stehen, ducken, suchen, nachdenken, Schwert verzaubern), Raumwechsel
    Spaziergänge durch die Halle (die Kamera scrollt mit), zur vollen Stunde eine Schatztruhe,
    nachts das Lagerfeuer
  - Internet weg: Shaia steht unter Strom
  - Beute: zerfallene Skelette lassen Orbs, Tränke oder Essen fallen, Shaia hebt sie auf;
    ihr Zauber schießt Element-Orbs (Feuer, Eis, Blitz, Gift);
    nachts liegt ein Hähnchen am Lagerfeuer
  - Antippen: Shaia springt

Gezeichnet wird in voller Display-Auflösung (480x320); nur der geänderte Zeilenbereich geht
ans Display.
"""
import glob
import os
import sys
import random
import time
from collections import deque
from datetime import datetime

import numpy as np
from PIL import Image, ImageDraw, ImageFont

import anzeige as daten
import hud
from pixelwelt import WETTER_WORT, Beruehrung

ASSETS = os.environ.get("PI_ASSETS", "/opt/pi-anzeige/assets")      # Grafiken liegen nicht im Repo
SPRITES = os.path.join(ASSETS, "shaia")
SHAIA_URL = "https://picopico256.itch.io/shaia-ruin-runnerscharacter"
BENOETIGT = ("shaia", "skeleton", "vfx", "prop", "item", "background")   # Unterordner des Shaia-Pakets (sprites/)
FB = "/dev/fb0"
W, H = 480, 320
FUSS = 243                     # Standlinie der Figuren
LEISTE = 248                   # Oberkante des Info-Streifens unten (5 px Boden zwischen Füßen und Leiste)
ANKER_X, ANKER_Y = 118, 246    # Fußpunkt in den 256er-Einzelbildern
SEITE_SEKUNDEN = 4             # so lange steht jeder Eintrag der großen Zeile unten
FPS = 24                       # Obergrenze; beim Scrollen bremst die Übertragung (~20/s)
SCHWERKRAFT = 1100.0
HALLE = 1536                   # Breite der ganzen Halle (drei Dungeon-Bilder nebeneinander)
RAND = 150                     # Kamera zieht nach, wenn Shaia näher als das an den Bildrand kommt
KAMERA_TEMPO = 240.0           # Höchsttempo der Kamera in Pixel pro Sekunde
KAMERA_BESCHL = 360.0          # sanftes Anfahren und Bremsen (Pixel pro Sekunde²)
KOPF = 94                      # Höhe der festen Kopfleiste (Uhr, Datum, Wetter)
NACHT = (22, 6)                  # Lagerfeuer-Zeit (von, bis)
TRUHE_X = HALLE - 120          # feste Truhe ganz rechts in der Halle
LAGER_X = 330                  # fester Platz am Lagerfeuer (Shaia sitzt links davon)


def spi_takt(standard=24_000_000):
    """Takt der Display-Leitung, wie ihn das System gesetzt hat (dtoverlay=piscreen,...,speed=...)."""
    try:
        with open("/sys/bus/spi/devices/spi0.0/of_node/spi-max-frequency", "rb") as f:
            return int.from_bytes(f.read(4), "big") or standard
    except OSError:
        return standard


SPI_HZ = spi_takt()

SCHRIFT = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"
SCHRIFT_FETT = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"
# kräftige Farben, damit es auch aus ein paar Metern lesbar ist
WEISS, GEDIMMT, GRUEN, ROT = (255, 255, 255), (228, 234, 245), (120, 255, 140), (255, 110, 110)
WARN_GELB, WARN_ROT = (225, 145, 20), (205, 40, 40)
KONTUR = (5, 8, 20)
WOCHENTAGE = ["Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag", "Sonntag"]
MONATE = ["Januar", "Februar", "März", "April", "Mai", "Juni", "Juli",
          "August", "September", "Oktober", "November", "Dezember"]


# --- Text ---------------------------------------------------------------------------------
def schrift(groesse, fett=False, _cache={}):
    schluessel = (groesse, fett)
    if schluessel not in _cache:
        _cache[schluessel] = ImageFont.truetype(SCHRIFT_FETT if fett else SCHRIFT, groesse)
    return _cache[schluessel]


def text(d, xy, inhalt, groesse, farbe=WEISS, fett=False, anker="la", kontur=2, max_breite=None):
    """Glatte Schrift mit dunkler Kontur; mit max_breite wird sie verkleinert, bis der Text passt."""
    while max_breite and groesse > 9 and d.textlength(inhalt, font=schrift(groesse, fett)) > max_breite:
        groesse -= 1
    d.text(xy, inhalt, font=schrift(groesse, fett), fill=farbe, anchor=anker,
           stroke_width=kontur, stroke_fill=KONTUR)


def laufzeit():
    with open("/proc/uptime") as f:
        minuten = int(float(f.read().split()[0]) // 60)
    tage, rest = divmod(minuten, 1440)
    stunden, minuten = divmod(rest, 60)
    return f"{tage} T {stunden} h" if tage else f"{stunden} h {minuten} min"


def warnungen(wetter):
    """Liste (Text, Farbe) der aktuell gültigen Wetter-Warnungen."""
    if not wetter:
        return []
    c = wetter["current"]
    w = []
    code = c.get("weather_code")
    if code in (95, 96, 99):
        w.append(("⚡ GEWITTER", WARN_ROT))
    boeen = c.get("wind_gusts_10m") or 0
    if boeen >= 75:
        w.append((f"⚠ STURM {boeen:.0f} km/h", WARN_ROT))
    elif boeen >= 50:
        w.append((f"⚠ Sturmböen {boeen:.0f} km/h", WARN_GELB))
    uv = c.get("uv_index") or 0
    if c.get("is_day", 1) and uv >= 8:
        w.append((f"☀ UV {uv:.0f} sehr hoch", WARN_ROT))
    elif c.get("is_day", 1) and uv >= 6:
        w.append((f"☀ UV {uv:.0f} hoch", WARN_GELB))
    temp = c.get("temperature_2m") or 0
    if temp >= 38:
        w.append((f"⚠ HITZE {temp:.0f}°", WARN_ROT))
    elif temp >= 35:
        w.append((f"⚠ Hitze {temp:.0f}°", WARN_GELB))
    if code in (65, 82):
        w.append(("⚠ Starkregen", WARN_GELB))
    if code in (71, 73, 75, 77, 85, 86):
        w.append(("❄ Schnee", WARN_GELB))
    return w


STIL_DATEI = "/opt/pi-anzeige/stil.txt"   # enthält den Namen des HUD-Stils (v7, glut, retro, klar)


def stil_lesen():
    try:
        with open(STIL_DATEI) as f:
            name = f.read().strip()
        return name if name in hud.STILE else "klar"
    except OSError:
        return "klar"


# --- Grafiken -----------------------------------------------------------------------------
class Bild:
    """Auf den sichtbaren Teil zugeschnittenes Einzelbild mit Versatz im 256er-Raster."""
    __slots__ = ("bild", "dx", "dy")

    def __init__(self, bild, dx, dy):
        self.bild, self.dx, self.dy = bild, dx, dy


def lade_reihe(*muster):
    """Einzelbilder (schauen nach rechts) plus gespiegelte, zugeschnitten auf ihren Inhalt."""
    rechts, links = [], []
    for m in muster:
        for pfad in sorted(glob.glob(os.path.join(SPRITES, m))):
            b = Image.open(pfad).convert("RGBA")
            box = b.getbbox() or (0, 0, 1, 1)
            teil = b.crop(box)
            rechts.append(Bild(teil, box[0], box[1]))
            links.append(Bild(teil.transpose(Image.Transpose.FLIP_LEFT_RIGHT), b.width - box[2], box[1]))
    if not rechts:
        raise FileNotFoundError(f"keine Bilder für {muster}")
    return rechts, links


def lade_einfach(muster):
    return [Image.open(p).convert("RGBA") for p in sorted(glob.glob(os.path.join(SPRITES, muster)))]


# Animationen: Name -> (Muster, Bilder pro Sekunde)
S = "shaia/"
SHAIA_ANIM = {
    "stehen_a": ((S + "sprites_common/common_00_idle_stand_A*.png",), 10),
    "stehen_b": ((S + "sprites_common/common_00_idle_stand_B*.png",), 10),
    "gehen": ((S + "sprites_common/common_11_walk*.png",), 14),
    "rennen": ((S + "sprites_common/common_12_run*.png",), 14),
    "drehen": ((S + "sprites_common/common_03_turn_stand_A*.png",), 14),
    "ducken_rein": ((S + "sprites_common/common_01_idle_to_crouch*.png",), 12),
    "ducken": ((S + "sprites_common/common_01_idle_crouch.png",), 4),
    "ducken_raus": ((S + "sprites_common/common_01_crouch_to_idle*.png",), 12),
    "absprung": ((S + "sprites_common/common_21_jump_begin*.png",), 12),
    "steigen": ((S + "sprites_common/common_21_jump_up*.png",), 12),
    "fallen": ((S + "sprites_common/common_21_jump_down*.png",), 10),
    "landen": ((S + "sprites_common/common_22_landing*.png",), 14),
    "abwehr": ((S + "sprites_common/common_31_guard_stand*.png",), 10),
    "kombo1": ((S + "sprites_attack/attack_01_cobination*.png",), 16),
    "kombo2": ((S + "sprites_attack/attack_02_cobination*.png",), 16),
    "kombo3": ((S + "sprites_attack/attack_03_cobination*.png",), 16),
    "kombo4": ((S + "sprites_attack/attack_04_cobination*.png",), 16),
    "knie": ((S + "sprites_attack/attack_05_knee*.png",), 14),
    "duckangriff": ((S + "sprites_attack/attack_11_crounch_attack*.png",), 16),
    "sprungangriff": ((S + "sprites_attack/attack_21_jump_attack*.png",), 14),
    "tritt_rein": ((S + "sprites_attack/attack_06_run_kick_begin*.png",), 12),
    "tritt": ((S + "sprites_attack/attack_06_run_kick_0*.png",), 10),
    "tritt_raus": ((S + "sprites_attack/attack_06_run_kick_end*.png",), 12),
    "schild_rein": ((S + "special/special_01_shield_dash_begin_*.png",), 12),
    "schild": ((S + "special/special_01_shield_dash_loop_*.png",), 14),
    "schild_raus": ((S + "special/special_01_shield_dash_end_*.png",), 12),
    "aufwaerts_rein": ((S + "special/special_02_rising_begin_*.png",), 14),
    "aufwaerts": ((S + "special/special_02_rising_attack_*.png",), 12),
    "aufwaerts_raus": ((S + "special/special_02_rising_end_*.png",), 10),
    "kreisschlag": ((S + "special/special_03_circle_slash_*.png",), 18),
    "verzaubern": ((S + "special/special_06_enchant_*.png",), 10),
    "zauber_rein": ((S + "special/special_07_magic_begin_*.png",), 8),
    "zauber": ((S + "special/special_07_magic_loop_*.png",), 10),
    "zauber_raus": ((S + "special/special_07_magic_end_*.png",), 8),
    "charme_rein": ((S + "special/special_10_charm_magic_begin_*.png",), 10),
    "charme": ((S + "special/special_10_charm_magic_loop_*.png",), 4),
    "charme_raus": ((S + "special/special_10_charm_magic_end_*.png",), 10),
    "ultimate": ((S + "special/special_11_Ultimate_*.png",), 8),
    "treffer_kopf": ((S + "sprites_damage/damage_01_damage_head.png",), 4),
    "treffer_koerper": ((S + "sprites_damage/damage_02_damage_body.png",), 4),
    "umgeworfen": ((S + "through/special_05_through*.png",), 9),
    "aufstehen": ((S + "sprites_damage/damage_13_standup*.png",), 10),
    "strom": ((S + "sprites_damage/damage_23_electric*.png",), 12),
    "provozieren": ((S + "misc/misc_06_taunt_*.png",), 6),
    "lachen": ((S + "misc/misc_03_laugh_*.png",), 6),
    "suchen": ((S + "misc/misc_02_search_*.png",), 4),
    "nachdenken": ((S + "misc/misc_05_think_*.png",), 2),
    "lagerfeuer": ((S + "misc/misc_08_camp_*.png",), 3),
    "truhe_rein": ((S + "misc/misk_07_open_chest_begin_*.png",), 8),
    "truhe_raus": ((S + "misc/misk_07_open_chest_end_*.png",), 8),
}
SKELETT_ANIM = {
    "stehen": (("skeleton/common_01_idle*.png",), 4),
    "gehen": (("skeleton/common_11_walk*.png",), 10),
    "angriff": (("skeleton/attack_01_sword*.png",), 12),
    "treffer_kopf": (("skeleton/damage_01_damage_head.png",), 4),
    "treffer_koerper": (("skeleton/damage_02_damage_body.png",), 4),
    "wurf_rein": (("skeleton/damage_11_blow_begin*.png",), 12),
    "flug": (("skeleton/damage_11_blow_air*.png",), 12),
    "liegen": (("skeleton/damage_11_blow_landing*.png",), 4),
    "aufstehen": (("skeleton/damage_13_standup*.png",), 10),
    "sprung_rein": (("skeleton/common_21_jump_begin*.png",), 12),
    "sprung_hoch": (("skeleton/common_21_jump_up*.png",), 8),
    "sprung_runter": (("skeleton/common_21_jump_down*.png",), 8),
    "landung": (("skeleton/common_22_landing*.png",), 12),
}
# Animationen, bei denen ein auftauchender Gegner Shaias Alltag unterbrechen darf
ALLTAG = {"stehen_a", "stehen_b", "gehen", "ducken_rein", "ducken", "ducken_raus", "suchen", "nachdenken",
          "verzaubern", "drehen", "lagerfeuer", "lachen", "provozieren", "truhe_rein", "truhe_raus", None}


def lade_anims(tabelle):
    return {name: (lade_reihe(*muster), fps) for name, (muster, fps) in tabelle.items()}


# --- Ablauf aus Schritten -------------------------------------------------------------------
class Treffer:
    __slots__ = ("schaden", "reichweite", "art")

    def __init__(self, schaden=1, reichweite=120, art="normal"):
        self.schaden, self.reichweite, self.art = schaden, reichweite, art


class Schritt:
    """Ein Abschnitt einer Handlung: Animation, optional Bewegung, Treffer und Ende-Bedingung.
    Ohne dauer/fertig_wenn läuft die Animation genau einmal durch."""
    __slots__ = ("anim", "dauer", "vx", "treffer", "loop", "beim_start", "fertig_wenn")

    def __init__(self, anim, dauer=None, vx=0.0, treffer=None, loop=None, beim_start=None, fertig_wenn=None):
        self.anim, self.dauer, self.vx = anim, dauer, vx
        self.treffer = treffer or {}          # Bildnummer -> Treffer
        self.loop = (dauer is not None or fertig_wenn is not None) if loop is None else loop
        self.beim_start, self.fertig_wenn = beim_start, fertig_wenn


class Akteur:
    def __init__(self, anims, x, richtung=1):
        self.anims = anims
        self.x, self.hoehe, self.vy, self.vx_luft = x, 0.0, 0.0, 0.0
        self.richtung = richtung
        self.plan = deque()
        self.schritt = None
        self.seit = 0.0
        self.erledigt = set()     # schon ausgelöste Treffer-Bilder des aktuellen Schritts

    def planen(self, schritte, jetzt):
        """Ersetzt die laufende Handlung."""
        self.plan = deque(schritte)
        self.weiter(jetzt)

    def weiter(self, jetzt):
        self.schritt = self.plan.popleft() if self.plan else None
        self.seit, self.erledigt = jetzt, set()
        if self.schritt and self.schritt.beim_start:
            self.schritt.beim_start()

    def bildnummer(self, jetzt):
        (rechts, _), fps = self.anims[self.schritt.anim]
        n = len(rechts)
        roh = int((jetzt - self.seit) * fps)
        return ((roh % n) if self.schritt.loop else min(roh, n - 1)), roh, n

    def physik(self, dt):
        if self.hoehe > 0 or self.vy > 0:
            self.vy -= SCHWERKRAFT * dt
            self.hoehe = max(self.hoehe + self.vy * dt, 0.0)
            self.x += self.vx_luft * dt
            if self.hoehe == 0:
                self.vy = self.vx_luft = 0.0

    def tick(self, dt, jetzt):
        """Bewegt, prüft das Schritt-Ende und liefert die ausgelösten Treffer."""
        self.physik(dt)
        if not self.schritt:
            return []
        s = self.schritt
        self.x += s.vx * self.richtung * dt
        nr, roh, n = self.bildnummer(jetzt)
        ausgeloest = []
        for bild_nr, t in s.treffer.items():
            if bild_nr not in self.erledigt and roh >= bild_nr:
                self.erledigt.add(bild_nr)
                ausgeloest.append(t)
        if s.fertig_wenn:
            fertig = s.fertig_wenn()
        elif s.dauer is not None:
            fertig = jetzt - self.seit >= s.dauer
        else:
            fertig = roh >= n
        if fertig:
            self.weiter(jetzt)
        return ausgeloest

    @property
    def frei(self):
        return self.schritt is None

    @property
    def anim(self):
        return self.schritt.anim if self.schritt else None

    def bild(self, jetzt):
        if self.schritt:
            (rechts, links), _ = self.anims[self.schritt.anim]
            nr = self.bildnummer(jetzt)[0]
        else:
            (rechts, links), _ = next(iter(self.anims.values()))
            nr = 0
        b = (rechts if self.richtung > 0 else links)[nr]
        ax = ANKER_X if self.richtung > 0 else 256 - ANKER_X
        return b.bild, int(self.x - ax + b.dx), int(FUSS - self.hoehe - ANKER_Y + b.dy)


class Skelett(Akteur):
    def __init__(self, anims, x, richtung):
        super().__init__(anims, x, richtung)
        self.lp = random.randint(3, 6)
        self.zustand = "kommt"      # kommt, steht, getroffen, luft, liegt, betaeubt, tot
        self.angriff_ab = 0.0
        self.springer = random.random() < 0.5      # springt Shaia aus der Ferne an
        self.sprung_ab = 0.0
        self.teile = []
        self.tot_seit = None

    @property
    def kampffaehig(self):
        return self.zustand in ("kommt", "steht", "getroffen", "betaeubt")


# --- Szene --------------------------------------------------------------------------------
class Szene:
    def __init__(self):
        self.shaia_anims = lade_anims(SHAIA_ANIM)
        self.skelett_anims = lade_anims(SKELETT_ANIM)
        self.truemmer = [lade_reihe(f"skeleton/debris_{n}*.png")
                         for n in ("bone", "skul", "shiield", "sword", "waist", "chest")]
        self.vfx = {n: lade_einfach(f"vfx/vfx_{n}/vfx_{n}*.png")
                    for n in ("hit", "coin", "guard", "stun", "fire_banish", "ground_shock", "twinkle", "smoke")}
        self.feuer = lade_einfach("shaia/misc/misc_08_fire_*.png")
        self.truhe = lade_einfach("prop/chest_open*.png")
        self.orbs = {e: lade_einfach(f"item/item_orb/item_orb_{e}*.png")
                     for e in ("fire", "ice", "thunder", "poison", "cure")}
        self.essen = {n: [Image.open(os.path.join(SPRITES, f"item/item_food/item_{n}.png")).convert("RGBA")]
                      for n in ("chicken", "cheese", "bread", "potion_red", "potion_blue", "potion_green")}
        self.feuer_klein = lade_einfach("vfx/vfx_fire/vfx_fire*.png")
        panorama = Image.new("RGBA", (1536, 384))
        for i, n in enumerate(("01_dungeon_left", "02_dungeon_center", "03_dungeon_right")):
            teil = Image.open(os.path.join(SPRITES, f"background/sprites_dungeon/{n}.png")).convert("RGBA")
            panorama.paste(teil, (i * 512, 0))
        self.halle = panorama.crop((0, 40, HALLE, 40 + H))
        self.nacht_schicht = Image.new("RGBA", (W, H), (7, 16, 32, 62))     # ~24 % Nachtblau
        self.kamera = (HALLE - W) / 2
        self.kamera_v = 0.0

        self.jetzt = time.monotonic()
        self.shaia = Akteur(self.shaia_anims, HALLE / 2)
        self.shaia_offline = False
        self.im_kampf = False
        self.skelette = []
        self.wartend = 0
        self.effekte = []          # [bilder, x, y, start, fps]
        self.beute = []            # [x, bilder, seit]
        self.geschosse = []        # [x, y, ziel, element, bilder]
        self.kugeln = []           # Fernangriffe: dict(x, hoehe, richtung, bilder, reagiert, geblockt)
        self.kugel_ab = self.jetzt + random.uniform(25, 50)
        # fliegende Kugeln in Originalgröße und ohne Bild 4 (fast reines Weiß, wirkte wie ein Mond)
        self.kugel_bilder = {e: [b for i, b in enumerate(self.orbs[e]) if i != 3]
                             for e in ("fire", "ice", "thunder", "poison")}
        self.funkeln_klein = [b.resize((b.width // 2, b.height // 2), Image.Resampling.NEAREST)
                              for b in self.vfx["twinkle"]]
        self.nacht = False
        self.truhe_x = TRUHE_X
        self.truhe_seit = None          # Öffnen läuft seit ...
        self.truhe_zu_seit = None       # Schließen läuft seit ...
        self.truhe_besuch = False       # Shaia ist gerade bei der Truhe (Kamera folgt)
        self.truhe_ab = self.jetzt + random.uniform(40, 90)
        self.truhe_fuer_stunde = datetime.now().hour
        self.spaziergang_ab = self.jetzt + random.uniform(40, 90)
        self.bonus_ab = self.jetzt + 45
        self.letzter_angriff = None
        self.sieg = False
        self.lager_ab = 0.0            # nach einem Kampf erst durchatmen, dann wieder ans Feuer
        self.verzaubert_im_kampf = False
        self.statisch = None
        self.statisch_schluessel = None

    # ---------- statisches Bild ----------
    def oberflaeche(self, jetzt, wetter, pihole, netz, seite=0):
        """Alles Textliche als eigene Ebene über der scrollenden Halle (Gestaltung: hud.py)."""
        bild, self.seiten = hud.zeichnen(stil_lesen(), W, H, KOPF, LEISTE, jetzt, wetter, pihole, netz,
                                         warnungen(wetter), WETTER_WORT, daten.cpu_temperatur, laufzeit,
                                         seite)
        return bild

    # ---------- Hilfen ----------
    def effekt(self, name, x, y, fps=14):
        self.effekte.append([self.vfx[name], x, y, self.jetzt, fps])

    def gegner(self):
        return next((s for s in self.skelette if s.zustand != "tot"), None)

    def skelette_dazu(self, anzahl):
        self.wartend = min(self.wartend + anzahl, 6)

    # ---------- Treffer auf ein Skelett ----------
    def treffe(self, s, t):
        sh, jetzt = self.shaia, self.jetzt
        dx = s.x - sh.x
        if not s.kampffaehig or abs(dx) > t.reichweite or (dx * sh.richtung < -20 and t.reichweite < 1000):
            return
        weg = 1 if dx >= 0 else -1
        if t.art == "charme":
            s.zustand = "betaeubt"
            s.planen([Schritt("stehen", dauer=2.8)], jetzt)
            return
        if t.art in ("ice", "thunder"):
            self.effekt("hit", s.x - 64, FUSS - 144, fps=18)
            self.effekt("twinkle" if t.art == "ice" else "stun", s.x - 32, FUSS - 150, fps=12)
            s.lp -= t.schaden
            if s.lp <= 0:
                self.umwerfen(s, weg * 190, 280)
            else:
                s.zustand = "betaeubt"
                s.planen([Schritt("treffer_koerper", dauer=1.4)], jetzt)
            return
        if t.art == "poison":
            self.effekt("smoke", s.x - 16, FUSS - 140, fps=10)
            self.effekt("smoke", s.x - 26, FUSS - 110, fps=8)
        if t.art == "fire":
            self.effekt("fire_banish", s.x - 32, FUSS - 110, fps=12)
            self.effekte.append([self.feuer_klein, s.x - 16, FUSS - 64, self.jetzt, 16])
        if t.art == "feuer":
            self.effekt("fire_banish", s.x - 32, FUSS - 110, fps=12)
        elif t.art == "ultimate":
            self.effekt("ground_shock", s.x - 64, FUSS - 26, fps=12)
            self.effekt("hit", s.x - 64, FUSS - 144, fps=16)
        else:
            self.effekt("hit", s.x - 64 - weg * 10, FUSS - 144, fps=18)
        s.lp -= t.schaden
        if s.lp <= 0 or t.art in ("wurf", "hoch", "ultimate"):
            vx, vy = {"hoch": (70, 520), "ultimate": (300, 420)}.get(t.art, (190, 280))
            self.umwerfen(s, weg * vx, vy)
        else:
            s.zustand = "getroffen"
            s.x += weg * 12
            s.planen([Schritt(random.choice(["treffer_kopf", "treffer_koerper"]), dauer=0.3)], jetzt)

    def umwerfen(self, s, vx, vy):
        s.richtung = -1 if vx > 0 else 1        # fliegt rückwärts, Blick zu Shaia
        s.zustand = "luft"

        def abheben():
            s.vy, s.vx_luft, s.hoehe = vy, vx, 1.0

        def liegt():
            s.zustand = "liegt"
            self.effekt("smoke", s.x - 16, FUSS - 28, fps=16)

        def steht_auf():
            s.zustand = "steht"

        schritte = [Schritt("wurf_rein", dauer=0.08),
                    Schritt("flug", beim_start=abheben, fertig_wenn=lambda: s.hoehe == 0 and s.vy == 0),
                    Schritt("liegen", dauer=0.9, beim_start=liegt)]
        if s.lp > 0:
            schritte += [Schritt("aufstehen"), Schritt("stehen", dauer=0.3, beim_start=steht_auf)]
        else:
            schritte.append(Schritt("liegen", dauer=0.01, beim_start=lambda: self.zerfallen(s)))
        s.planen(schritte, self.jetzt)

    def zerfallen(self, s):
        s.zustand, s.tot_seit = "tot", self.jetzt
        for reihe in self.truemmer:
            s.teile.append([s.x, FUSS - 50, random.uniform(-170, 170), random.uniform(-340, -150), reihe[0]])
        self.effekt("coin", s.x - 8, FUSS - 120, fps=10)
        self.effekt("twinkle", s.x - 32, FUSS - 150, fps=12)
        self.sieg = True
        if random.random() < 0.6 and len(self.beute) < 3:
            art = random.choice(["orb"] * 3 + ["essen"] * 2)
            bilder = (self.orbs[random.choice(list(self.orbs))] if art == "orb"
                      else self.essen[random.choice(["cheese", "bread", "potion_red", "potion_blue", "potion_green"])])
            self.beute.append([min(max(s.x, 80), HALLE - 80), bilder, self.jetzt])

    # ---------- Angriff des Skeletts ----------
    def skelett_springt(self, s):
        """Sprung auf Shaia zu: Anlauf, Flug, Landung ~85 px vor ihr - der Schlag kommt mit der Landung."""
        sh, jetzt = self.shaia, self.jetzt
        s.sprung_ab = jetzt + random.uniform(5, 9)
        weite = max(abs(sh.x - s.x) - 85, 55)
        flug = 2 * 300 / SCHWERKRAFT

        def ab():
            s.zustand = "luft"
            s.vy = 300.0
            s.vx_luft = s.richtung * weite / flug

        def unten():
            s.zustand = "steht"
        s.planen([Schritt("sprung_rein", dauer=0.25),
                  Schritt("sprung_hoch", beim_start=ab, fertig_wenn=lambda: s.vy <= 0),
                  Schritt("sprung_runter", fertig_wenn=lambda: s.hoehe == 0),
                  Schritt("landung", beim_start=unten, treffer={0: Treffer(1, 150)})], jetzt)

    def skelett_schlaegt(self, s):
        sh, jetzt = self.shaia, self.jetzt
        if abs(s.x - sh.x) > 150 or self.shaia_offline or sh.hoehe > 0:
            return
        if sh.anim not in ("stehen_a", "stehen_b", "abwehr", "gehen", "rennen", "drehen", None):
            return          # Shaia greift gerade an oder liegt schon
        mitte = (s.x + sh.x) / 2
        sh.richtung = 1 if s.x > sh.x else -1
        wurf = random.random()
        if wurf < 0.6:
            self.effekt("guard", mitte - 32, FUSS - 110, fps=16)
            sh.planen([Schritt("abwehr", dauer=0.45, vx=-25)], jetzt)
        elif wurf < 0.9:
            self.effekt("hit", sh.x - 64, FUSS - 150, fps=18)
            sh.planen([Schritt(random.choice(["treffer_kopf", "treffer_koerper"]), dauer=0.35, vx=-40),
                       Schritt("stehen_b", dauer=0.3)], jetzt)
        else:
            self.effekt("hit", sh.x - 64, FUSS - 150, fps=18)
            sh.planen([Schritt("umgeworfen", vx=-35), Schritt("stehen_b", dauer=0.4)], jetzt)

    # ---------- Shaias Angriffe ----------
    def angriff_planen(self, s):
        sh, jetzt = self.shaia, self.jetzt
        d = abs(s.x - sh.x)
        T = Treffer
        zuege = {
            "kombo": 3.0, "knie": 1.3, "duck": 0.0, "sprung": 1.1, "tritt": 3.0 if d > 170 else 0.8,
            "kreis": 1.2, "schild": 1.0, "aufwaerts": 1.0, "zauber": 1.6 if d > 160 else 0.5,
            "charme": 0.0 if s.zustand == "betaeubt" else 0.6, "ultimate": 2.5 if s.lp <= 2 else 0.0,
        }
        if self.letzter_angriff in zuege:
            zuege[self.letzter_angriff] *= 0.15
        zug = random.choices(list(zuege), weights=list(zuege.values()))[0]
        self.letzter_angriff = zug

        reichweite = {"zauber": 300, "charme": 300, "ultimate": 190, "tritt": 130, "kreis": 140,
                      "schild": 260, "sprung": 120}.get(zug, 110)
        plan = []
        if not self.verzaubert_im_kampf and random.random() < 0.35:
            self.verzaubert_im_kampf = True
            plan.append(Schritt("verzaubern"))
        if d > reichweite:
            ziel = reichweite - 10
            weit = d - ziel > 70
            plan.append(Schritt("rennen" if weit else "gehen", vx=170 if weit else 60,
                                fertig_wenn=lambda: abs(s.x - sh.x) <= ziel or not s.kampffaehig))

        def springen():
            sh.vy, sh.vx_luft = 310, 95 * sh.richtung     # flach: Kopf bleibt unter der Kopfleiste

        if zug == "kombo":
            for i in range(1, random.randint(2, 4) + 1):
                plan.append(Schritt(f"kombo{i}", vx=25,
                                    treffer={5 if i == 4 else 2: T(1, 125, "wurf" if i == 4 else "normal")}))
        elif zug == "knie":
            plan.append(Schritt("knie", vx=40, treffer={3: T(1, 105)}))
        elif zug == "duck":
            plan += [Schritt("ducken_rein"),
                     Schritt("duckangriff", treffer={4: T(1, 135), 10: T(1, 135)}),
                     Schritt("ducken_raus")]
        elif zug == "sprung":
            plan += [Schritt("absprung", dauer=0.08),
                     Schritt("sprungangriff", beim_start=springen, loop=False,
                             fertig_wenn=lambda: sh.hoehe == 0 and sh.vy == 0, treffer={3: T(1, 130)}),
                     Schritt("landen")]
        elif zug == "tritt":
            plan += [Schritt("tritt_rein", vx=160), Schritt("tritt", vx=200, treffer={0: T(1, 140, "wurf")}),
                     Schritt("tritt_raus", vx=40)]
        elif zug == "kreis":
            plan.append(Schritt("kreisschlag", treffer={4: T(1, 150), 9: T(1, 150), 14: T(1, 150, "wurf")}))
        elif zug == "schild":
            plan += [Schritt("schild_rein"),
                     Schritt("schild", vx=280, dauer=0.45, treffer={1: T(1, 90, "wurf")}),
                     Schritt("schild_raus")]
        elif zug == "aufwaerts":
            plan += [Schritt("aufwaerts_rein"), Schritt("aufwaerts", treffer={1: T(2, 125, "hoch")}),
                     Schritt("aufwaerts_raus")]
        elif zug == "zauber":
            element = random.choice(["fire", "ice", "thunder", "poison"])

            def abschiessen():
                self.geschosse.append([sh.x + 50 * sh.richtung, FUSS - 125, s, element, self.orbs[element]])
            plan += [Schritt("zauber_rein"), Schritt("zauber", dauer=0.7, beim_start=abschiessen),
                     Schritt("zauber_raus")]
        elif zug == "charme":
            plan += [Schritt("charme_rein"), Schritt("charme", dauer=1.0, treffer={1: T(0, 320, "charme")}),
                     Schritt("charme_raus")]
        elif zug == "ultimate":
            plan += [Schritt("verzaubern"),
                     Schritt("ultimate", dauer=0.8, treffer={3: T(99, 200, "ultimate")})]
        plan.append(Schritt(random.choice(["stehen_b", "abwehr"]), dauer=random.uniform(0.3, 0.8)))
        sh.planen(plan, jetzt)

    # ---------- Alltag ----------
    def gehen_nach(self, ziel):
        sh = self.shaia

        def ausrichten():
            sh.richtung = 1 if ziel > sh.x else -1
        return Schritt("gehen", vx=60, beim_start=ausrichten,
                       fertig_wenn=lambda: abs(sh.x - ziel) < 4 or (ziel - sh.x) * sh.richtung < 0)

    def truhe_weg(self):
        self.truhe_seit = self.truhe_zu_seit = None
        self.truhe_besuch = False

    def alltag_planen(self, jetzt_dt):
        sh, jetzt = self.shaia, self.jetzt
        stunde = jetzt_dt.hour
        if self.sieg:
            self.sieg, self.verzaubert_im_kampf = False, False
            sh.planen([Schritt(random.choice(["lachen", "provozieren"]), dauer=random.uniform(1.4, 2.4)),
                       Schritt("stehen_a", dauer=1.0)], jetzt)
            return
        if self.beute:
            stueck = min(self.beute, key=lambda b: abs(b[0] - sh.x))
            richtung = 1 if stueck[0] > sh.x else -1

            def aufheben():
                if stueck in self.beute:
                    self.beute.remove(stueck)
                    self.effekt("twinkle", stueck[0] - 32, FUSS - 70, fps=12)
                    self.effekt("coin", stueck[0] - 8, FUSS - 90, fps=10)
            sh.planen([self.gehen_nach(stueck[0] - 45 * richtung),
                       Schritt("drehen", beim_start=lambda: setattr(sh, "richtung", richtung)),
                       Schritt("suchen", loop=False),                  # bückt sich, sucht den Boden ab
                       Schritt("ducken", dauer=0.4, beim_start=aufheben), Schritt("ducken_raus"),
                       Schritt("lachen", dauer=1.0)], jetzt)
            return
        # Truhe ganz links: zur vollen Stunde und ab und zu zwischendurch geht sie hin und holt etwas raus
        if (stunde != self.truhe_fuer_stunde or jetzt > self.truhe_ab) and not self.kugeln:
            self.truhe_fuer_stunde = stunde
            self.truhe_ab = jetzt + random.uniform(120, 240)
            tx = self.truhe_x
            seite = 1 if tx > HALLE / 2 else -1             # Truhe rechts: Shaia steht links davon
            ziel = tx - seite * 88                                 # Abstand, damit Truhe und Shaia getrennt bleiben

            def hin():
                self.truhe_besuch = True

            def oeffnen():
                self.truhe_seit, self.truhe_zu_seit = self.jetzt, None

            def schatz():
                self.effekt("twinkle", tx - 32, FUSS - 100, fps=10)
                self.effekt("coin", tx - 8, FUSS - 70, fps=10)
                art = random.choice(["orb", "orb", "cheese", "bread", "potion_red", "potion_blue", "potion_green"])
                bilder = (self.orbs[random.choice(list(self.orbs))] if art == "orb" else self.essen[art])
                self.beute.append([tx - seite * 170, bilder, self.jetzt])

            def zu():
                self.truhe_zu_seit = self.jetzt

            sh.planen([self.gehen_nach(ziel),
                       Schritt("drehen", beim_start=lambda: (hin(), setattr(sh, "richtung", seite))),
                       Schritt("truhe_rein", beim_start=oeffnen),
                       Schritt("truhe_raus", beim_start=schatz),
                       Schritt("lachen", dauer=1.2),
                       Schritt("stehen_a", dauer=0.4, beim_start=zu),
                       Schritt("stehen_a", dauer=0.6, beim_start=lambda: setattr(self, "truhe_besuch", False))], jetzt)
            return
        if stunde >= NACHT[0] or stunde < NACHT[1]:
            if jetzt < self.lager_ab:                        # erst durchatmen, nicht gleich ans Feuer
                sh.planen([Schritt("stehen_a", dauer=1.5)], jetzt)
                return
            if self.kugeln:                                  # erst lassen, was noch fliegt
                sh.planen([Schritt("stehen_a", dauer=0.5)], jetzt)
                return
            if jetzt > self.spaziergang_ab:                  # nachts: eine Runde durch die Halle, dann zurück ans Feuer
                self.spaziergang_ab = jetzt + random.uniform(20, 50)
                lo, hi = 120, HALLE - 120
                ziele = [random.uniform(lo, hi) for _ in range(2)]
                sh.planen([self.gehen_nach(z) for z in ziele] + [Schritt("stehen_a", dauer=1.0)], jetzt)
                return
            plan = [self.gehen_nach(LAGER_X)] if abs(sh.x - LAGER_X) > 8 else []
            sh.planen(plan + [Schritt("stehen_a", dauer=0.4),
                              Schritt("drehen", beim_start=lambda: setattr(sh, "richtung", 1)),
                              Schritt("lagerfeuer", dauer=60)], jetzt)
            return
        # ab und zu ein Spaziergang durch die Halle - die Kamera läuft mit
        if jetzt > self.spaziergang_ab:
            self.spaziergang_ab = jetzt + random.uniform(40, 100)
            ziel = random.uniform(120, HALLE - 120)
            while abs(ziel - sh.x) < 400:
                ziel = random.uniform(120, HALLE - 120)
            sh.planen([self.gehen_nach(ziel), Schritt("stehen_a", dauer=1.5)], jetzt)
            return
        auswahl = random.choices(
            ["stehen_a", "stehen_b", "gehen", "nachdenken", "verzaubern", "drehen"],
            weights=[4, 2, 3, 1, 0.5, 1])[0]
        if auswahl == "gehen":
            ziel = min(max(sh.x + random.uniform(-220, 220), 120), HALLE - 120)
            sh.planen([self.gehen_nach(ziel), Schritt("stehen_a", dauer=1.0)], jetzt)
        elif auswahl == "drehen":
            sh.planen([Schritt("drehen", beim_start=lambda: setattr(sh, "richtung", -sh.richtung)),
                       Schritt("stehen_a", dauer=2)], jetzt)
        elif auswahl == "verzaubern":
            sh.planen([Schritt("verzaubern"), Schritt("stehen_b", dauer=1.5)], jetzt)
        else:
            sh.planen([Schritt(auswahl, dauer=random.uniform(2.5, 7))], jetzt)

    # ---------- Zauberkugeln aus der Ferne ----------
    KUGEL_HOEHE = {"kopf": 122, "mitte": 88, "boden": 18}    # Flughöhe über der Standlinie (Kopf bleibt unter der Leiste)

    def kugeln_bewegen(self, dt):
        sh, jetzt = self.shaia, self.jetzt
        frei_fuer_kugel = (not self.shaia_offline and not self.gegner() and not self.kugeln
                           and sh.anim in ALLTAG and sh.anim != "lagerfeuer" and sh.hoehe == 0)
        if frei_fuer_kugel and jetzt > self.kugel_ab:
            self.kugel_ab = jetzt + random.uniform(35, 80)
            # meist von vorn (sie sieht sie kommen), manchmal von hinten (dann trifft sie)
            von_vorn = random.random() < 0.75
            von_links = (sh.richtung < 0) if von_vorn else (sh.richtung > 0)
            self.kugeln.append({"x": self.kamera - 40 if von_links else self.kamera + W + 40,
                                "art": random.choice(list(self.KUGEL_HOEHE)),
                                "richtung": 1 if von_links else -1, "reagiert": False,
                                "element": random.choice(list(self.kugel_bilder))})
        for k in list(self.kugeln):
            k["x"] += k["richtung"] * 260 * dt
            abstand = (sh.x - k["x"]) * k["richtung"]       # > 0: Kugel kommt noch auf Shaia zu
            sieht_sie = k["richtung"] == -sh.richtung            # Kugel fliegt auf ihr Gesicht zu
            if not k["reagiert"] and 0 < abstand < 170 and not self.shaia_offline and sieht_sie:
                k["reagiert"] = True
                self.ausweichen(k)
            elif not k["reagiert"] and 0 < abstand < 30 and not self.shaia_offline:
                # von hinten: sie hat sie nicht gesehen und wird getroffen
                ruecken = sh.x - sh.richtung * 22          # Kontaktpunkt am Rücken
                self.effekt("hit", ruecken - 64, FUSS - 64 - self.KUGEL_HOEHE[k["art"]], fps=18)
                sh.hoehe = sh.vy = sh.vx_luft = 0.0
                sh.planen([Schritt(random.choice(["treffer_kopf", "treffer_koerper"]), dauer=0.4,
                                   vx=50),            # die Kugel schiebt sie nach vorn
                           Schritt("drehen", beim_start=lambda: setattr(sh, "richtung", -sh.richtung)),
                           Schritt("stehen_b", dauer=1.0)], jetzt)
                self.kugeln.remove(k)
                continue
            if k["art"] == "mitte" and k["reagiert"] and 0 < abstand < 45:
                # am Schild zerplatzt
                self.effekt("guard", sh.x - 32 - k["richtung"] * 30, FUSS - 132, fps=16)
                self.effekt("hit", sh.x - 64 - k["richtung"] * 30, FUSS - 164, fps=18)
                self.kugeln.remove(k)
            elif not (self.kamera - 80 < k["x"] < self.kamera + W + 80):
                self.kugeln.remove(k)

    def ausweichen(self, k):
        """Kopfhöhe: ducken, Boden: springen, Mitte: mit dem Schild blocken."""
        sh, jetzt = self.shaia, self.jetzt
        if k["art"] == "kopf":
            sh.planen([Schritt("ducken_rein"), Schritt("ducken", dauer=0.55), Schritt("ducken_raus"),
                       Schritt("stehen_b", dauer=0.6)], jetzt)
        elif k["art"] == "boden":
            def hoch():
                sh.vy = 310
            sh.planen([Schritt("absprung", dauer=0.06), Schritt("steigen", beim_start=hoch,
                                                                fertig_wenn=lambda: sh.vy <= 0),
                       Schritt("fallen", fertig_wenn=lambda: sh.hoehe == 0), Schritt("landen"),
                       Schritt("stehen_b", dauer=0.5)], jetzt)
        else:
            sh.planen([Schritt("abwehr", dauer=0.7), Schritt("stehen_b", dauer=0.5)], jetzt)

    def kampfbereit(self, von_links):
        """Weg vom Rand, zu dem das Skelett hereinkommt, und in seine Richtung schauen."""
        sh = self.shaia
        k = self.kamera
        ziel = max(sh.x, k + 250) if von_links else min(sh.x, k + W - 250)
        blick = -1 if von_links else 1
        plan = [self.gehen_nach(ziel)] if abs(ziel - sh.x) > 8 else []
        plan += [Schritt("drehen", beim_start=lambda: setattr(sh, "richtung", blick)),
                 Schritt("stehen_b", fertig_wenn=lambda: not self.gegner() or self.gegner().zustand != "kommt")]
        self.im_kampf = True
        sh.planen(plan, self.jetzt)

    def shaia_planen(self, jetzt_dt):
        sh, jetzt = self.shaia, self.jetzt
        if not sh.frei:
            return
        if self.shaia_offline:
            # Stromschläge in Abständen, dazwischen steht sie - als Dauerzustand sonst zu grell
            sh.planen([Schritt("stehen_b", dauer=random.uniform(4, 7)), Schritt("strom", dauer=1.6)], jetzt)
            return
        g = self.gegner()
        if g and g.kampffaehig and g.zustand != "kommt":
            self.im_kampf = True
            richtung = 1 if g.x > sh.x else -1
            if richtung != sh.richtung:
                sh.planen([Schritt("drehen", beim_start=lambda: setattr(sh, "richtung", richtung))], jetzt)
            else:
                self.angriff_planen(g)
        elif g:
            sh.planen([Schritt("stehen_b", dauer=0.4)], jetzt)
        else:
            self.alltag_planen(jetzt_dt)

    def skelett_planen(self, s):
        sh, jetzt = self.shaia, self.jetzt
        if not s.frei or s.zustand == "tot":
            return
        if s.zustand in ("getroffen", "betaeubt"):
            s.zustand = "steht"
        dx = sh.x - s.x
        s.richtung = 1 if dx > 0 else -1
        if s.zustand == "kommt":
            if self.kamera + 90 <= s.x <= self.kamera + W - 90 or abs(dx) <= 125:
                s.zustand = "steht"
            else:
                s.planen([Schritt("gehen", vx=55, dauer=0.25)], jetzt)
                return
        if (s.springer and 140 < abs(dx) < 330 and jetzt > s.sprung_ab and not self.shaia_offline
                and sh.hoehe == 0 and random.random() < 0.5):
            self.skelett_springt(s)
            return
        if abs(dx) > 125:
            s.planen([Schritt("gehen", vx=48, dauer=0.25)], jetzt)
        elif jetzt > s.angriff_ab and random.random() < 0.55 and not self.shaia_offline:
            s.angriff_ab = jetzt + random.uniform(1.4, 3.0)
            s.planen([Schritt("angriff", treffer={3: Treffer(1, 150)})], jetzt)
        else:
            s.planen([Schritt("stehen", dauer=0.4)], jetzt)

    # ---------- ein Takt ----------
    def schritt(self, dt, jetzt_dt, jetzt, offline, getippt):
        self.jetzt = jetzt
        sh = self.shaia
        if offline and not self.shaia_offline:
            self.shaia_offline = True
            sh.hoehe = sh.vy = sh.vx_luft = 0.0
            sh.planen([Schritt("strom", dauer=2.0)], jetzt)
        elif not offline and self.shaia_offline:
            self.shaia_offline = False
            sh.planen([Schritt("aufstehen"), Schritt("stehen_a", dauer=1)], jetzt)

        # Gegner losschicken: für jede geblockte Anfrage eins, sonst ab und zu ein Bonus-Skelett
        if not offline and sh.anim != "lagerfeuer" and not self.gegner() and (self.wartend or jetzt > self.bonus_ab):
            if self.wartend:
                self.wartend -= 1
            else:
                self.bonus_ab = jetzt + random.uniform(60, 120)
            von_links = random.random() < 0.5
            start_x = self.kamera - 70 if von_links else self.kamera + W + 70
            self.skelette.append(Skelett(self.skelett_anims, start_x, 1 if von_links else -1))
            self.truhe_weg()
            if not self.shaia_offline and sh.anim in ALLTAG and sh.hoehe == 0:
                self.kampfbereit(von_links)

        if getippt and sh.hoehe == 0 and not self.im_kampf and sh.anim in ALLTAG and sh.anim != "lagerfeuer":
            def hoch():
                sh.vy = 330
            sh.planen([Schritt("absprung", dauer=0.08),
                       Schritt("steigen", beim_start=hoch, fertig_wenn=lambda: sh.vy <= 0),
                       Schritt("fallen", fertig_wenn=lambda: sh.hoehe == 0),
                       Schritt("landen")], jetzt)

        if not self.gegner():
            if self.im_kampf:
                self.lager_ab = jetzt + 8.0
            self.im_kampf = False
        self.shaia_planen(jetzt_dt)

        for t in sh.tick(dt, jetzt):
            ziel = self.gegner()
            if ziel:
                self.treffe(ziel, t)
        sh.x = min(max(sh.x, 50), HALLE - 50)
        self.shaia_planen(jetzt_dt)

        # Skelette
        for s in self.skelette:
            if s.zustand == "tot":
                for t in s.teile:
                    t[0] += t[2] * dt
                    t[1] += t[3] * dt
                    t[3] += 700 * dt
                    if t[1] > FUSS - 8:
                        t[1], t[3], t[2] = FUSS - 8, -t[3] * 0.3, t[2] * 0.6
                continue
            self.skelett_planen(s)
            for _ in s.tick(dt, jetzt):
                self.skelett_schlaegt(s)
            if s.zustand != "kommt":
                s.x = min(max(s.x, 50), HALLE - 50)      # Hallenwände
            self.skelett_planen(s)
        self.skelette = [s for s in self.skelette if not (s.tot_seit and jetzt - s.tot_seit > 1.8)]
        self.kugeln_bewegen(dt)
        for g in list(self.geschosse):
            x, y, ziel, element, _ = g
            richtung = 1 if ziel.x > x else -1
            g[0] = x + richtung * 420 * dt
            g[1] = y + (FUSS - 100 - y) * min(1.0, 3 * dt)
            if abs(ziel.x - g[0]) < 30 or not ziel.kampffaehig:
                self.geschosse.remove(g)
                if ziel.kampffaehig:
                    self.treffe(ziel, Treffer(2, 10000, element))
        self.kamera_fuehren(dt)
        self.effekte = [e for e in self.effekte if (jetzt - e[3]) * e[4] < len(e[0])]

    def kamera_fuehren(self, dt):
        """Im Kampf beide im Bild halten, sonst nur nachziehen, wenn Shaia dem Rand nahe kommt.
        Die Kamera beschleunigt und bremst sanft - gleichmäßige Schritte ruckeln weniger."""
        sh, k = self.shaia, self.kamera
        g = self.gegner()
        if g and self.im_kampf and g.zustand != "kommt":
            ziel = (sh.x + g.x) / 2 - W / 2
        elif sh.anim == "lagerfeuer":
            ziel = sh.x + 65 * sh.richtung - W / 2          # Shaia, Feuer und Hähnchen zusammen
        elif self.truhe_besuch:
            ziel = (sh.x + self.truhe_x) / 2 - W / 2
        elif sh.x - k < RAND:
            ziel = sh.x - RAND
        elif sh.x - k > W - RAND:
            ziel = sh.x - (W - RAND)
        else:
            ziel = k
        ziel = min(max(ziel, 0), HALLE - W)
        # Wunschtempo wächst mit dem Abstand, Tempoänderung ist begrenzt
        wunsch = max(-KAMERA_TEMPO, min(KAMERA_TEMPO, (ziel - k) * 2.5))
        aenderung = KAMERA_BESCHL * dt
        self.kamera_v += max(-aenderung, min(aenderung, wunsch - self.kamera_v))
        self.kamera = min(max(k + self.kamera_v * dt, 0), HALLE - W)
        if self.kamera in (0, HALLE - W):
            self.kamera_v = 0.0

    # ---------- zeichnen ----------
    def zeichnen(self, jetzt):
        k = int(round(self.kamera))
        bild = self.halle.crop((k, 0, k + W, H))
        if self.nacht:
            bild.alpha_composite(self.nacht_schicht)
        for kugel in self.kugeln:
            reihe = self.kugel_bilder[kugel["element"]]
            b = reihe[int(jetzt * 14) % len(reihe)]
            einfuegen(bild, b, int(kugel["x"] - b.width / 2) - k,
                      int(FUSS - self.KUGEL_HOEHE[kugel["art"]] - b.height / 2))
        for x, bilder, seit in self.beute:
            b = bilder[int(jetzt * 10) % len(bilder)]
            phase = (jetzt - seit) % 1.6                    # kurzes Funkeln alle 1,6 s
            if phase < 0.75:
                fk = self.funkeln_klein[int(phase * 8) % len(self.funkeln_klein)]
                einfuegen(bild, fk, int(x + 6) - k, FUSS - b.height - 14)
            schweben = int(3 * np.sin((jetzt - seit) * 3)) if len(bilder) > 1 else 0
            einfuegen(bild, b, int(x - b.width / 2) - k, FUSS - b.height - 2 - schweben)
        n = len(self.truhe)
        if self.truhe_zu_seit is not None:                      # schließt sich rückwärts
            i = n - 1 - int((jetzt - self.truhe_zu_seit) * 6)
            if i <= 0:
                i, self.truhe_seit, self.truhe_zu_seit = 0, None, None
        else:
            i = 0 if self.truhe_seit is None else min(int((jetzt - self.truhe_seit) * 6), n - 1)
        einfuegen(bild, self.truhe[i], int(self.truhe_x - 64) - k, FUSS - 110)
        sh = self.shaia
        if self.nacht:                                       # festes Lager: Feuer brennt die ganze Nacht
            f = self.feuer[int(jetzt * 10) % len(self.feuer)]
            einfuegen(bild, f, LAGER_X + 70 - 32 - k, FUSS - 50)
        for s in self.skelette:
            if s.zustand != "tot":
                b, x, y = s.bild(jetzt)
                einfuegen(bild, b, x - k, y)
                if s.zustand == "betaeubt":
                    stern = self.vfx["stun"][int(jetzt * 12) % len(self.vfx["stun"])]
                    einfuegen(bild, stern, int(s.x - 32) - k, FUSS - 150)
            for x, y, _, _, reihe in s.teile:
                stueck = reihe[int((jetzt - s.tot_seit) * 16) % len(reihe)].bild
                einfuegen(bild, stueck, int(x - stueck.width / 2) - k, int(y - stueck.height / 2))
        b, x, y = sh.bild(jetzt)
        einfuegen(bild, b, x - k, y)
        for x, y, _, _, bilder in self.geschosse:
            o = bilder[int(jetzt * 14) % len(bilder)]
            einfuegen(bild, o, int(x - o.width / 2) - k, int(y - o.height / 2))
        for reihe, ex, ey, start, fps in self.effekte:
            einfuegen(bild, reihe[min(int((jetzt - start) * fps), len(reihe) - 1)], int(ex) - k, int(ey))
        bild.alpha_composite(self.statisch)
        return bild


def einfuegen(ziel, bild, x, y):
    """alpha_composite, das auch mit teilweise außerhalb liegenden Bildern zurechtkommt."""
    links, oben = max(0, -x), max(0, -y)
    rechts, unten = min(bild.width, W - x), min(bild.height, H - y)
    if rechts <= links or unten <= oben:
        return
    if links or oben or rechts < bild.width or unten < bild.height:
        bild = bild.crop((links, oben, rechts, unten))
    ziel.alpha_composite(bild, (x + links, y + oben))


class Ausgabe:
    """Bringt Bilder aufs Display. Bevorzugt direkt über DRM/KMS (card2, Treiber ili9486):
    jede Übertragung wird sofort und im eigenen Takt ausgelöst. Ohne pykms oder ohne Zugriff
    geht es über /dev/fb0, dort überträgt der Treiber in seinem eigenen Rhythmus (ruckeliger)."""

    def __init__(self):
        self.kms = None
        try:
            import pykms
            karte = next(f"/dev/dri/{n}" for n in sorted(os.listdir("/sys/class/drm"))
                         if n.startswith("card") and "-" not in n
                         and os.path.exists(f"/sys/class/drm/{n}/device/driver")
                         and os.path.basename(os.readlink(f"/sys/class/drm/{n}/device/driver")) == "ili9486")
            card = pykms.Card(karte)
            res = pykms.ResourceManager(card)
            conn = res.reserve_connector()
            crtc = res.reserve_crtc(conn)
            mode = conn.get_default_mode()
            fb = pykms.DumbFramebuffer(card, W, H, "RG16")
            if crtc.set_mode(conn, fb, mode) != 0:
                raise OSError("set_mode fehlgeschlagen")
            self.kms = (card, fb)
            self.pixel = np.asarray(fb.map(0)).view("<u2")
        except Exception as fehler:
            print(f"KMS nicht verfügbar ({fehler}), nehme /dev/fb0", flush=True)
            self.fd = os.open(FB, os.O_RDWR)

    def zeigen(self, neu, oben, unten):
        if self.kms:
            self.pixel[oben:unten] = neu[oben:unten]
            self.kms[1].flush(0, int(oben), W, int(unten - oben))   # wartet, bis übertragen ist
            return 0.0
        os.lseek(self.fd, int(oben) * W * 2, os.SEEK_SET)
        os.write(self.fd, neu[oben:unten].tobytes())
        return (unten - oben) * W * 16 / SPI_HZ * 1.15          # geschätzte Übertragungszeit


def nach_rgb565(bild):
    a = np.asarray(bild.convert("RGB"), dtype=np.uint16)
    return (((a[..., 0] >> 3) << 11) | ((a[..., 1] >> 2) << 5) | (a[..., 2] >> 3)).astype("<u2")


def main():
    szene = Szene()
    tippen = Beruehrung()
    wetter, wetter_zeit = None, 0.0
    pihole, pihole_zeit = None, 0.0
    netz, netz_zeit, offline_seit = None, 0.0, None
    vorher = None
    ausgabe = Ausgabe()
    stil, stil_zeit = stil_lesen(), 0.0
    letzt = time.monotonic()
    while True:
        start = time.monotonic()
        jetzt_dt = datetime.now()
        dt = min(start - letzt, 0.2)
        letzt = start

        if not wetter_zeit or start - wetter_zeit >= daten.WETTER_ALLE:
            try:
                wetter, wetter_zeit = daten.wetter_holen(), start
            except Exception:
                wetter_zeit = start - daten.WETTER_ALLE + 120
        if not pihole_zeit or start - pihole_zeit >= daten.PIHOLE_ALLE:
            try:
                neu = daten.pihole_heute()
                if pihole is not None and neu[1] > pihole[1]:
                    szene.skelette_dazu(neu[1] - pihole[1])
                pihole = neu
            except Exception:
                pass
            pihole_zeit = start
        if not netz_zeit or start - netz_zeit >= daten.NETZ_ALLE:
            ms = daten.online_pruefen()
            offline_seit = None if ms is not None else (offline_seit or jetzt_dt)
            netz, netz_zeit = (ms, offline_seit, daten.speed_lesen()), start

        offline = netz is not None and netz[0] is None
        stunde = jetzt_dt.hour
        szene.nacht = (bool(wetter) and not wetter["current"].get("is_day", 1)) or \
            stunde >= NACHT[0] or stunde < NACHT[1]
        szene.schritt(dt, jetzt_dt, start, offline, tippen.angetippt())

        if start - stil_zeit >= 2:
            stil, stil_zeit = stil_lesen(), start
        seite = int(start // SEITE_SEKUNDEN)            # Wechselzeile unten blättert weiter
        schluessel = (jetzt_dt.strftime("%Y%m%d%H%M"), id(wetter), pihole, stil, seite,
                      None if netz is None else (netz[0] is None, netz[1], str(netz[2])))
        if schluessel != szene.statisch_schluessel:
            szene.statisch = szene.oberflaeche(jetzt_dt, wetter, pihole, netz, seite)
            szene.statisch_schluessel = schluessel

        neu = nach_rgb565(szene.zeichnen(start))
        zeilen = (np.array([0, H - 1]) if vorher is None
                  else np.flatnonzero((neu != vorher).any(axis=1)))
        uebertragung = 0.0
        if zeilen.size:
            uebertragung = ausgabe.zeigen(neu, zeilen[0], zeilen[-1] + 1)
        vorher = neu

        # nicht schneller liefern, als die Leitung überträgt - sonst stauen sich Bilder und es ruckelt
        rest = max(1 / FPS, uebertragung) - (time.monotonic() - start)
        if rest > 0:
            time.sleep(rest)


def grafiken_fehlen():
    return [n for n in BENOETIGT if not os.path.isdir(os.path.join(SPRITES, n))]


if __name__ == "__main__":
    os.nice(5)
    fehlt = grafiken_fehlen()
    if fehlt:
        print(f"Shaia-Grafiken fehlen in {SPRITES} (Ordner: {', '.join(fehlt)}).\n"
              f"Bitte bei picopico256 laden: {SHAIA_URL}\n"
              f"und den Ordner 'sprites' des Pakets nach {SPRITES} kopieren "
              f"(oder PI_ASSETS auf den Assets-Ordner setzen). Die Grafiken sind nicht Teil dieses Repos.\n"
              f"Bis dahin läuft die einfache Text-Anzeige ohne Figuren.", file=sys.stderr, flush=True)
        daten.main()
    else:
        main()
