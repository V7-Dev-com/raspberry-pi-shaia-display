//! Zeitmessung der Bildschritte (ohne Display): schritt, zeichnen, RGB565, Vergleich.
use chrono::{Local, TimeZone};
use shaia_display::assets::*;
use shaia_display::data::*;
use shaia_display::hud::{self, Eingabe};
use shaia_display::scene::*;
use shaia_display::text::Schriften;
use std::rc::Rc;
use std::time::Instant;

fn main() {
    let nacht: bool = std::env::args().nth(1).as_deref() == Some("nacht");
    let a = Rc::new(Assets::laden(&assets_ordner().join("shaia")).expect("Grafiken"));
    let mut sf = Schriften::neu(assets_ordner().join("fonts"));
    let stunde = if nacht { 23 } else { 14 };
    let mut sz = Szene::neu(a, Some(7), stunde, 1000.0);
    sz.nacht = nacht;
    let w = Wetter {
        current: Aktuell { temperature_2m: 16.4, apparent_temperature: 15.2, weather_code: 2, wind_speed_10m: 12.0,
            relative_humidity_2m: 73.0, is_day: Some(1), uv_index: Some(3.0), wind_gusts_10m: Some(20.0) },
        daily: Taeglich { temperature_2m_max: vec![24.0], temperature_2m_min: vec![12.0], precipitation_probability_max: vec![Some(30.0)] },
    };
    let netz = Netz { ms: Some(4.0), offline_seit: None, speed: Some(Speed { down_mbit: 227.0, up_mbit: 70.0, zeit: 0 }), pi_ip: Some("192.168.1.9".into()), router_ip: Some("192.168.1.1".into()), router_ok: None };
    let e = Eingabe { wetter_alter: 0, pihole_alter: 0, jetzt: Local.with_ymd_and_hms(2026, 10, 18, stunde, 42, 0).unwrap(), wetter: Some(&w), pihole: Some((300, 120)),
        netz: Some(&netz), warnungen: &[], cpu: Some(66.6), laufzeit: "5 h 12 min".into(), seite: 0 };
    let th = Instant::now();
    for _ in 0..10 {
        sz.statisch_setzen(hud::zeichnen(&mut sf, "glut", W as i32, H as i32, 94, 248, &e).0);
    }
    println!("HUD neu zeichnen: {:.2} ms", th.elapsed().as_secs_f64() * 100.0);
    let (mut t_schritt, mut t_zeichnen, mut t_565, mut t_diff) = (0.0, 0.0, 0.0, 0.0);
    let mut puffer = vec![0u16; W * H];
    let mut vorher = vec![0u16; W * H];
    let n = 3000;
    let mut t = 1000.0;
    for i in 0..n {
        if i % 200 == 0 {
            sz.skelette_dazu(1);
        }
        let s = Instant::now();
        sz.schritt(1.0 / 24.0, stunde, t, false, false);
        t_schritt += s.elapsed().as_secs_f64();
        let s = Instant::now();
        let b = sz.zeichnen(t);
        t_zeichnen += s.elapsed().as_secs_f64();
        let s = Instant::now();
        b.nach_rgb565(&mut puffer);
        t_565 += s.elapsed().as_secs_f64();
        let s = Instant::now();
        let mut geaendert = 0;
        for y in 0..H {
            if puffer[y * W..(y + 1) * W] != vorher[y * W..(y + 1) * W] {
                geaendert += 1;
            }
        }
        vorher.copy_from_slice(&puffer);
        std::hint::black_box(geaendert);
        t_diff += s.elapsed().as_secs_f64();
        t += 1.0 / 24.0;
    }
    let ms = |x: f64| x / n as f64 * 1000.0;
    println!("{} je Bild in ms: schritt {:.3}  zeichnen {:.3}  rgb565 {:.3}  vergleich {:.3}  gesamt {:.3}",
        if nacht { "Nacht" } else { "Tag" }, ms(t_schritt), ms(t_zeichnen), ms(t_565), ms(t_diff),
        ms(t_schritt + t_zeichnen + t_565 + t_diff));
}
