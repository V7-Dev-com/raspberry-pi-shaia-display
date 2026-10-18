#!/usr/bin/env python3
"""Misst die Internet-Geschwindigkeit über Cloudflare und legt das Ergebnis
für die Info-Anzeige in /var/lib/pi-anzeige/internet.json ab.

Mehrere parallele Downloads/Uploads über einige Sekunden, weil Cloudflare
einzelne Downloads auf etwa 25 MB begrenzt. Läuft per systemd-Timer.
"""
import json
import os
import socket
import threading
import time
import urllib.request

ZIEL = "/var/lib/pi-anzeige/internet.json"
UA = {"User-Agent": "pi-anzeige"}
DAUER = 6          # Sekunden je Richtung
FAEDEN = 4         # parallele Verbindungen
UP_STUECK = b"0" * 5_000_000


def ping_ms(host="1.1.1.1", port=443):
    """Kleinste Verbindungsaufbauzeit aus drei Versuchen."""
    zeiten = []
    for _ in range(3):
        t = time.perf_counter()
        with socket.create_connection((host, port), timeout=5):
            zeiten.append((time.perf_counter() - t) * 1000)
    return min(zeiten)


def download():
    req = urllib.request.Request("https://speed.cloudflare.com/__down?bytes=25000000", headers=UA)
    n = 0
    with urllib.request.urlopen(req, timeout=30) as antwort:
        while True:
            block = antwort.read(1 << 16)
            if not block:
                return n
            n += len(block)


def upload():
    req = urllib.request.Request("https://speed.cloudflare.com/__up", data=UP_STUECK, method="POST",
                                 headers=UA | {"Content-Type": "application/octet-stream"})
    with urllib.request.urlopen(req, timeout=30) as antwort:
        antwort.read()
    return len(UP_STUECK)


def messen(funktion):
    """Mbit/s über DAUER Sekunden mit FAEDEN parallelen Verbindungen."""
    summe, fehler = [0], []
    sperre = threading.Lock()
    ende = time.monotonic() + DAUER

    def arbeiter():
        while time.monotonic() < ende:
            try:
                n = funktion()
            except Exception as e:  # noqa: BLE001 - jeder Fehler beendet nur diesen Faden
                fehler.append(e)
                return
            with sperre:
                summe[0] += n

    start = time.monotonic()
    faeden = [threading.Thread(target=arbeiter) for _ in range(FAEDEN)]
    for f in faeden:
        f.start()
    for f in faeden:
        f.join()
    if summe[0] == 0:
        raise RuntimeError(f"keine Daten übertragen: {fehler[0] if fehler else 'unbekannt'}")
    return summe[0] * 8 / (time.monotonic() - start) / 1e6


def main():
    try:
        ergebnis = {
            "zeit": int(time.time()),
            "ping_ms": round(ping_ms(), 1),
            "down_mbit": round(messen(download), 1),
            "up_mbit": round(messen(upload), 1),
        }
    except Exception as e:  # noqa: BLE001
        # Letzte gute Messung behalten, nur den Fehler dazuschreiben.
        try:
            with open(ZIEL) as f:
                ergebnis = json.load(f)
        except (OSError, ValueError):
            ergebnis = {}
        ergebnis["fehler"] = str(e)[:200]
        ergebnis["fehler_zeit"] = int(time.time())
    tmp = ZIEL + ".tmp"
    with open(tmp, "w") as f:
        json.dump(ergebnis, f)
    os.replace(tmp, ZIEL)
    print(json.dumps(ergebnis))


if __name__ == "__main__":
    main()
