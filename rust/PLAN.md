# Rust-Neubau (v2) – Plan

Ziel: dieselbe Anzeige wie v1 (Python), aber als **eine statische Binärdatei** ohne Python, Pillow und numpy.
Weniger CPU und RAM, schneller Start, kein `pip`/`apt`-Ballast auf dem Pi.

## Entscheidungen

| Thema | Wahl | Grund |
|---|---|---|
| Bilder laden | `image` (nur PNG) | klein, ausreichend |
| Schrift | `ab_glyph` | reines Rust, TTF; PIL-Größenlogik (Em-Pixel) wird nachgebaut |
| Zeichnen | eigenes `gfx`-Modul (RGBA, Alpha-Blit mit Clipping, Linien, Polygone, Weichzeichner) | PIL-Verhalten 1:1 nachbilden, keine schwere Abhängigkeit |
| Display | `drm` (reines Rust, Dumb-Buffer RG16, `dirty_framebuffer`), Rückfall `/dev/fb0` | wie `pykms`: Übertragung im eigenen Takt |
| Touch | rohe `input_event`-Daten von `/dev/input/eventN` (ADS7846) | keine Abhängigkeit |
| Daten | `ureq` (Open-Meteo), `rusqlite` bundled (Pi-hole-FTL-DB, nur lesend), `std::net` (Ping) | |
| Zufall | `fastrand` | |
| Zeit | `chrono` (lokale Zeit) | |
| Ablauf der Figuren | Schritte mit **Aktions-Enums** statt Closures | Closures auf `&mut Szene` sind in Rust nicht borgbar; Enums sind testbar |

## Aufbau (`src/`)

- `gfx.rs` – RGBA-Bild, Blit, Füllen, Linie, Polygon, Ellipse, Skalieren, Weichzeichnen, Umwandlung RGB565
- `text.rs` – Schriften laden (Ersatzschrift DejaVu), Breite, Kappenhöhe, `setzen`/`schreiben` mit Anker, Schatten, Kontur
- `hud.rs` – Stile (glut, klar, retro, v7), Kopf- und Fußleiste, Wechselzeile, Warnung, Englisch (`PI_SPRACHE=en`)
- `assets.rs` – Sprite-Reihen laden, auf den Inhalt zuschneiden, spiegeln; Hintergrund mit Zoom
- `scene.rs` – Akteur, Schritte, Skelett, Szene: Kampf, Alltag, Kugeln, Kiste, Lagerfeuer, Kamera, Zeichnen
- `data.rs` – Wetter, Pi-hole, Ping, Speedtest-Datei, CPU, Laufzeit
- `output.rs` – KMS, fb0, PNG-Ausgabe für Tests
- `touch.rs` – Antippen
- `main.rs` – Hauptschleife wie `main()` in v1
- `bin/shot.rs` – erzeugt die Testbilder (wie `werkzeuge/bildschirmfotos.py`)
- `bin/speedtest.rs` – Cloudflare-Messung wie `speedtest.py`

## Abnahme

1. **Oberfläche (HUD):** Pixelvergleich gegen die Python-Ausgabe bei gleicher Eingabe. Erlaubt: Abweichung durch andere Glyphen-Rasterung (mittlere Abweichung klein, Layout identisch auf ±1 px).
2. **Szene:** gleiche Konstanten und Regeln wie v1. Ein Zufallsvergleich ist nicht möglich; stattdessen
   Probeläufe: keine Abstürze über viele simulierte Stunden, alle Zustände erreichbar (Kampf, Sprung des Skeletts, Kiste, Lagerfeuer, Offline), Kiste und Feuer an den festen Stellen.
3. **Auf dem Pi:** CPU und RAM gemessen und gegen v1 gestellt (v1: ca. 27 % eines Kerns, 90 MB, 24 fps).
4. **Dienst:** `pi-ruinrunner` läuft mit der Rust-Binärdatei; Python bleibt als Rückfall, bis v2 mindestens einen Tag stabil lief.

## Arbeitspakete (auch für Codex geeignet)

Jedes Paket ist in sich geschlossen und hat die Python-Datei als Vorlage.

| Paket | Vorlage | Ergebnis | Prüfung |
|---|---|---|---|
| A `gfx` + `text` | `hud.py`, `ruinrunner.py` (`einfuegen`, `nach_rgb565`) | Module + Tests | Einheitentests, Vergleichsbilder |
| B `hud` | `hud.py` | `hud.rs` | Pixelvergleich mit Python |
| C `assets` | `ruinrunner.py` (`lade_reihe`, Hintergrund) | `assets.rs` | Bildgrößen und Versätze gleich |
| D `scene` | `ruinrunner.py` (Szene, Akteur) | `scene.rs` | Probeläufe, `bin/shot` |
| E `data` + `touch` | `anzeige.py`, `pixelwelt.py` | `data.rs`, `touch.rs` | Werte gleich wie Python |
| F `output` + `main` | `ruinrunner.py` (`Ausgabe`, `main`) | KMS-Ausgabe, Schleife | läuft auf dem Pi |
| G `speedtest` | `speedtest.py` | `bin/speedtest.rs` | Werte ähnlich wie Python |

Stand und Entscheidungen stehen unten im Abschnitt „Fortschritt“.

## Fortschritt

- [x] A gfx + text (Einheitentests; Schrift an PIL angeglichen: Kappenhöhe, Zeichenabstand, Ersatzschrift)
- [x] B hud (Pixelvergleich mit Python: mittlere Abweichung 1,1 von 255 über das ganze Bild, Layout deckungsgleich)
- [x] C assets
- [x] D scene (alle Kampf-, Alltags-, Kisten-, Lagerfeuer- und Offline-Zustände; `bin/sim` 48 h ohne Absturz)
- [x] E data + touch
- [x] F output (KMS/fb0/PNG) + main
- [x] G speedtest
- [x] Messung auf dem Pi (19.10.2026): Rust 22 % eines Kerns und 39 MB RAM gegen Python 27 % und 90 MB; KMS-Ausgabe läuft (Plane RG16, Dumb-Buffer von `shaia-display`)
- [ ] Dienst umstellen: `pi-shaia.service` ist installiert, aber nicht aktiviert; Python (`pi-ruinrunner`) bleibt beim Start Standard, bis v2 einen Tag lief

Werkzeuge: `bin/shot` (Testbilder wie `bildschirmfotos.py`), `bin/hudtest` (Oberfläche mit festen Eingaben),
`bin/sim` (Dauerlauf), `bin/metrics` (Schriftmaße).

## Hinweise

- Bauen auf dem Pi: `apt install cargo build-essential`, dann `nice -n 19 taskset -c 3 cargo build --release --locked`.
  Ohne Lüfter heizt ein Bau auf allen Kernen den Pi bis ~83 °C (Drosselung); einen Kern zu nehmen hält ihn bei ~72 °C. Dauert ca. 15 Minuten.
- `rust-version = "1.85"` entspricht dem Cargo von Debian Trixie; das Lockfile ist darauf aufgelöst.

## Optimierung 19.10.2026 (nach der ersten Messung)

| Maßnahme | Wirkung auf dem Pi |
|---|---|
| Weichzeichner nur im Textbereich, gleitende Summe statt Mehrfach-Summen (Ergebnis bitgleich) | Oberfläche neu zeichnen 302 ms → 12,6 ms (vorher ruckelte es alle 4 s kurz) |
| Nacht-Halle einmal vorgerechnet | Nachtbild 3,8 ms → 1,3 ms |
| Oberfläche zeilenweise: leere Zeilen überspringen, deckende kopieren | Zeichnen 1,3 ms → 0,9 ms |
| Puffer tauschen statt kopieren, Zeilenkopie beim Display | weniger Speicherverkehr |

Gemessen im Betrieb (60 s): **7 % eines Kerns** (5 % Nutzer + 2 % System), 40 MB RAM, 62 °C. Vorher Rust 22 %, Python 27 % und 90 MB.

Schrift: Randdeckung^0,65 und echte Alpha-Mischung (vorher dunkelten die Ränder ab und die Schrift wirkte blass); Farben im Stil „glut“ etwas satter.
