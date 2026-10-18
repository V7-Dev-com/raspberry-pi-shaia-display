#!/usr/bin/env python3
"""Rendert Kampfszenen der Display-Anzeige als PNG: sucht in vielen Zufallsläufen je einen typischen Moment.

Auf dem Pi:  PI_SPRACHE=en sudo -E -u anzeige python3 kampffotos.py /tmp/kampf
"""
import os
import random
import sys

sys.argv = [sys.argv[0], sys.argv[1] if len(sys.argv) > 1 else "/tmp/kampf"]
src = open(os.path.join(os.path.dirname(os.path.abspath(__file__)), "bildschirmfotos.py")).read()
exec(src.split("sz = neu(); lauf(sz, 3)")[0])      # Einrichtung und Hilfsfunktionen übernehmen

# Name -> Bedingung auf (Szene, Gegner)
GESUCHT = {
    "1_kombo": lambda sz, g: sz.shaia.anim.startswith("kombo") and sz.jetzt - sz.shaia.seit > 0.15,
    "2_kreisschlag": lambda sz, g: sz.shaia.anim == "kreisschlag" and sz.jetzt - sz.shaia.seit > 0.2,
    "3_tritt": lambda sz, g: sz.shaia.anim == "tritt" and sz.jetzt - sz.shaia.seit > 0.05,
    "4_skelett_springt": lambda sz, g: g and g.anim in ("sprung_hoch", "sprung_runter") and g.hoehe > 25,
    "5_skelett_landet": lambda sz, g: g and g.anim == "landung" and sz.jetzt - g.seit > 0.05,
    "6_abwehr": lambda sz, g: sz.shaia.anim == "abwehr" and sz.jetzt - sz.shaia.seit > 0.1,
    "7_umgeworfen": lambda sz, g: sz.shaia.anim == "umgeworfen" and sz.jetzt - sz.shaia.seit > 0.25,
    "8_skelett_zerfaellt": lambda sz, g: any(s.zustand == "tot" and s.tot_seit and sz.jetzt - s.tot_seit > 0.25
                                             for s in sz.skelette),
    "9_zauber": lambda sz, g: sz.shaia.anim in ("zauber", "verzaubern", "aufwaerts", "ultimate")
                              and sz.jetzt - sz.shaia.seit > 0.3,
}
offen = dict(GESUCHT)
for seed in range(1, 200):
    if not offen:
        break
    random.seed(seed)
    sz = neu()
    sz.skelette_dazu(3)
    t = sz.jetzt
    for _ in range(15 * 90):
        sz.schritt(1 / 15, TAG, t, False, False)
        t += 1 / 15
        sz.jetzt = t
        for s in sz.skelette:
            s.springer = True
        g = next((s for s in sz.skelette if s.zustand != "tot"), None)
        for name in list(offen):
            if offen[name](sz, g):
                foto(sz, name + ".png")
                del offen[name]
        if not offen:
            break
print("fertig:", sorted(os.listdir(ZIEL)), "offen:", sorted(offen), file=sys.stderr)
