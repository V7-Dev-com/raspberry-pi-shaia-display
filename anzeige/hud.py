"""Text-Oberfläche (HUD) für die Display-Szene in austauschbaren Stilen.

Stile:
  v7     - "V7 Neon": Saira Condensed, leuchtendes V7-Orange #FF6F09 mit Glühen, JetBrains-Mono-Labels
  glut   - "Dungeon Glut": Rajdhani, Gold/Bernstein mit Glut-Orange, Zierlinien mit Raute
  retro  - "Retro-HUD": Pixel-Schrift Silkscreen in Orange, harte Schatten
  klar   - der bisherige schlichte Stil mit DejaVu

Schriften: Google Fonts (OFL) in /opt/pi-anzeige/assets/fonts. Zeichen, die einer Schrift fehlen
(Pfeile, Punkte, Warnsymbole), zeichnet automatisch DejaVu Sans Bold.
"""
import os
import re
from datetime import datetime

from PIL import Image, ImageDraw, ImageFilter, ImageFont

FONTS = os.path.join(os.environ.get("PI_ASSETS", "/opt/pi-anzeige/assets"), "fonts")
DEJAVU = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"
DEJAVU_NORMAL = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"

# Sprache: Umgebungsvariable PI_SPRACHE=en schaltet die Anzeige auf Englisch (Standard: Deutsch)
ENGLISCH = os.environ.get("PI_SPRACHE", "de").lower().startswith("en")
EN_WOERTER = {
    "sehr hoch": "very high", "hoch": "high", "luftfeuchte": "humidity", "gefühlt": "feels", "geblockt": "blocked",
    "läuft": "uptime", "offline seit": "OFFLINE since", "sturmböen": "gusts", "sturm": "storm", "hitze": "heat",
    "starkregen": "heavy rain", "eisregen": "freezing rain", "regen": "rain", "klar": "clear", "heiter": "fair",
    "wolkig": "cloudy", "bedeckt": "overcast", "nebel": "fog", "niesel": "drizzle", "schnee": "snow",
    "schauer": "showers", "gewitter": "thunderstorm", "anfr.": "req.", "heute": "today",
}
_EN_MUSTER = re.compile(r"\b(" + "|".join(sorted(map(re.escape, EN_WOERTER), key=len, reverse=True)) + r")(?!\w)",
                        re.IGNORECASE)


def tr(text):
    """Übersetzt Anzeigetexte ins Englische, wenn PI_SPRACHE=en gesetzt ist; sonst unverändert."""
    if not ENGLISCH:
        return text

    def wort(m):
        en = EN_WOERTER[m.group(1).lower()]
        if any(z.isupper() for z in en):
            return en
        return en.upper() if m.group(1).isupper() else (en.capitalize() if m.group(1)[0].isupper() else en)
    return _EN_MUSTER.sub(wort, text).replace(" T ", " d ")


WOCHENTAGE = ["Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag", "Sonntag"]
MONATE = ["Januar", "Februar", "März", "April", "Mai", "Juni", "Juli",
          "August", "September", "Oktober", "November", "Dezember"]
if ENGLISCH:
    WOCHENTAGE = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"]
    MONATE = ["January", "February", "March", "April", "May", "June", "July",
              "August", "September", "October", "November", "December"]
WARN_GELB, WARN_ROT = (225, 145, 20), (205, 40, 40)


def _f(datei):
    return datei if datei.startswith("/") else os.path.join(FONTS, datei)


STILE = {
    "v7": {
        "name": "V7 Neon",
        "uhr": ("SairaCondensed-ExtraBold.ttf", 68), "uhr_farbe": (255, 111, 9), "glut": (255, 111, 9),
        "datum": ("SairaCondensed-Bold.ttf", 21), "datum_farbe": (255, 255, 255), "gross_datum": True,
        "wert": ("SairaCondensed-ExtraBold.ttf", 36), "wert_farbe": (255, 255, 255),
        "label": ("JetBrainsMono[wght].ttf", 12), "label_farbe": (255, 111, 9), "label_gross": True,
        "label_gewicht": 800, "sperren": 1,
        "zeile": ("SairaCondensed-Bold.ttf", 19), "zeile_farbe": (255, 255, 255),
        "zeile2_farbe": (175, 180, 190), "gruen": (255, 140, 40), "rot": (255, 59, 48),
        "leiste_oben": ((10, 10, 13), (18, 19, 24)), "leiste_unten": (10, 10, 13),
        "linie": (255, 111, 9), "deko": "v7", "kontur": 0, "schatten": None,
    },
    "glut": {
        "name": "Dungeon Glut",
        "uhr": ("Rajdhani-Bold.ttf", 76), "uhr_farbe": (255, 205, 117), "glut": (239, 125, 87),
        "datum": ("Rajdhani-SemiBold.ttf", 23), "datum_farbe": (245, 230, 200), "gross_datum": False,
        "wert": ("Rajdhani-Bold.ttf", 38), "wert_farbe": (255, 226, 168),
        "label": ("Rajdhani-SemiBold.ttf", 17), "label_farbe": (243, 191, 122), "label_gross": False,
        "label_gewicht": None, "sperren": 0,
        "zeile": ("Rajdhani-SemiBold.ttf", 21), "zeile_farbe": (245, 230, 200),
        "zeile2_farbe": (214, 190, 150), "gruen": (167, 240, 112), "rot": (255, 90, 70),
        "leiste_oben": ((14, 8, 5), (30, 16, 9)), "leiste_unten": (16, 9, 6),
        "linie": (169, 120, 52), "deko": "glut", "kontur": 0, "schatten": (0, 0, 0),
    },
    "retro": {
        "name": "Retro-HUD",
        "uhr": ("Silkscreen-Bold.ttf", 48), "uhr_farbe": (255, 138, 30), "glut": None,
        "datum": ("Silkscreen-Regular.ttf", 16), "datum_farbe": (255, 255, 255), "gross_datum": True,
        "wert": ("Silkscreen-Bold.ttf", 32), "wert_farbe": (255, 255, 255),
        "label": ("Silkscreen-Regular.ttf", 16), "label_farbe": (255, 138, 30), "label_gross": True,
        "label_gewicht": None, "sperren": 0,
        "zeile": ("Silkscreen-Regular.ttf", 16), "zeile_farbe": (255, 255, 255),
        "zeile2_farbe": (200, 205, 220), "gruen": (167, 240, 112), "rot": (255, 80, 80),
        "leiste_oben": ((17, 19, 31), (17, 19, 31)), "leiste_unten": (17, 19, 31),
        "linie": (255, 138, 30), "deko": "retro", "kontur": 0, "schatten": (60, 20, 0),
    },
    "klar": {
        "name": "Klar (bisher)",
        "uhr": (DEJAVU, 64), "uhr_farbe": (255, 255, 255), "glut": None,
        "datum": (DEJAVU, 20), "datum_farbe": (255, 255, 255), "gross_datum": False,
        "wert": (DEJAVU, 32), "wert_farbe": (255, 255, 255),
        "label": (DEJAVU, 16), "label_farbe": (228, 234, 245), "label_gross": False,
        "label_gewicht": None, "sperren": 0,
        "zeile": (DEJAVU, 17), "zeile_farbe": (255, 255, 255),
        "zeile2_farbe": (228, 234, 245), "gruen": (120, 255, 140), "rot": (255, 110, 110),
        "leiste_oben": ((2, 3, 8), (6, 9, 22)), "leiste_unten": (3, 5, 12),
        "linie": (60, 110, 200), "deko": "klar", "kontur": 1, "schatten": None,
    },
}


# --- Schriften mit Ersatz für fehlende Zeichen ----------------------------------------------
_schriften, _zeichen = {}, {}


def schrift(datei, groesse, gewicht=None):
    schluessel = (datei, groesse, gewicht)
    if schluessel not in _schriften:
        try:
            f = ImageFont.truetype(_f(datei), groesse)
        except OSError:               # Schrift fehlt (nicht im Repo): Ersatzschrift
            f = ImageFont.truetype(DEJAVU, groesse)
        if gewicht:
            try:
                f.set_variation_by_axes([gewicht])
            except Exception:
                pass
        _schriften[schluessel] = f
    return _schriften[schluessel]


ERSATZ_IMMER = {"Silkscreen-Bold.ttf": "°%", "Silkscreen-Regular.ttf": "°%"}


def _hat(datei, z):
    if z in ERSATZ_IMMER.get(datei, ""):
        return False
    if datei not in _zeichen:
        try:
            from fontTools.ttLib import TTFont
            _zeichen[datei] = set(TTFont(_f(datei)).getBestCmap())
        except Exception:
            _zeichen[datei] = None          # ohne fontTools: alles der Schrift selbst überlassen
    menge = _zeichen[datei]
    return menge is None or ord(z) in menge


def _laeufe(text, datei):
    """Text in Abschnitte zerlegen: (Teiltext, nutzt_Ersatzschrift)."""
    laeufe = []
    for z in text:
        ersatz = not _hat(datei, z)
        if laeufe and laeufe[-1][1] == ersatz:
            laeufe[-1][0] += z
        else:
            laeufe.append([z, ersatz])
    return laeufe


def breite(d, text, spec, gewicht=None, sperren=0):
    datei, groesse = spec
    b = 0.0
    for teil, ersatz in _laeufe(text, datei):
        f = schrift(DEJAVU, int(groesse * 0.8)) if ersatz else schrift(datei, groesse, gewicht)
        b += d.textlength(teil, font=f) + sperren * len(teil)
    return b


def schreiben(d, xy, text, spec, farbe, anker="la", gewicht=None, sperren=0, kontur=0,
              kontur_farbe=(5, 8, 20), schatten=None, max_breite=None, grundlinie=False):
    """Text mit Ersatzschrift, optionaler Sperrung, Kontur und hartem Schatten.
    anker: la (links), ma (Mitte), ra (rechts) - jeweils Oberkante."""
    datei, groesse = spec
    while max_breite and groesse > 8 and breite(d, text, (datei, groesse), gewicht, sperren) > max_breite:
        groesse -= 1
    spec = (datei, groesse)
    gesamt = breite(d, text, spec, gewicht, sperren)
    x, y = xy
    if anker[0] == "m":
        x -= gesamt / 2
    elif anker[0] == "r":
        x -= gesamt
    basis = schrift(datei, groesse, gewicht).getmetrics()[0]
    if grundlinie:          # y ist die Grundlinie statt der Oberkante
        y -= basis
    for teil, ersatz in _laeufe(text, datei):
        f = schrift(DEJAVU, int(groesse * 0.8)) if ersatz else schrift(datei, groesse, gewicht)
        dy = basis - f.getmetrics()[0] if ersatz else 0
        for stueck in (teil if sperren else [teil]):
            if schatten:
                d.text((x + 2, y + dy + 2), stueck, font=f, fill=schatten)
            d.text((x, y + dy), stueck, font=f, fill=farbe, stroke_width=kontur, stroke_fill=kontur_farbe)
            x += d.textlength(stueck, font=f) + (sperren if sperren else 0)


def gluehen(bild, xy, text, spec, farbe, glut, anker="ma", radius=7, staerke=2):
    """Leuchtschrift: weich verwischte Kopie in der Glutfarbe unter dem eigentlichen Text."""
    schicht = Image.new("RGBA", bild.size, (0, 0, 0, 0))
    schreiben(ImageDraw.Draw(schicht), xy, text, spec, glut + (255,), anker=anker)
    weich = schicht.filter(ImageFilter.GaussianBlur(radius))
    for _ in range(staerke):
        bild.alpha_composite(weich)
    schreiben(ImageDraw.Draw(bild), xy, text, spec, farbe, anker=anker)


# --- Zierlinien ------------------------------------------------------------------------------
def deko_linie(d, y, stil, W, oben=True):
    art, farbe = stil["deko"], stil["linie"]
    if art == "v7":
        # durchgehende Linie mit abgeschrägten Akzent-Blöcken an den Enden (V7-Signatur)
        d.line((0, y, W, y), fill=farbe, width=2)
        for x0, richtung in ((0, 1), (W, -1)):
            d.polygon([(x0, y - 5), (x0 + richtung * 70, y - 5), (x0 + richtung * 78, y + 1),
                       (x0, y + 1)], fill=farbe)
    elif art == "glut":
        gold = farbe
        d.line((0, y - 2, W, y - 2), fill=gold, width=1)
        d.line((0, y + 1, W, y + 1), fill=(120, 70, 30), width=1)
        mx = W // 2
        d.polygon([(mx, y - 3), (mx + 4, y), (mx, y + 3), (mx - 4, y)], fill=(239, 125, 87), outline=gold)
    elif art == "retro":
        for x in range(0, W, 8):
            d.rectangle((x, y - 2, x + 4, y + 1), fill=farbe)
    else:
        d.line((0, y, W, y), fill=farbe, width=2)


# --- die ganze Oberfläche ----------------------------------------------------------------------
def grad(d, x_rechts_der_zahl, kappe_oben, kappe_px, farbe):
    """Gradzeichen als sauberer Ring, ca. 28 % der Versalhöhe, 3 px neben der Zahl."""
    r = max(4, round(kappe_px * 0.14))
    x0, y0 = x_rechts_der_zahl + 3, kappe_oben
    d.ellipse((x0, y0, x0 + 2 * r, y0 + 2 * r), outline=farbe, width=max(2, r // 2))
    return x0 + 2 * r


def kappen_hoehe(spec, gewicht=None):
    """Höhe eines Großbuchstabens in Pixeln und sein Abstand zur Text-Oberkante."""
    f = schrift(spec[0], spec[1], gewicht)
    box = f.getbbox("H")
    return box[3] - box[1], box[1]


def groesse_fuer(datei, kappe_px, gewicht=None):
    """Schriftgröße, bei der Großbuchstaben kappe_px hoch sind."""
    h, _ = kappen_hoehe((datei, 100), gewicht)
    return max(8, round(100 * kappe_px / h))


def setzen(d, x, kappe_oben, text, datei, kappe_px, farbe, anker="la", max_breite=None, **art):
    """Text so setzen, dass die Oberkante der Großbuchstaben bei kappe_oben liegt."""
    gr = groesse_fuer(datei, kappe_px, art.get("gewicht"))
    while max_breite and gr > 8 and breite(d, text, (datei, gr), art.get("gewicht"), art.get("sperren", 0)) > max_breite:
        gr -= 1
    _, versatz = kappen_hoehe((datei, gr), art.get("gewicht"))
    schreiben(d, (x, kappe_oben - versatz), text, (datei, gr), farbe, anker=anker, **art)
    return gr


def laufschrift(stil, jetzt, wetter, pihole, netz, warnungen, wetter_wort, cpu_temperatur, laufzeit):
    """Einträge der großen Wechselzeile unten: (Text, Farbe). Kurz halten - sie sollen groß bleiben.
    Offline verdrängt alles."""
    ms, offline_seit, speed = netz if netz else (None, None, None)
    if netz and ms is None and offline_seit:
        return [(tr(f"OFFLINE seit {offline_seit:%H:%M}"), stil["rot"])]
    gross = (lambda t: t.upper()) if stil["gross_datum"] else (lambda t: t)
    eintraege = [(gross(t.lstrip("⚠☀⚡❄ ")), stil["rot"] if f == WARN_ROT else (255, 190, 60))
                 for t, f in warnungen]
    if wetter:
        c, t = wetter["current"], wetter["daily"]
        wort = wetter_wort.get(c["weather_code"], "Wetter").capitalize()
        eintraege.append((gross(f"{wort} {round(t['temperature_2m_min'][0])}–{round(t['temperature_2m_max'][0])}°"),
                          stil["zeile_farbe"]))
        regen = t["precipitation_probability_max"][0]
        if regen and regen >= 20:
            eintraege.append((gross(f"Regen {regen} %"), stil["zeile_farbe"]))
        eintraege.append((gross(f"Gefühlt {round(c['apparent_temperature'])}° · Wind {round(c['wind_speed_10m'])}"),
                          stil["zeile_farbe"]))
        if c.get("is_day", 1) and c.get("uv_index") is not None and c["uv_index"] >= 1:
            eintraege.append((gross(f"UV-Index {c['uv_index']:.0f}"), stil["zeile_farbe"]))
    if ms is not None:
        eintraege.append((gross(f"Ping {ms:.0f} ms"), stil["gruen"]))
        if speed and "down_mbit" in speed:
            eintraege.append((gross(f"↓{speed['down_mbit']:.0f}  ↑{speed['up_mbit']:.0f} Mbit/s"), stil["gruen"]))
    gesamt, geblockt = pihole if pihole else (None, None)
    if geblockt is not None:
        eintraege.append((gross(f"Pi-hole · {geblockt} geblockt"), stil["zeile_farbe"]))
    try:
        eintraege.append((gross(f"CPU {cpu_temperatur():.0f} °C"), stil["zeile2_farbe"]))
    except OSError:
        pass
    eintraege.append((gross(f"Läuft {laufzeit()}"), stil["zeile2_farbe"]))
    return [(tr(t), f) for t, f in eintraege]


def zeichnen(stil_name, W, H, KOPF, FUSS, jetzt, wetter, pihole, netz, warnungen, wetter_wort,
             cpu_temperatur, laufzeit, seite=0):
    """Fernansicht: große Uhr, große Werte, unten eine große Zeile, die mit 'seite' wechselt."""
    stil = STILE.get(stil_name, STILE["klar"])
    bild = Image.new("RGBA", (W, H), (0, 0, 0, 0))
    d = ImageDraw.Draw(bild)
    (o1, o2) = stil["leiste_oben"]
    for y in range(KOPF):
        t = y / KOPF
        d.line((0, y, W, y), fill=tuple(int(a + (b - a) * t) for a, b in zip(o1, o2)) + (255,))
    d.rectangle((0, FUSS + 4, W, H), fill=stil["leiste_unten"] + (255,))
    deko_linie(d, KOPF - 2, stil, W)
    deko_linie(d, FUSS + 4, stil, W, oben=False)
    art = dict(kontur=stil["kontur"], schatten=stil["schatten"])
    lab = dict(gewicht=stil["label_gewicht"], sperren=stil["sperren"], kontur=stil["kontur"])

    # Uhr: Großbuchstaben 56 px hoch (~1,5 m lesbar)
    uhr = jetzt.strftime("%H:%M")
    uhr_datei = stil["uhr"][0]
    gr = groesse_fuer(uhr_datei, 56)
    while gr > 20 and breite(d, uhr, (uhr_datei, gr)) > W - 2 * 122:   # Platz für Temperatur/Feuchte
        gr -= 1
    _, versatz = kappen_hoehe((uhr_datei, gr))
    if stil["glut"]:
        gluehen(bild, (W // 2, 6 - versatz), uhr, (uhr_datei, gr), stil["uhr_farbe"], stil["glut"])
        d = ImageDraw.Draw(bild)
    else:
        schreiben(d, (W // 2, 6 - versatz), uhr, (uhr_datei, gr), stil["uhr_farbe"], anker="ma",
                  kontur=stil["kontur"] * 2, schatten=stil["schatten"])
    datum = f"{WOCHENTAGE[jetzt.weekday()]}, {jetzt.day}. {MONATE[jetzt.month - 1]}"
    setzen(d, W // 2, 69, datum.upper() if stil["gross_datum"] else datum, stil["datum"][0], 16,
           stil["datum_farbe"], anker="ma", max_breite=210, **art)

    # Temperatur links, Luftfeuchte rechts: Großbuchstaben 32 px (~80 cm)
    if wetter:
        c = wetter["current"]
        zahl = f"{round(c['temperature_2m'])}"
        setzen(d, 12, 8, zahl, stil["wert"][0], 32, stil["wert_farbe"], **art)
        grad(d, 12 + breite(d, zahl, (stil["wert"][0], groesse_fuer(stil["wert"][0], 32))), 8, 32,
             stil["wert_farbe"])
        setzen(d, W - 12, 8, f"{c['relative_humidity_2m']}%", stil["wert"][0], 32, stil["wert_farbe"],
               anker="ra", **art)
        klein = (lambda t: t.upper()) if stil["label_gross"] else (lambda t: t)
        if warnungen:       # Warnung ersetzt das Wetterwort unter der Temperatur (alle stehen groß in der Wechselzeile)
            text_w, farbe_w = sorted(warnungen, key=lambda w: w[1] != WARN_ROT)[0]
            text_w = klein(tr(text_w.lstrip("⚠☀⚡❄ ")))
            text_w = text_w.replace("THUNDERSTORM", "THUNDER").replace("Thunderstorm", "Thunder")   # kurz halten
            if len(warnungen) > 1:
                text_w += f" +{len(warnungen) - 1}"
            ix, iy, dunkel = 13, 51, (10, 7, 5, 255)             # Warndreieck, 12 px
            d.polygon([(ix + 6, iy), (ix + 12, iy + 12), (ix, iy + 12)], fill=farbe_w + (255,))
            d.rectangle((ix + 5, iy + 4, ix + 7, iy + 8), fill=dunkel)
            d.rectangle((ix + 5, iy + 9, ix + 7, iy + 10), fill=dunkel)
            setzen(d, ix + 18, 50, text_w, stil["label"][0], 14, farbe_w, max_breite=118 - 18, **lab)
        else:
            setzen(d, 13, 50, klein(tr(wetter_wort.get(c["weather_code"], "").capitalize())), stil["label"][0], 14,
                   stil["label_farbe"], max_breite=118, **lab)
        setzen(d, W - 12, 50, klein(tr("Luftfeuchte")), stil["label"][0], 14, stil["label_farbe"], anker="ra",
               max_breite=118, **lab)

    # große Wechselzeile unten: Versalhöhe 34 px, nie unter 30 px (sonst Text kürzer machen)
    eintraege = laufschrift(stil, jetzt, wetter, pihole, netz, warnungen, wetter_wort, cpu_temperatur, laufzeit)
    text_, farbe = eintraege[seite % len(eintraege)]
    band = H - (FUSS + 6)
    datei = stil["zeile"][0]
    kappe = 34
    while kappe > 30 and breite(d, text_, (datei, groesse_fuer(datei, kappe))) > W - 20:
        kappe -= 1
    setzen(d, W // 2, FUSS + 6 + (band - kappe) // 2, text_, datei, kappe, farbe, anker="ma", **art)
    return bild, len(eintraege)
