#!/usr/bin/env python3
"""Info-Anzeige für das 3,5-Zoll-Display (480x320, ILI9486) am Raspberry Pi.

Zeigt Uhrzeit, Datum, Wetter (Open-Meteo, ohne Schlüssel) und eine Statuszeile
(Pi-hole heute, CPU-Temperatur, IP). Schreibt direkt in den Framebuffer /dev/fb0
(RGB565), braucht also keinen Desktop.
"""
import json
import os
import socket
import sqlite3
import time
import urllib.request
from datetime import datetime

import numpy as np
from PIL import Image, ImageDraw, ImageFont

# --- Einstellungen -----------------------------------------------------------
ORT = "Nikosia"
BREITE, LAENGE = 35.17, 33.35
FB = "/dev/fb0"
W, H = 480, 320
FTL_DB = "/etc/pihole/pihole-FTL.db"
WETTER_ALLE = 15 * 60      # Sekunden zwischen Wetterabfragen
PIHOLE_ALLE = 60
NETZ_ALLE = 60             # Sekunden zwischen Online-Prüfungen
SPEED_DATEI = "/var/lib/pi-anzeige/internet.json"   # schreibt speedtest.py
# Nachtmodus: Die Beleuchtung dieses Displays ist fest verdrahtet (per GPIO-Test
# 18.10.2026 geprüft), also nachts nur ein schwarzes Bild statt Ausschalten.
NACHT_VON, NACHT_BIS = None, None   # z. B. 23, 7 – aus, weil Schwarz bei dieser Beleuchtung kaum dunkler ist
TAKT = 5                   # Sekunden zwischen Prüfungen, ob neu gezeichnet werden muss

SCHRIFT = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"
SCHRIFT_FETT = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"

HINTERGRUND = (14, 17, 22)
TEXT = (235, 238, 242)
GEDIMMT = (140, 148, 160)
AKZENT = (255, 176, 59)
LINIE = (45, 52, 62)
GRUEN = (76, 201, 110)
ROT = (235, 87, 87)

WOCHENTAGE = ["Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag", "Sonntag"]
MONATE = ["Januar", "Februar", "März", "April", "Mai", "Juni", "Juli",
          "August", "September", "Oktober", "November", "Dezember"]

# WMO-Wettercodes (Open-Meteo) -> Text
WETTER_TEXT = {
    0: "Klar", 1: "Überwiegend klar", 2: "Teils bewölkt", 3: "Bedeckt",
    45: "Nebel", 48: "Reifnebel",
    51: "Leichter Niesel", 53: "Niesel", 55: "Starker Niesel",
    56: "Gefrierender Niesel", 57: "Gefrierender Niesel",
    61: "Leichter Regen", 63: "Regen", 65: "Starker Regen",
    66: "Gefrierender Regen", 67: "Gefrierender Regen",
    71: "Leichter Schnee", 73: "Schnee", 75: "Starker Schnee", 77: "Schneegriesel",
    80: "Regenschauer", 81: "Regenschauer", 82: "Heftige Schauer",
    85: "Schneeschauer", 86: "Schneeschauer",
    95: "Gewitter", 96: "Gewitter mit Hagel", 99: "Gewitter mit Hagel",
}

# Pi-hole-v6-Status, die als "geblockt" zählen
GEBLOCKT = (1, 4, 5, 6, 7, 8, 9, 10, 11, 15, 16, 18)


def schrift(groesse, fett=False):
    return ImageFont.truetype(SCHRIFT_FETT if fett else SCHRIFT, groesse)


F_UHR = schrift(84, True)
F_TAG = schrift(24, True)
F_DATUM = schrift(22)
F_TEMP = schrift(64, True)
F_GROSS = schrift(24, True)
F_MITTEL = schrift(18)
F_KLEIN = schrift(15)


def wetter_holen():
    url = ("https://api.open-meteo.com/v1/forecast"
           f"?latitude={BREITE}&longitude={LAENGE}"
           "&current=temperature_2m,apparent_temperature,weather_code,"
           "wind_speed_10m,relative_humidity_2m,is_day,uv_index,wind_gusts_10m"
           "&daily=temperature_2m_max,temperature_2m_min,"
           "precipitation_probability_max,uv_index_max,wind_gusts_10m_max"
           "&timezone=auto&forecast_days=1")
    with urllib.request.urlopen(url, timeout=10) as antwort:
        return json.load(antwort)


def pihole_heute():
    mitternacht = datetime.now().replace(hour=0, minute=0, second=0, microsecond=0)
    seit = int(mitternacht.timestamp())
    platzhalter = ",".join("?" * len(GEBLOCKT))
    con = sqlite3.connect(f"file:{FTL_DB}?mode=ro", uri=True, timeout=5)
    try:
        gesamt, geblockt = con.execute(
            f"SELECT COUNT(*), COALESCE(SUM(status IN ({platzhalter})), 0) "
            "FROM queries WHERE timestamp >= ?", (*GEBLOCKT, seit)).fetchone()
    finally:
        con.close()
    return gesamt, geblockt


def cpu_temperatur():
    with open("/sys/class/thermal/thermal_zone0/temp") as f:
        return int(f.read()) / 1000


def eigene_ip():
    s = socket.socket(socket.AF_INET, socket.SOCK_DGRAM)
    try:
        s.connect(("192.0.2.1", 9))  # baut keine Verbindung auf, wählt nur die Schnittstelle
        return s.getsockname()[0]
    except OSError:
        return "kein Netz"
    finally:
        s.close()


def online_pruefen():
    """Verbindungsaufbau zu zwei öffentlichen DNS-Diensten; liefert ms oder None."""
    for ziel in (("1.1.1.1", 443), ("9.9.9.9", 443)):
        t = time.perf_counter()
        try:
            with socket.create_connection(ziel, timeout=3):
                return (time.perf_counter() - t) * 1000
        except OSError:
            continue
    return None


def speed_lesen():
    try:
        with open(SPEED_DATEI) as f:
            return json.load(f)
    except (OSError, ValueError):
        return None


def ist_nacht(jetzt):
    if NACHT_VON is None:
        return False
    if NACHT_VON > NACHT_BIS:  # über Mitternacht, z. B. 23–7
        return jetzt.hour >= NACHT_VON or jetzt.hour < NACHT_BIS
    return NACHT_VON <= jetzt.hour < NACHT_BIS


def zeichnen(jetzt, wetter, pihole, netz=None):
    if ist_nacht(jetzt):
        return Image.new("RGB", (W, H), (0, 0, 0))
    bild = Image.new("RGB", (W, H), HINTERGRUND)
    d = ImageDraw.Draw(bild)

    # Uhrzeit und Datum
    d.text((12, 16), jetzt.strftime("%H:%M"), font=F_UHR, fill=TEXT)
    d.text((W - 14, 30), WOCHENTAGE[jetzt.weekday()], font=F_TAG, fill=AKZENT, anchor="ra")
    d.text((W - 14, 66), f"{jetzt.day}. {MONATE[jetzt.month - 1]}", font=F_DATUM, fill=TEXT, anchor="ra")
    d.text((W - 14, 96), str(jetzt.year), font=F_DATUM, fill=GEDIMMT, anchor="ra")
    d.line((14, 134, W - 14, 134), fill=LINIE, width=2)

    # Wetter
    if wetter:
        c, t = wetter["current"], wetter["daily"]
        d.text((14, 146), f"{round(c['temperature_2m'])}°", font=F_TEMP, fill=AKZENT)
        x = 170
        d.text((x, 148), WETTER_TEXT.get(c["weather_code"], "–"), font=F_GROSS, fill=TEXT)
        d.text((x, 182), f"gefühlt {round(c['apparent_temperature'])}°  ·  "
               f"Luftf. {c['relative_humidity_2m']} %", font=F_MITTEL, fill=GEDIMMT)
        d.text((x, 207), f"Wind {round(c['wind_speed_10m'])} km/h", font=F_MITTEL, fill=GEDIMMT)
        regen = t["precipitation_probability_max"][0]
        d.text((14, 228), f"Heute {round(t['temperature_2m_min'][0])}° bis "
               f"{round(t['temperature_2m_max'][0])}°"
               + (f"  ·  Regen {regen} %" if regen is not None else ""),
               font=F_MITTEL, fill=TEXT)
        d.text((W - 14, 230), ORT, font=F_KLEIN, fill=GEDIMMT, anchor="ra")
    else:
        d.text((14, 160), "Wetter: noch keine Daten", font=F_GROSS, fill=GEDIMMT)
        d.text((14, 196), ORT, font=F_MITTEL, fill=GEDIMMT)
    d.line((14, 260, W - 14, 260), fill=LINIE, width=2)

    # Internet: online/offline, Ping, letzte Geschwindigkeitsmessung
    ms, offline_seit, speed = netz if netz else (None, None, None)
    if ms is not None:
        d.text((14, 266), "●", font=F_KLEIN, fill=GRUEN)
        zeile = f"Online {ms:.0f} ms"
        if speed and "down_mbit" in speed:
            gemessen = datetime.fromtimestamp(speed["zeit"]).strftime("%H:%M")
            zeile += (f"   ↓ {speed['down_mbit']:.0f}  ↑ {speed['up_mbit']:.0f} Mbit/s"
                      f"  ({gemessen})")
        d.text((32, 266), zeile, font=F_KLEIN, fill=TEXT)
    elif offline_seit:
        d.text((14, 266), "●", font=F_KLEIN, fill=ROT)
        d.text((32, 266), f"OFFLINE seit {offline_seit.strftime('%H:%M')}", font=F_KLEIN, fill=ROT)
    else:
        d.text((14, 266), "Internet: wird geprüft …", font=F_KLEIN, fill=GEDIMMT)

    # Statuszeilen
    if pihole is None:
        ph = "Pi-hole: keine Daten"
    else:
        gesamt, geblockt = pihole
        anteil = f"{100 * geblockt / gesamt:.0f} %" if gesamt else "0 %"
        ph = f"Pi-hole heute {gesamt} Anfr. · {geblockt} geblockt ({anteil})"
    d.text((14, 284), ph, font=F_KLEIN, fill=GEDIMMT)
    try:
        temp = f"CPU {cpu_temperatur():.0f} °C"
    except OSError:
        temp = "CPU –"
    d.text((14, 302), f"{temp}  ·  {eigene_ip()}", font=F_KLEIN, fill=GEDIMMT)
    return bild


def nach_rgb565(bild):
    a = np.asarray(bild, dtype=np.uint16)
    rgb565 = ((a[..., 0] >> 3) << 11) | ((a[..., 1] >> 2) << 5) | (a[..., 2] >> 3)
    return rgb565.astype("<u2").tobytes()


def main():
    wetter, wetter_zeit = None, 0.0
    pihole, pihole_zeit = None, 0.0
    netz, netz_zeit, offline_seit = None, 0.0, None
    letzter_stand = None
    with open(FB, "r+b", buffering=0) as fb:
        while True:
            jetzt = datetime.now()
            mono = time.monotonic()
            if not wetter_zeit or mono - wetter_zeit >= WETTER_ALLE:
                try:
                    wetter = wetter_holen()
                    wetter_zeit = mono
                except Exception:
                    # Bei Fehler in 2 Minuten erneut versuchen, alte Daten bleiben stehen.
                    wetter_zeit = mono - WETTER_ALLE + 120
            if not pihole_zeit or mono - pihole_zeit >= PIHOLE_ALLE:
                try:
                    pihole = pihole_heute()
                except Exception:
                    pihole = None
                pihole_zeit = mono
            if not netz_zeit or mono - netz_zeit >= NETZ_ALLE:
                ms = online_pruefen()
                if ms is None:
                    offline_seit = offline_seit or jetzt
                else:
                    offline_seit = None
                netz = (ms, offline_seit, speed_lesen())
                netz_zeit = mono
            stand = (jetzt.strftime("%Y%m%d%H%M"), id(wetter), pihole,
                     None if netz is None else (netz[0] is None, netz[1], str(netz[2])))
            if stand != letzter_stand:
                fb.seek(0)
                fb.write(nach_rgb565(zeichnen(jetzt, wetter, pihole, netz)))
                letzter_stand = stand
            time.sleep(TAKT)


if __name__ == "__main__":
    os.nice(10)
    main()
