# Rust-Neubau (v2) – Plan

Ziel: dieselbe Anzeige wie v1 (Python), aber als **eine Rust-Binärdatei** ohne Python, Pillow und numpy. Der normale GNU/Linux-Build ist dynamisch an Systembibliotheken (insbesondere glibc) gebunden; SQLite ist eingebettet.
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
4. **Dienst:** `pi-shaia.service` startet die Rust-Binärdatei; **Stand 20.10.2026: die Python-Version v1 ist vom Pi und aus dem Repo entfernt (steht in der Git-Historie); auch der Speedtest läuft als Rust-Binärdatei.**

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
- Dienstdatei: `pi-shaia.service`; Aktivierung und Live-Status sind hier nicht festgeschrieben. Dieses lokale Review prüft keinen Pi-Dienststatus.

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

Schrift: Randdeckung^0,45 und echte Alpha-Mischung (vorher dunkelten die Ränder ab und die Schrift wirkte blass); Farben im Stil „glut“ etwas satter.


## Lokales Review 19.10.2026

Geprüft: sämtliche `src/*.rs`, `src/bin/*.rs`, Cargo-Konfiguration, Dienstdatei und README/Plan.
Keine neuen Abhängigkeiten, keine geänderten Darstellungsregeln oder Abruf-/Animationsintervalle.

Kleine Korrekturen: verkürzte Routentabellen ohne Index-Panic; begrenzte Skelett-Warteschlange
auch vor Integerüberläufen; große Pi-hole-Differenzen ohne abschneidenden u32-Cast;
Netz-Fehlversuche sättigen, Wetter-Version läuft ausdrücklich umlaufend weiter.
Kein Rückfall auf Unix-Epoche 0 bei einem nicht existierenden lokalen Tagesbeginn.
Speedtest-Fehlerbehandlung verträgt JSON-Skalare/Arrays und meldet Dateifehler ohne `expect`.
Displayfehler werden weitergegeben: der Prozess endet mit Diagnose, systemd kann nach den
vorhandenen 10 Sekunden neu starten. DRM-Framebuffer/Dumb-Buffer werden explizit freigegeben,
auch bei teilweise fehlgeschlagener Initialisierung. fb0 nutzt einen wiederverwendeten Bytepuffer.
Wetter/Netz nutzen Arc-Snapshots; Cache-Schlüssel nutzen wiederverwendete Strings und erfassen
auch Router-Erreichbarkeit und Speedtest-Zeit. Akteur-Ticks borgen ihren Schritt, statt dessen
Vektoren jedes Bild zu klonen. Fehlende Schriftdateien werden ebenfalls gecacht.

Bewusst offen (größere Änderung, Hardwareabhängigkeit oder würde Darstellungsverhalten ändern):

- `main.rs`: Datenfehler bei Wetter/DB werden weiter still behandelt; letzte gute Werte bleiben
  samt Alter erhalten. Ein panikender Hintergrundfaden wird nicht überwacht/neugestartet;
  Mutex-Poisoning führt über `unwrap` zum Prozessabbruch. Aktuell kein belegter Daten-Race:
  Mutex schützt den Snapshot, Quellen schreiben getrennte Felder. Rust wertete bereits vor
  der Umstellung die rechte Seite der Netz-Zuweisung vor dem Lock aus.
- Tageswechsel: Pi-hole-Rückgänge setzen die Differenzbasis zurück; Anfragen zwischen dem
  letzten Abruf vor und dem ersten Abruf nach Mitternacht lassen sich damit nicht vollständig
  in Skelette umsetzen. Nach DB-Reset gilt dasselbe. Wetter-Tageswerte und `is_day` bleiben bis
  zum nächsten erfolgreichen Abruf alt; veraltetes `is_day=0` kann die Nachtszene tagsüber halten.
  Wetter nutzt Nikosia/`timezone=auto`, Uhr/DB die Systemzeitzone: diese muss dazu passen.
  In Zonen mit übersprungener Mitternacht wird nun ein Fehler statt aller historischen Anfragen
  geliefert; Unterstützung des ersten tatsächlich existierenden Tageszeitpunkts bleibt offen.
- Ausgabe: KMS-Modus/CRTC-Auswahl und fb0 setzen die vorgesehene Hardware voraus;
  fb0 fragt weder Pixelformat noch Zeilenabstand per ioctl ab. Display-Hotplug und Wiederherstellung
  des vorherigen CRTC sind nicht implementiert. DRM-Cleanup bei SIGKILL erfolgt durch den Kernel.
- Touch: nur 64-Bit-Linux-Ereignisformat; nach Geräteverlust kein erneutes Öffnen.
  Kein Touch-/Display-Hardwaretest auf diesem Rechner. Keine echte Netz-/Pi-hole-Ausfallprüfung
  am Pi; SQL-Fehler und Schreibfehler werden lokal mit Testdaten/Fehlergeräten geprüft.
- Ressourcen: Szene erzeugt weiterhin einen RGBA-Frame je Bild, Kontext-/Aktions-/Treffer-Vektoren
  allokieren stellenweise. Keine nachgewiesenen unbeschränkten Ressourcenlecks im regulären
  Szenenpfad; vollständige Puffer-Wiederverwendung wäre ein größerer Umbau. Szene-IDs sind u32
  und können nach Milliarden erzeugten Objekten umlaufen; kein realistischer Kurzzeitfehler.
- Tote/ungenutzte Teile: `strom`-Animation und `chicken`-Asset werden geladen, aber intern
  nicht gewählt; `Werkstatt.sf` in `bin/shot` ist ungenutzt. Keine Entfernung aus der Asset-/
  Animationsauswahl im Rahmen dieses Reviews.
- Panics: Grafik-/Animationsindexe vertrauen den vollständig geladenen Assets; öffentliche
  Bild-/Szenenstrukturen erlauben ungültige Größen, leere Reihen oder nicht endliche Koordinaten.
  DejaVu komplett fehlend/defekt führt weiter zu einem klaren Start-Panic. CLI-Testwerkzeuge
  verwenden `unwrap` für feste gültige Zeiten, Assets und Ausgabedateien.
- `bin/sim` zählt Zustände, erzwingt aber weder vollständige Zustandsabdeckung noch Obergrenzen
  für sämtliche Effekt-/Beute-/Geschosslisten. `bin/shot` kann beim Erreichen eines Suchlimits
  ein Ersatzbild liefern. Das ist kein Beweis vollständiger Animationsabdeckung.
- Speedtest: fester `.tmp`-Name setzt nicht überlappende Läufe voraus; atomarer Rename ist nicht
  stromausfallsicher (kein fsync). Requests dürfen die Messdauer überschreiten; Gesamtmittel
  zählt vollständig abgeschlossene Übertragungen. Messverfahren/Timer bleiben unverändert.
- Dienst: feste Konten/Gruppen, `/opt`-Pfad und `vtcon1/bind` müssen auf dem Pi existieren;
  fehlendes vtcon1 verhindert den Start. `After=time-sync.target` erzwingt keine Zeitsynchronisation.
  Keine neuen Sandbox-/Speicherlimits ohne Hardwareprüfung gesetzt. Keine Deployment-Aussage.


Validierung dieses Reviews (Workstation, kein Pi-Live-Test):

- `cargo +1.85 test --release --locked -j 2`: 35 Tests bestanden, davon 12 neue.
- `cargo +1.98.1 clippy --release --all-targets --locked -j 2 -- -D warnings`: ohne Warnungen.
  Clippy ist hier nur in Toolchain 1.98.1 installiert; Build/Tests laufen ausdrücklich mit 1.85.
- `cargo +1.85 build --release --locked -j 2 --bins`: alle Binärdateien gebaut.
- Neun `shot`-PNGs (Stil glut, echte lokale Assets, feste Seeds) gegen unverändertes HEAD:
  alle neun Dateien byteidentisch. Das prüft die Stichprobe, nicht alle Zufallsverläufe.
- `sim 48 1`: 48 simulierte Stunden ohne Assertion/Panic; maximal 2 Skelette gleichzeitig,
  Warteschlange maximal 6. Dies ersetzt keinen 48-Stunden-Test auf dem Pi.
- `ldd` bestätigt dynamische glibc/libm/libgcc_s-Abhängigkeiten des normalen Release-Builds.
- `systemd-analyze verify pi-shaia.service` meldet hier den nicht installierten
  `/opt/pi-anzeige/shaia-display`-Pfad; Dienststart/Benutzer/Geräte wurden nicht validiert.
