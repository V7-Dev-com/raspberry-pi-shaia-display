//! Rendert eine Vorführung der Anzeige als rohe RGB-Bilder auf stdout (24 fps), zum Einspeisen in ffmpeg.
//! Aufruf:  PI_SPRACHE=en PI_ASSETS=../assets cargo run --release --bin video -- [sekunden] [seed] [stil] | ffmpeg ...
use chrono::{Duration, Local, TimeZone};
use shaia_display::assets::{assets_ordner, Assets, H, W};
use shaia_display::data::*;
use shaia_display::hud::{self, Eingabe};
use shaia_display::scene::*;
use shaia_display::text::Schriften;
use std::io::Write;
use std::rc::Rc;

const FPS: f64 = 24.0;

fn main() {
    let arg = |i: usize| std::env::args().nth(i);
    let sekunden: f64 = arg(1).and_then(|s| s.parse().ok()).unwrap_or(60.0);
    let seed: u64 = arg(2).and_then(|s| s.parse().ok()).unwrap_or(5);
    let stil = arg(3).unwrap_or("glut".into());
    let a = Rc::new(Assets::laden(&assets_ordner().join("shaia")).expect("Grafiken"));
    let mut sf = Schriften::neu(assets_ordner().join("fonts"));
    let stunde = 14;
    let mut sz = Szene::neu(a, Some(seed), stunde, 5000.0);
    sz.bonus_ab = 1e9;
    sz.spaziergang_ab = 12.0 + 5000.0;
    sz.kugel_ab = 5000.0 + 38.0;
    sz.truhe_ab = 5000.0 + 48.0;
    sz.truhe_fuer_stunde = stunde;
    let start = Local.with_ymd_and_hms(2026, 10, 20, 18, 42, 0).unwrap();
    let w = Wetter {
        current: Aktuell { temperature_2m: 19.0, apparent_temperature: 18.0, weather_code: 2, wind_speed_10m: 14.0,
            relative_humidity_2m: 64.0, is_day: Some(1), uv_index: Some(3.0), wind_gusts_10m: Some(24.0) },
        daily: Taeglich { temperature_2m_max: vec![24.0], temperature_2m_min: vec![14.0], precipitation_probability_max: vec![Some(30.0)] },
    };
    // Gegner erscheinen, wenn Pi-hole etwas blockiert: Zeitpunkte der "blockierten Anfragen"
    let blockiert_bei = [6.0, 24.0, 26.0, 44.0];
    let (mut gesamt, mut geblockt) = (412i64, 138i64);
    let mut naechste = 0;
    let (mut seite_alt, mut ping) = (usize::MAX, 4.0f64);
    let out = std::io::stdout();
    let mut out = std::io::BufWriter::with_capacity(1 << 20, out.lock());
    let n = (sekunden * FPS) as usize;
    for i in 0..n {
        let t = i as f64 / FPS;
        while naechste < blockiert_bei.len() && t >= blockiert_bei[naechste] {
            sz.skelette_dazu(1);
            geblockt += 1;
            gesamt += 3;
            naechste += 1;
        }
        sz.schritt(1.0 / FPS, stunde, 5000.0 + t, false, false);
        let seite = (t / 4.0) as usize;
        if seite != seite_alt || (i % 48 == 0) {
            seite_alt = seite;
            ping = 3.5 + ((t * 0.7).sin() + 1.0) * 1.4;
            let jetzt = start + Duration::milliseconds((t * 1000.0) as i64);
            let netz = Netz {
                ms: Some(ping), offline_seit: None,
                speed: Some(Speed { down_mbit: 241.0, up_mbit: 70.0, zeit: jetzt.timestamp() - 2 * 3600 - 600 }),
                pi_ip: Some("192.168.1.9".into()), router_ip: Some("192.168.1.1".into()), router_ok: None,
            };
            let warn = hud::warnungen(Some(&w));
            let e = Eingabe {
                wetter_alter: 0, pihole_alter: 0, jetzt, wetter: Some(&w), pihole: Some((gesamt, geblockt)), netz: Some(&netz),
                warnungen: &warn, cpu: Some(61.0), laufzeit: "1 d 15 h".into(), seite,
            };
            sz.statisch_setzen(hud::zeichnen(&mut sf, &stil, W as i32, H as i32, 94, 248, &e).0);
        }
        let b = sz.zeichnen(5000.0 + t);
        let mut rgb = Vec::with_capacity(W * H * 3);
        for y in 0..H {
            for x in 0..W {
                let p = b.px(x, y);
                rgb.extend_from_slice(&[p[0], p[1], p[2]]);
            }
        }
        if out.write_all(&rgb).is_err() {
            break;
        }
    }
    let _ = out.flush();
}
