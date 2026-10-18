//! Rendert typische Szenen der Anzeige als PNG (ohne Display), wie `werkzeuge/bildschirmfotos.py`.
//! Aufruf:  PI_ASSETS=../assets cargo run --release --bin shot -- /tmp/rustbilder [stil]
use chrono::{Local, TimeZone};
use shaia_display::assets::{assets_ordner, Assets, H, W};
use shaia_display::data::*;
use shaia_display::gfx::Img;
use shaia_display::hud::{self, Eingabe};
use shaia_display::scene::*;
use shaia_display::text::Schriften;
use std::path::Path;
use std::rc::Rc;

struct Werkstatt {
    a: Rc<Assets>,
    sf: Schriften,
    stil: String,
    ziel: String,
}

fn wetter(code: i64, boeen: f64, uv: f64) -> Wetter {
    Wetter {
        current: Aktuell {
            temperature_2m: 16.4, apparent_temperature: 15.2, weather_code: code, wind_speed_10m: 12.0,
            relative_humidity_2m: 73.0, is_day: Some(1), uv_index: Some(uv), wind_gusts_10m: Some(boeen),
        },
        daily: Taeglich {
            temperature_2m_max: vec![24.0], temperature_2m_min: vec![12.0], precipitation_probability_max: vec![Some(30.0)],
        },
    }
}

impl Werkstatt {
    fn neu(&self, seed: u64, stunde: u32) -> Szene {
        let mut sz = Szene::neu(self.a.clone(), Some(seed), stunde, 5000.0);
        sz.bonus_ab = 1e9;
        sz.spaziergang_ab = 1e9;
        sz.kugel_ab = 1e9;
        sz.truhe_ab = 1e9;
        sz.truhe_fuer_stunde = stunde;
        self.oberflaeche(&mut sz, &wetter(2, 20.0, 3.0), Some(Speed { down_mbit: 227.0, up_mbit: 70.0 }), false, 18, 42, 0);
        sz
    }

    #[allow(clippy::too_many_arguments)]
    fn oberflaeche(&self, sz: &mut Szene, w: &Wetter, speed: Option<Speed>, offline: bool, std: u32, min: u32, seite: usize) {
        let mut sf = Schriften::neu(assets_ordner().join("fonts"));
        let jetzt = Local.with_ymd_and_hms(2026, 10, 18, std, min, 0).unwrap();
        let netz = if offline {
            Netz { ms: None, offline_seit: Some(Local.with_ymd_and_hms(2026, 10, 18, 18, 31, 0).unwrap()), speed: None }
        } else {
            Netz { ms: Some(5.0), offline_seit: None, speed }
        };
        let warn = hud::warnungen(Some(w));
        let e = Eingabe {
            jetzt, wetter: Some(w), pihole: Some((300, 120)), netz: Some(&netz), warnungen: &warn, cpu: Some(66.6),
            laufzeit: "5 h 12 min".into(), seite,
        };
        let _ = &self.sf;
        sz.statisch = Some(hud::zeichnen(&mut sf, &self.stil, W as i32, H as i32, 94, 248, &e).0);
    }

    fn lauf(&self, sz: &mut Szene, sekunden: f64, bis: Option<&dyn Fn(&Szene) -> bool>, stunde: u32, offline: bool) -> bool {
        let mut t = sz.jetzt;
        for _ in 0..(sekunden * 15.0) as usize {
            sz.schritt(1.0 / 15.0, stunde, t, offline, false);
            t += 1.0 / 15.0;
            if let Some(f) = bis {
                if f(sz) {
                    return true;
                }
            }
        }
        false
    }

    fn foto(&self, sz: &mut Szene, name: &str) {
        let jetzt = sz.jetzt;
        let b: Img = sz.zeichnen(jetzt);
        b.speichern(&Path::new(&self.ziel).join(name)).unwrap();
    }
}

fn main() {
    let ziel = std::env::args().nth(1).unwrap_or("/tmp/rustbilder".into());
    let stil = std::env::args().nth(2).unwrap_or("glut".into());
    std::fs::create_dir_all(&ziel).unwrap();
    let sprites = assets_ordner().join("shaia");
    let a = Rc::new(Assets::laden(&sprites).unwrap_or_else(|e| panic!("Grafiken: {e}")));
    let ws = Werkstatt { a, sf: Schriften::neu(assets_ordner().join("fonts")), stil, ziel };
    let (tag, nacht) = (14, 23);

    let mut sz = ws.neu(5, tag);
    ws.lauf(&mut sz, 3.0, None, tag, false);
    ws.foto(&mut sz, "1_ruhe.png");

    let mut sz = ws.neu(5, tag);
    sz.skelette_dazu(1);
    ws.lauf(&mut sz, 40.0, Some(&|s: &Szene| s.gegner_idx().is_some_and(|i| s.skelette[i].zustand == Zustand::Getroffen)), tag, false);
    ws.foto(&mut sz, "2_kampf.png");

    let mut sz = ws.neu(1, tag);
    sz.kugel_ab = 0.0;
    ws.lauf(&mut sz, 90.0, Some(&|s: &Szene| s.shaia.anim() == Some("ducken")), tag, false);
    ws.foto(&mut sz, "3_ausweichen.png");

    for seed in 2..60 {
        let mut sz = ws.neu(seed, tag);
        sz.kugel_ab = 0.0;
        let ok = ws.lauf(&mut sz, 240.0, Some(&|s: &Szene| matches!(s.shaia.anim(), Some("treffer_kopf" | "treffer_koerper"))), tag, false);
        if ok || seed == 59 {
            ws.foto(&mut sz, "4_treffer_von_hinten.png");
            break;
        }
    }

    let mut sz = ws.neu(5, tag);
    sz.beute_hinzu_test();
    ws.foto(&mut sz, "5_beute.png");

    let mut sz = ws.neu(5, nacht);
    sz.nacht = true;
    ws.oberflaeche(&mut sz, &wetter(2, 20.0, 3.0), Some(Speed { down_mbit: 227.0, up_mbit: 70.0 }), false, 23, 10, 3);
    sz.shaia.x = LAGER_X;
    sz.kamera = LAGER_X + 65.0 - W as f64 / 2.0;
    ws.lauf(&mut sz, 8.0, Some(&|s: &Szene| s.shaia.anim() == Some("lagerfeuer")), nacht, false);
    ws.lauf(&mut sz, 2.0, None, nacht, false);
    ws.foto(&mut sz, "6_nacht_lagerfeuer.png");

    let mut sz = ws.neu(5, tag);
    ws.oberflaeche(&mut sz, &wetter(2, 20.0, 3.0), None, true, 18, 42, 0);
    ws.lauf(&mut sz, 1.0, None, tag, true);
    ws.foto(&mut sz, "7_offline.png");

    let mut sz = ws.neu(5, tag);
    ws.oberflaeche(&mut sz, &wetter(95, 62.0, 9.0), Some(Speed { down_mbit: 227.0, up_mbit: 70.0 }), false, 18, 42, 0);
    ws.foto(&mut sz, "8_warnungen.png");

    let mut sz = ws.neu(3, tag);
    sz.truhe_ab = sz.jetzt + 1.0;
    let mut t = sz.jetzt;
    for _ in 0..15 * 90 {
        sz.schritt(1.0 / 15.0, tag, t, false, false);
        t += 1.0 / 15.0;
        sz.jetzt = t;
        if sz.shaia.anim() == Some("truhe_raus") && sz.truhe_seit.is_some_and(|s| t - s > 0.6) {
            ws.foto(&mut sz, "9_truhe.png");
            break;
        }
    }
    eprintln!("fertig: {ziel}", ziel = ws.ziel);
}
