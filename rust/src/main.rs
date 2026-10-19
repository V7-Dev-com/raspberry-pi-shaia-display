//! Shaia-Anzeige (v2, Rust): Hauptschleife wie `main()` in ruinrunner.py.

use chrono::{Local, Timelike};
use shaia_display::assets::{assets_ordner, grafiken_fehlen, Assets, H, SHAIA_URL, W};
use shaia_display::data::*;
use shaia_display::hud::{self, Eingabe};
use shaia_display::output::{spi_takt, Ausgabe};
use shaia_display::scene::{Szene, NACHT};
use shaia_display::text::Schriften;
use shaia_display::touch::Beruehrung;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const FPS: f64 = 24.0;
const SEITE_SEKUNDEN: f64 = 4.0;
const KOPF: i32 = 94;
const LEISTE: i32 = 248;
const STIL_DATEI: &str = "/opt/pi-anzeige/stil.txt";

#[derive(Default)]
struct Daten {
    wetter: Option<Wetter>,
    wetter_version: u64,
    pihole: Option<(i64, i64)>,
    wetter_zeit: Option<Instant>,
    pihole_zeit: Option<Instant>,
    netz: Option<Netz>,
}

/// Holt Wetter, Pi-hole und Netzstatus im Hintergrund, damit die Animation nie wartet.
/// Jede Quelle hat ihren eigenen Faden - ein zäher Wetterabruf darf die Netzprüfung nicht aufhalten.
fn daten_faden(d: Arc<Mutex<Daten>>) {
    let dw = d.clone();
    std::thread::spawn(move || loop {
        let pause = match wetter_holen() {
            Ok(w) => {
                let mut g = dw.lock().unwrap();
                g.wetter = Some(w);
                g.wetter_version += 1;
                g.wetter_zeit = Some(Instant::now());
                WETTER_ALLE
            }
            Err(_) => 120.0,
        };
        std::thread::sleep(Duration::from_secs_f64(pause));
    });
    let dp = d.clone();
    std::thread::spawn(move || loop {
        if let Ok(p) = pihole_heute() {
            let mut g = dp.lock().unwrap();
            g.pihole = Some(p);
            g.pihole_zeit = Some(Instant::now());
        }
        std::thread::sleep(Duration::from_secs_f64(PIHOLE_ALLE));
    });
    std::thread::spawn(move || {
        let mut offline_seit: Option<chrono::DateTime<Local>> = None;
        let mut fehlversuche = 0u32;
        loop {
            let ms = online_pruefen();
            // Erst nach zwei Fehlversuchen hintereinander gilt das Internet als weg (ein verlorenes Paket reicht nicht)
            fehlversuche = if ms.is_some() { 0 } else { fehlversuche + 1 };
            let pause = if fehlversuche == 1 { 4.0 } else { NETZ_ALLE };
            if fehlversuche != 1 {
                offline_seit = if fehlversuche == 0 { None } else { offline_seit.or(Some(Local::now())) };
                let router_ip = router_ip();
                let router_ok = if ms.is_none() { router_ip.as_deref().map(router_erreichbar) } else { None };
                d.lock().unwrap().netz = Some(Netz { ms, offline_seit, speed: speed_lesen(), pi_ip: eigene_ip(), router_ip, router_ok });
            }
            std::thread::sleep(Duration::from_secs_f64(pause));
        }
    });
}

fn stil_lesen() -> String {
    std::fs::read_to_string(std::env::var("PI_STIL_DATEI").unwrap_or_else(|_| STIL_DATEI.into()))
        .map(|s| s.trim().to_string())
        .ok()
        .filter(|s| hud::stil_bekannt(s))
        .unwrap_or_else(|| "klar".into())
}

fn main() {
    let sprites = assets_ordner().join("shaia");
    let fehlt = grafiken_fehlen(&sprites);
    if !fehlt.is_empty() {
        eprintln!(
            "Shaia-Grafiken fehlen in {} (Ordner: {}).\nBitte bei picopico256 laden: {SHAIA_URL}\n\
             und den Ordner 'sprites' des Pakets nach {} kopieren (oder PI_ASSETS auf den Assets-Ordner setzen).\n\
             Die Grafiken sind nicht Teil dieses Repos.",
            sprites.display(), fehlt.join(", "), sprites.display()
        );
        std::process::exit(2);
    }
    let a = Rc::new(Assets::laden(&sprites).unwrap_or_else(|e| {
        eprintln!("Grafiken laden: {e}");
        std::process::exit(2)
    }));
    let mut sf = Schriften::neu(assets_ordner().join("fonts"));
    let start = Instant::now();
    let zeit = |i: Instant| 1000.0 + i.duration_since(start).as_secs_f64();
    let mut szene = Szene::neu(a, None, Local::now().hour(), zeit(Instant::now()));
    let mut tippen = Beruehrung::neu();
    let daten = Arc::new(Mutex::new(Daten::default()));
    daten_faden(daten.clone());
    let mut ausgabe = Ausgabe::neu();
    let spi_hz = spi_takt();
    let (mut stil, mut stil_zeit) = (stil_lesen(), Instant::now());
    let mut letzt = Instant::now();
    let mut vorher = vec![0u16; W * H];
    let mut erstes = true;
    let mut puffer = vec![0u16; W * H];
    let mut letzter_schluessel = String::new();
    // Die Temperatur nur einmal pro Sekunde lesen - ein Dateizugriff je Bild kostet auf dem Pi spürbar CPU
    let (mut cpu_wert, mut cpu_zeit): (Option<f64>, Option<Instant>) = (None, None);
    let mut letzte_blockiert: Option<i64> = None;

    loop {
        let anfang = Instant::now();
        let jetzt_dt = Local::now();
        let dt = anfang.duration_since(letzt).as_secs_f64().min(0.2);
        letzt = anfang;
        let (wetter, wetter_v, pihole, netz, wetter_alter, pihole_alter) = {
            let g = daten.lock().unwrap();
            (
                g.wetter.clone(), g.wetter_version, g.pihole, g.netz.clone(),
                g.wetter_zeit.map_or(0, |t| t.elapsed().as_secs() as i64), g.pihole_zeit.map_or(0, |t| t.elapsed().as_secs() as i64),
            )
        };
        if let Some((_, geblockt)) = pihole {
            if let Some(alt) = letzte_blockiert {
                if geblockt > alt {
                    szene.skelette_dazu((geblockt - alt) as u32);
                }
            }
            letzte_blockiert = Some(geblockt);
        }
        let offline = netz.as_ref().is_some_and(|n| n.offline());
        let stunde = jetzt_dt.hour();
        szene.nacht = wetter.as_ref().is_some_and(|w| w.current.is_day == Some(0)) || stunde >= NACHT.0 || stunde < NACHT.1;
        szene.schritt(dt, stunde, zeit(anfang), offline, tippen.angetippt());

        if stil_zeit.elapsed().as_secs_f64() >= 2.0 {
            stil = stil_lesen();
            stil_zeit = Instant::now();
        }
        if cpu_zeit.is_none_or(|t| t.elapsed().as_secs_f64() >= 1.0) {
            cpu_wert = cpu_temperatur();
            cpu_zeit = Some(Instant::now());
        }
        let seite = (zeit(anfang) / SEITE_SEKUNDEN) as usize;
        let schluessel = format!(
            "{}|{}|{:?}|{}|{}|{:?}|{:?}",
            jetzt_dt.format("%Y%m%d%H%M"), wetter_v, pihole, stil, seite, cpu_wert.map(|c| c.round() as i64),
            netz.as_ref().map(|n| (n.ms.map(|m| m as i64), n.offline_seit, n.speed.as_ref().map(|s| (s.down_mbit as i64, s.up_mbit as i64)), &n.pi_ip, &n.router_ip))
        );
        if schluessel != letzter_schluessel || szene.statisch.is_none() {
            let warn = if wetter_alter > 1800 { vec![] } else { hud::warnungen(wetter.as_ref()) };
            let e = Eingabe { wetter_alter, pihole_alter,
                jetzt: jetzt_dt, wetter: wetter.as_ref(), pihole, netz: netz.as_ref(), warnungen: &warn,
                cpu: cpu_wert, laufzeit: laufzeit(), seite,
            };
            szene.statisch_setzen(hud::zeichnen(&mut sf, &stil, W as i32, H as i32, KOPF, LEISTE, &e).0);
            letzter_schluessel = schluessel;
        }

        let bild = szene.zeichnen(zeit(anfang));
        bild.nach_rgb565(&mut puffer);
        let (oben, unten) = if erstes {
            (0, H)
        } else {
            let geaendert = |y: &usize| puffer[y * W..(y + 1) * W] != vorher[y * W..(y + 1) * W];
            match ((0..H).find(geaendert), (0..H).rev().find(geaendert)) {
                (Some(a), Some(b)) => (a, b + 1),
                _ => (0, 0),
            }
        };
        let mut uebertragung = 0.0;
        if unten > oben {
            uebertragung = ausgabe.zeigen(&bild, &puffer, oben, unten, spi_hz);
        }
        erstes = false;
        std::mem::swap(&mut vorher, &mut puffer);

        // nicht schneller liefern, als die Leitung überträgt - sonst stauen sich Bilder und es ruckelt
        let rest = (1.0f64 / FPS).max(uebertragung) - anfang.elapsed().as_secs_f64();
        if rest > 0.0 {
            std::thread::sleep(Duration::from_secs_f64(rest));
        }
    }
}
