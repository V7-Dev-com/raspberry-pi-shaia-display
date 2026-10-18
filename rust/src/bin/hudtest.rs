//! Zeichnet die Oberfläche mit festen Eingaben als PNG (zum Vergleich mit hud.py).
use chrono::{Local, TimeZone};
use shaia_display::data::*;
use shaia_display::hud::{self, Eingabe};
use shaia_display::text::Schriften;

fn main() {
    let ziel = std::env::args().nth(1).unwrap_or("/tmp/hud_rust.png".into());
    let fonts = std::env::var("PI_FONTS").unwrap_or("../schriften".into());
    let seite: usize = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(0);
    let stil = std::env::args().nth(3).unwrap_or("glut".into());
    let mut sf = Schriften::neu(fonts);
    let w = Wetter {
        current: Aktuell { temperature_2m: 16.4, apparent_temperature: 15.2, weather_code: 2, wind_speed_10m: 12.0,
            relative_humidity_2m: 73.0, is_day: Some(1), uv_index: Some(3.0), wind_gusts_10m: Some(20.0) },
        daily: Taeglich { temperature_2m_max: vec![24.0], temperature_2m_min: vec![12.0], precipitation_probability_max: vec![Some(30.0)] },
    };
    let netz = Netz { ms: Some(4.0), offline_seit: None, speed: Some(Speed { down_mbit: 227.0, up_mbit: 70.0 }) };
    let jetzt = Local.with_ymd_and_hms(2026, 10, 18, 18, 42, 0).unwrap();
    let e = Eingabe { jetzt, wetter: Some(&w), pihole: Some((300, 120)), netz: Some(&netz), warnungen: &[], cpu: Some(66.6),
        laufzeit: "5 h 12 min".into(), seite };
    let (bild, _) = hud::zeichnen(&mut sf, &stil, 480, 320, 94, 248, &e);
    let mut grund = shaia_display::gfx::Img::gefuellt(480, 320, [40, 90, 160, 255]);
    grund.darueber(&bild, 0, 0);
    grund.speichern(std::path::Path::new(&ziel)).unwrap();
}
