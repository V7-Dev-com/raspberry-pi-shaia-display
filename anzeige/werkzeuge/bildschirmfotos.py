#!/usr/bin/env python3
"""Rendert typische Szenen der Display-Anzeige als PNG (ohne Display), z. B. für Reviews.

Auf dem Pi ausführen:  sudo -u anzeige python3 bildschirmfotos.py /tmp/codexbilder [stil]
"""
import copy
import os
import random
import sys
from datetime import datetime

sys.path.insert(0, "/opt/pi-anzeige")
import anzeige as d  # noqa: E402
import hud  # noqa: E402
import ruinrunner as rr  # noqa: E402

ZIEL = sys.argv[1] if len(sys.argv) > 1 else "/tmp/codexbilder"
STIL = sys.argv[2] if len(sys.argv) > 2 else "glut"
os.makedirs(ZIEL, exist_ok=True)
random.seed(5)
try:
    WETTER = d.wetter_holen()
except Exception:
    WETTER = None
PIHOLE, SPEED = d.pihole_heute(), d.speed_lesen()
TAG, NACHT = datetime(2026, 10, 18, 14, 30), datetime(2026, 10, 18, 23, 10)


def oberflaeche(sz, wetter=WETTER, netz=(5.0, None, SPEED), zeit=datetime(2026, 10, 18, 18, 42), seite=0):
    sz.statisch, _ = hud.zeichnen(STIL, rr.W, rr.H, rr.KOPF, rr.FUSS, zeit, wetter, PIHOLE, netz,
                                  rr.warnungen(wetter), rr.WETTER_WORT, d.cpu_temperatur, rr.laufzeit, seite)


def lauf(sz, sekunden, bis=None, zeit=TAG, offline=False):
    t = sz.jetzt
    for _ in range(int(sekunden * 15)):
        sz.schritt(1 / 15, zeit, t, offline, False)
        t += 1 / 15
        if bis and bis(sz):
            return True
    return False


def neu():
    sz = rr.Szene()
    sz.jetzt = 5000.0
    sz.bonus_ab = sz.spaziergang_ab = sz.kugel_ab = sz.truhe_ab = 10 ** 9
    sz.truhe_fuer_stunde = TAG.hour
    oberflaeche(sz)
    return sz


def foto(sz, name):
    sz.zeichnen(sz.jetzt).convert("RGB").save(os.path.join(ZIEL, name))


sz = neu(); lauf(sz, 3); foto(sz, "1_ruhe.png")
sz = neu(); sz.skelette_dazu(1)
lauf(sz, 40, lambda s: s.gegner() and s.gegner().zustand == "getroffen"); foto(sz, "2_kampf.png")
random.seed(1); sz = neu(); sz.kugel_ab = 0
lauf(sz, 90, lambda s: s.shaia.anim == "ducken"); foto(sz, "3_ausweichen.png")
for seed in range(2, 40):
    random.seed(seed); sz = neu(); sz.kugel_ab = 0
    if lauf(sz, 240, lambda s: s.shaia.anim in ("treffer_kopf", "treffer_koerper")):
        break
foto(sz, "4_treffer_von_hinten.png")
sz = neu(); sz.beute.append([sz.shaia.x + 120, sz.orbs["ice"], sz.jetzt]); foto(sz, "5_beute.png")
sz = neu(); sz.nacht = True; oberflaeche(sz, zeit=NACHT, seite=3)
sz.truhe_ab = 10 ** 9; sz.truhe_fuer_stunde = NACHT.hour
sz.shaia.x = rr.LAGER_X; sz.shaia.planen([], sz.jetzt); sz.kamera = rr.LAGER_X + 65 - rr.W / 2
lauf(sz, 8, lambda s: s.shaia.anim == "lagerfeuer", zeit=NACHT); lauf(sz, 2, zeit=NACHT); foto(sz, "6_nacht_lagerfeuer.png")
sz = neu(); oberflaeche(sz, netz=(None, datetime(2026, 10, 18, 18, 31), None)); lauf(sz, 1, offline=True)
foto(sz, "7_offline.png")
if WETTER:
    unwetter = copy.deepcopy(WETTER)
    unwetter["current"].update(weather_code=95, wind_gusts_10m=62, uv_index=9, is_day=1)
    sz = neu(); oberflaeche(sz, wetter=unwetter); foto(sz, "8_warnungen.png")
random.seed(3); sz = neu(); sz.truhe_ab = sz.jetzt + 1
t = sz.jetzt
for _ in range(15 * 90):
    sz.schritt(1 / 15, TAG, t, False, False); t += 1 / 15; sz.jetzt = t
    if sz.shaia.anim == "truhe_raus" and t - sz.truhe_seit > 0.6:
        foto(sz, "9_truhe.png"); break
print("fertig:", sorted(os.listdir(ZIEL)), file=sys.stderr)
