//! Oberfläche (Kopfleiste, Fußleiste, große Wechselzeile, Warnung) als eigene Ebene über der Szene.
//! Entspricht `hud.py` und `warnungen()`/`laufzeit()` aus `ruinrunner.py`.

use crate::data::{Netz, Wetter};
use crate::gfx::Img;
use crate::text::{Art, Schriften, DEJAVU_FETT};
use chrono::{DateTime, Datelike, Local};
use std::sync::OnceLock;

pub type Rgb = [u8; 3];
pub const WARN_GELB: Rgb = [225, 145, 20];
pub const WARN_ROT: Rgb = [205, 40, 40];

/// `PI_SPRACHE=en` schaltet die Anzeige auf Englisch (Standard: Deutsch).
pub fn englisch() -> bool {
    static E: OnceLock<bool> = OnceLock::new();
    *E.get_or_init(|| std::env::var("PI_SPRACHE").map(|v| v.to_lowercase().starts_with("en")).unwrap_or(false))
}

const WOCHENTAGE_DE: [&str; 7] = ["Montag", "Dienstag", "Mittwoch", "Donnerstag", "Freitag", "Samstag", "Sonntag"];
const WOCHENTAGE_EN: [&str; 7] = ["Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday", "Sunday"];
const MONATE_DE: [&str; 12] = ["Januar", "Februar", "März", "April", "Mai", "Juni", "Juli", "August", "September",
    "Oktober", "November", "Dezember"];
const MONATE_EN: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September",
    "October", "November", "December"];

const EN_WOERTER: [(&str, &str); 24] = [
    ("sehr hoch", "very high"), ("hoch", "high"), ("luftfeuchte", "humidity"), ("gefühlt", "feels"),
    ("geblockt", "blocked"), ("läuft", "uptime"), ("offline seit", "OFFLINE since"), ("sturmböen", "gusts"),
    ("sturm", "storm"), ("hitze", "heat"), ("starkregen", "heavy rain"), ("eisregen", "freezing rain"),
    ("regen", "rain"), ("klar", "clear"), ("heiter", "fair"), ("wolkig", "cloudy"), ("bedeckt", "overcast"),
    ("nebel", "fog"), ("niesel", "drizzle"), ("schnee", "snow"), ("schauer", "showers"),
    ("gewitter", "thunderstorm"), ("anfr.", "req."), ("heute", "today"),
];

fn wort_zeichen(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Übersetzt Anzeigetexte ins Englische (nur wenn `PI_SPRACHE=en`), Groß-/Kleinschreibung bleibt erhalten.
pub fn tr(text: &str) -> String {
    if !englisch() {
        return text.to_string();
    }
    uebersetze(text)
}

fn uebersetze(text: &str) -> String {
    let zeichen: Vec<char> = text.chars().collect();
    let klein: Vec<char> = text.to_lowercase().chars().collect();
    if klein.len() != zeichen.len() {
        return text.to_string();
    }
    let mut keys: Vec<&(&str, &str)> = EN_WOERTER.iter().collect();
    keys.sort_by_key(|k| std::cmp::Reverse(k.0.chars().count()));
    let mut aus = String::new();
    let mut i = 0;
    'aussen: while i < zeichen.len() {
        let vorher_ok = i == 0 || !wort_zeichen(zeichen[i - 1]);
        if vorher_ok {
            for (de, en) in &keys {
                let d: Vec<char> = de.chars().collect();
                if klein[i..].starts_with(&d) && zeichen.get(i + d.len()).map_or(true, |c| !wort_zeichen(*c)) {
                    let orig: String = zeichen[i..i + d.len()].iter().collect();
                    let ersatz = if en.chars().any(|c| c.is_uppercase()) {
                        en.to_string()
                    } else if orig.chars().any(|c| c.is_alphabetic()) && orig.chars().filter(|c| c.is_alphabetic()).all(|c| c.is_uppercase()) {
                        en.to_uppercase()
                    } else if orig.chars().next().is_some_and(|c| c.is_uppercase()) {
                        let mut it = en.chars();
                        it.next().map(|f| f.to_uppercase().collect::<String>() + it.as_str()).unwrap_or_default()
                    } else {
                        en.to_string()
                    };
                    aus.push_str(&ersatz);
                    i += d.len();
                    continue 'aussen;
                }
            }
        }
        aus.push(zeichen[i]);
        i += 1;
    }
    aus.replace(" T ", " d ")
}

/// WMO-Wettercode -> Wort (wie `WETTER_WORT` in pixelwelt.py).
pub fn wetter_wort(code: i64) -> &'static str {
    match code {
        0 => "KLAR", 1 => "HEITER", 2 => "WOLKIG", 3 => "BEDECKT", 45 | 48 => "NEBEL",
        51 | 53 | 55 | 56 | 57 => "NIESEL", 61 | 63 => "REGEN", 65 => "STARKREGEN", 66 | 67 => "EISREGEN",
        71 | 73 | 75 | 77 | 85 | 86 => "SCHNEE", 80..=82 => "SCHAUER", 95 | 96 | 99 => "GEWITTER",
        _ => "",
    }
}

fn capitalize(s: &str) -> String {
    let mut it = s.chars();
    match it.next() {
        Some(f) => f.to_uppercase().collect::<String>() + &it.as_str().to_lowercase(),
        None => String::new(),
    }
}

pub struct Warnung {
    pub text: String,
    pub farbe: Rgb,
}

/// Aktuell gültige Wetter-Warnungen.
pub fn warnungen(w: Option<&Wetter>) -> Vec<Warnung> {
    let Some(w) = w else { return vec![] };
    let c = &w.current;
    let mut v = Vec::new();
    let mut add = |t: String, f: Rgb| v.push(Warnung { text: t, farbe: f });
    let code = c.weather_code;
    if [95, 96, 99].contains(&code) {
        add("⚡ GEWITTER".into(), WARN_ROT);
    }
    let boeen = c.wind_gusts_10m.unwrap_or(0.0);
    if boeen >= 75.0 {
        add(format!("⚠ STURM {boeen:.0} km/h"), WARN_ROT);
    } else if boeen >= 50.0 {
        add(format!("⚠ Sturmböen {boeen:.0} km/h"), WARN_GELB);
    }
    let uv = c.uv_index.unwrap_or(0.0);
    if w.ist_tag() && uv >= 8.0 {
        add(format!("☀ UV {uv:.0} sehr hoch"), WARN_ROT);
    } else if w.ist_tag() && uv >= 6.0 {
        add(format!("☀ UV {uv:.0} hoch"), WARN_GELB);
    }
    let temp = c.temperature_2m;
    if temp >= 38.0 {
        add(format!("⚠ HITZE {temp:.0}°"), WARN_ROT);
    } else if temp >= 35.0 {
        add(format!("⚠ Hitze {temp:.0}°"), WARN_GELB);
    }
    if [65, 82].contains(&code) {
        add("⚠ Starkregen".into(), WARN_GELB);
    }
    if [71, 73, 75, 77, 85, 86].contains(&code) {
        add("❄ Schnee".into(), WARN_GELB);
    }
    v
}

#[derive(Clone, Copy, PartialEq)]
pub enum Deko {
    V7,
    Glut,
    Retro,
    Klar,
}

pub struct Stil {
    pub uhr: (&'static str, i32),
    pub uhr_farbe: Rgb,
    pub glut: Option<Rgb>,
    pub datum: (&'static str, i32),
    pub datum_farbe: Rgb,
    pub gross_datum: bool,
    pub wert: (&'static str, i32),
    pub wert_farbe: Rgb,
    pub label: (&'static str, i32),
    pub label_farbe: Rgb,
    pub label_gross: bool,
    pub sperren: i32,
    pub zeile: (&'static str, i32),
    pub zeile_farbe: Rgb,
    pub zeile2_farbe: Rgb,
    pub gruen: Rgb,
    pub rot: Rgb,
    pub leiste_oben: (Rgb, Rgb),
    pub leiste_unten: Rgb,
    pub linie: Rgb,
    pub deko: Deko,
    pub kontur: i32,
    pub schatten: Option<Rgb>,
}

pub fn stil(name: &str) -> Stil {
    match name {
        "v7" => Stil {
            uhr: ("SairaCondensed-ExtraBold.ttf", 68), uhr_farbe: [255, 111, 9], glut: Some([255, 111, 9]),
            datum: ("SairaCondensed-Bold.ttf", 21), datum_farbe: [255, 255, 255], gross_datum: true,
            wert: ("SairaCondensed-ExtraBold.ttf", 36), wert_farbe: [255, 255, 255],
            label: ("JetBrainsMono[wght].ttf", 12), label_farbe: [255, 111, 9], label_gross: true, sperren: 1,
            zeile: ("SairaCondensed-Bold.ttf", 19), zeile_farbe: [255, 255, 255], zeile2_farbe: [175, 180, 190],
            gruen: [255, 140, 40], rot: [255, 59, 48], leiste_oben: ([10, 10, 13], [18, 19, 24]),
            leiste_unten: [10, 10, 13], linie: [255, 111, 9], deko: Deko::V7, kontur: 0, schatten: None,
        },
        "glut" => Stil {
            uhr: ("Rajdhani-Bold.ttf", 76), uhr_farbe: [255, 200, 85], glut: Some([239, 125, 87]),
            datum: ("Rajdhani-SemiBold.ttf", 23), datum_farbe: [255, 236, 178], gross_datum: false,
            wert: ("Rajdhani-Bold.ttf", 38), wert_farbe: [255, 214, 125],
            label: ("Rajdhani-SemiBold.ttf", 17), label_farbe: [255, 184, 72], label_gross: false, sperren: 0,
            zeile: ("Rajdhani-SemiBold.ttf", 21), zeile_farbe: [255, 236, 180], zeile2_farbe: [240, 190, 105],
            gruen: [167, 240, 112], rot: [255, 90, 70], leiste_oben: ([14, 8, 5], [30, 16, 9]),
            leiste_unten: [16, 9, 6], linie: [169, 120, 52], deko: Deko::Glut, kontur: 0, schatten: Some([0, 0, 0]),
        },
        "retro" => Stil {
            uhr: ("Silkscreen-Bold.ttf", 48), uhr_farbe: [255, 138, 30], glut: None,
            datum: ("Silkscreen-Regular.ttf", 16), datum_farbe: [255, 255, 255], gross_datum: true,
            wert: ("Silkscreen-Bold.ttf", 32), wert_farbe: [255, 255, 255],
            label: ("Silkscreen-Regular.ttf", 16), label_farbe: [255, 138, 30], label_gross: true, sperren: 0,
            zeile: ("Silkscreen-Regular.ttf", 16), zeile_farbe: [255, 255, 255], zeile2_farbe: [200, 205, 220],
            gruen: [167, 240, 112], rot: [255, 80, 80], leiste_oben: ([17, 19, 31], [17, 19, 31]),
            leiste_unten: [17, 19, 31], linie: [255, 138, 30], deko: Deko::Retro, kontur: 0,
            schatten: Some([60, 20, 0]),
        },
        _ => Stil {
            uhr: (DEJAVU_FETT, 64), uhr_farbe: [255, 255, 255], glut: None,
            datum: (DEJAVU_FETT, 20), datum_farbe: [255, 255, 255], gross_datum: false,
            wert: (DEJAVU_FETT, 32), wert_farbe: [255, 255, 255],
            label: (DEJAVU_FETT, 16), label_farbe: [228, 234, 245], label_gross: false, sperren: 0,
            zeile: (DEJAVU_FETT, 17), zeile_farbe: [255, 255, 255], zeile2_farbe: [228, 234, 245],
            gruen: [120, 255, 140], rot: [255, 110, 110], leiste_oben: ([2, 3, 8], [6, 9, 22]),
            leiste_unten: [3, 5, 12], linie: [60, 110, 200], deko: Deko::Klar, kontur: 1, schatten: None,
        },
    }
}

pub fn stil_bekannt(name: &str) -> bool {
    ["v7", "glut", "retro", "klar"].contains(&name)
}

fn rgba(c: Rgb) -> [u8; 4] {
    [c[0], c[1], c[2], 255]
}

fn deko_linie(d: &mut Img, y: i32, s: &Stil, w: i32) {
    let f = rgba(s.linie);
    match s.deko {
        Deko::V7 => {
            d.linie_waagerecht(y, 0, w, f, 2);
            for (x0, r) in [(0.0f32, 1.0f32), (w as f32, -1.0)] {
                let (yf, x) = (y as f32, x0);
                d.polygon(&[(x, yf - 5.0), (x + r * 70.0, yf - 5.0), (x + r * 78.0, yf + 1.0), (x, yf + 1.0)], f, None);
            }
        }
        Deko::Glut => {
            d.linie_waagerecht(y - 2, 0, w, f, 1);
            d.linie_waagerecht(y + 1, 0, w, [120, 70, 30, 255], 1);
            let mx = (w / 2) as f32;
            let yf = y as f32;
            d.polygon(&[(mx, yf - 3.0), (mx + 4.0, yf), (mx, yf + 3.0), (mx - 4.0, yf)], [239, 125, 87, 255], Some(f));
        }
        Deko::Retro => {
            let mut x = 0;
            while x < w {
                d.rechteck(x, y - 2, x + 4, y + 1, f);
                x += 8;
            }
        }
        Deko::Klar => d.linie_waagerecht(y, 0, w, f, 2),
    }
}

/// Gradzeichen als Ring, ca. 28 % der Versalhöhe, 3 px neben der Zahl.
fn grad(d: &mut Img, x_rechts: f32, kappe_oben: i32, kappe_px: i32, farbe: Rgb) {
    let r = ((kappe_px as f64 * 0.14).round_ties_even() as i32).max(4);
    let (x0, y0) = (x_rechts as i32 + 3, kappe_oben);
    d.ellipse_ring(x0, y0, x0 + 2 * r, y0 + 2 * r, rgba(farbe), (r / 2).max(2));
}

fn gross_wenn(s: &Stil, t: &str) -> String {
    if s.gross_datum { t.to_uppercase() } else { t.to_string() }
}

fn runden(v: f64) -> i64 {
    v.round_ties_even() as i64
}

pub struct Eingabe<'a> {
    pub jetzt: DateTime<Local>,
    pub wetter: Option<&'a Wetter>,
    /// (Anfragen heute, geblockt heute)
    pub pihole: Option<(i64, i64)>,
    pub netz: Option<&'a Netz>,
    pub warnungen: &'a [Warnung],
    pub cpu: Option<f64>,
    pub laufzeit: String,
    pub seite: usize,
}

/// Einträge der großen Wechselzeile unten: (Text, Farbe). Offline verdrängt alles.
pub fn laufschrift(s: &Stil, e: &Eingabe) -> Vec<(String, Rgb)> {
    let (ms, offline_seit, speed) = match e.netz {
        Some(n) => (n.ms, n.offline_seit, n.speed.as_ref()),
        None => (None, None, None),
    };
    if e.netz.is_some() && ms.is_none() {
        if let Some(seit) = offline_seit {
            return vec![(tr(&format!("OFFLINE seit {}", seit.format("%H:%M"))), s.rot)];
        }
    }
    let mut v: Vec<(String, Rgb)> = Vec::new();
    for w in e.warnungen {
        let t = w.text.trim_start_matches(['⚠', '☀', '⚡', '❄', ' ']);
        v.push((gross_wenn(s, t), if w.farbe == WARN_ROT { s.rot } else { [255, 190, 60] }));
    }
    if let Some(w) = e.wetter {
        let (c, t) = (&w.current, &w.daily);
        let wort = capitalize(match wetter_wort(c.weather_code) {
            "" => "Wetter",
            x => x,
        });
        let (mn, mx) = (t.temperature_2m_min.first().copied().unwrap_or(0.0), t.temperature_2m_max.first().copied().unwrap_or(0.0));
        v.push((gross_wenn(s, &format!("{wort} {}–{}°", runden(mn), runden(mx))), s.zeile_farbe));
        if let Some(Some(regen)) = t.precipitation_probability_max.first() {
            if *regen >= 20.0 {
                v.push((gross_wenn(s, &format!("Regen {} %", runden(*regen))), s.zeile_farbe));
            }
        }
        v.push((
            gross_wenn(s, &format!("Gefühlt {}° · Wind {}", runden(c.apparent_temperature), runden(c.wind_speed_10m))),
            s.zeile_farbe,
        ));
        if w.ist_tag() && c.uv_index.is_some_and(|u| u >= 1.0) {
            v.push((gross_wenn(s, &format!("UV-Index {:.0}", c.uv_index.unwrap())), s.zeile_farbe));
        }
    }
    if let Some(ms) = ms {
        v.push((gross_wenn(s, &format!("Ping {ms:.0} ms")), s.gruen));
        if let Some(sp) = speed {
            v.push((gross_wenn(s, &format!("↓{:.0}  ↑{:.0} Mbit/s", sp.down_mbit, sp.up_mbit)), s.gruen));
        }
    }
    if let Some(n) = e.netz {
        if let Some(r) = &n.router_ip {
            v.push((gross_wenn(s, &format!("Router {r}")), s.zeile2_farbe));
        }
        if let Some(p) = &n.pi_ip {
            v.push((gross_wenn(s, &format!("Pi {p}")), s.zeile2_farbe));
        }
    }
    if let Some((_, geblockt)) = e.pihole {
        v.push((gross_wenn(s, &format!("Pi-hole · {geblockt} geblockt")), s.zeile_farbe));
    }
    if let Some(cpu) = e.cpu {
        v.push((gross_wenn(s, &format!("CPU {cpu:.0} °C")), s.zeile2_farbe));
    }
    v.push((gross_wenn(s, &format!("Läuft {}", e.laufzeit)), s.zeile2_farbe));
    v.into_iter().map(|(t, f)| (tr(&t), f)).collect()
}

/// Fernansicht: große Uhr, große Werte, unten eine große Zeile, die mit `seite` wechselt.
/// Liefert die Ebene und die Zahl der Seiten.
pub fn zeichnen(sf: &mut Schriften, stil_name: &str, w: i32, h: i32, kopf: i32, fuss: i32, e: &Eingabe) -> (Img, usize) {
    let s = stil(if stil_bekannt(stil_name) { stil_name } else { "klar" });
    let mut bild = Img::neu(w as usize, h as usize);
    let (o1, o2) = s.leiste_oben;
    for y in 0..kopf {
        let t = y as f32 / kopf as f32;
        let f: Vec<u8> = (0..3).map(|k| (o1[k] as f32 + (o2[k] as f32 - o1[k] as f32) * t) as u8).collect();
        bild.linie_waagerecht(y, 0, w, [f[0], f[1], f[2], 255], 1);
    }
    bild.rechteck(0, fuss + 4, w, h, rgba(s.leiste_unten));
    deko_linie(&mut bild, kopf - 2, &s, w);
    deko_linie(&mut bild, fuss + 4, &s, w);
    let art = Art { sperren: 0, kontur: s.kontur, kontur_farbe: None, schatten: s.schatten };
    let lab = Art { sperren: s.sperren, kontur: s.kontur, kontur_farbe: None, schatten: None };

    // Uhr: Großbuchstaben 56 px hoch
    let uhr = e.jetzt.format("%H:%M").to_string();
    let uhr_datei = s.uhr.0;
    let mut gr = sf.groesse_fuer(uhr_datei, 56);
    while gr > 20 && sf.breite(&uhr, uhr_datei, gr, 0) > (w - 2 * 122) as f32 {
        gr -= 1;
    }
    let (_, versatz) = sf.kappen_hoehe(uhr_datei, gr);
    let uy = (6 - versatz) as f32;
    if let Some(glut) = s.glut {
        let mut schicht = Img::neu(w as usize, h as usize);
        sf.schreiben(&mut schicht, (w / 2) as f32, uy, &uhr, uhr_datei, gr, glut, 'm', Art::default(), None);
        let weich = schicht.weichzeichnen(7.0);
        for _ in 0..2 {
            bild.darueber(&weich, 0, 0);
        }
        sf.schreiben(&mut bild, (w / 2) as f32, uy, &uhr, uhr_datei, gr, s.uhr_farbe, 'm', Art::default(), None);
    } else {
        let a = Art { kontur: s.kontur * 2, ..art };
        sf.schreiben(&mut bild, (w / 2) as f32, uy, &uhr, uhr_datei, gr, s.uhr_farbe, 'm', a, None);
    }
    let (wt, mo) = if englisch() { (&WOCHENTAGE_EN, &MONATE_EN) } else { (&WOCHENTAGE_DE, &MONATE_DE) };
    let datum = format!(
        "{}, {}. {}",
        wt[e.jetzt.weekday().num_days_from_monday() as usize],
        e.jetzt.day(),
        mo[e.jetzt.month0() as usize]
    );
    let datum = if s.gross_datum { datum.to_uppercase() } else { datum };
    sf.setzen(&mut bild, (w / 2) as f32, 69, &datum, s.datum.0, 16, s.datum_farbe, 'm', Some(210.0), art);

    // Temperatur links, Luftfeuchte rechts
    if let Some(wet) = e.wetter {
        let c = &wet.current;
        let zahl = format!("{}", runden(c.temperature_2m));
        sf.setzen(&mut bild, 12.0, 8, &zahl, s.wert.0, 32, s.wert_farbe, 'l', None, art);
        let gz = sf.groesse_fuer(s.wert.0, 32);
        let bz = sf.breite(&zahl, s.wert.0, gz, 0);
        grad(&mut bild, 12.0 + bz, 8, 32, s.wert_farbe);
        sf.setzen(&mut bild, (w - 12) as f32, 8, &format!("{}%", runden(c.relative_humidity_2m)), s.wert.0, 32,
            s.wert_farbe, 'r', None, art);
        let klein = |t: &str| if s.label_gross { t.to_uppercase() } else { t.to_string() };
        if !e.warnungen.is_empty() {
            // Warnung ersetzt das Wetterwort unter der Temperatur (alle stehen groß in der Wechselzeile)
            let wichtig = e.warnungen.iter().find(|w| w.farbe == WARN_ROT).unwrap_or(&e.warnungen[0]);
            let mut t = klein(&tr(wichtig.text.trim_start_matches(['⚠', '☀', '⚡', '❄', ' '])));
            t = t.replace("THUNDERSTORM", "THUNDER").replace("Thunderstorm", "Thunder");
            if e.warnungen.len() > 1 {
                t += &format!(" +{}", e.warnungen.len() - 1);
            }
            let (ix, iy, dunkel) = (13, 51, [10, 7, 5, 255]);
            let fw = rgba(wichtig.farbe);
            bild.polygon(&[(ix as f32 + 6.0, iy as f32), (ix as f32 + 12.0, iy as f32 + 12.0), (ix as f32, iy as f32 + 12.0)], fw, None);
            bild.rechteck(ix + 5, iy + 4, ix + 7, iy + 8, dunkel);
            bild.rechteck(ix + 5, iy + 9, ix + 7, iy + 10, dunkel);
            sf.setzen(&mut bild, (ix + 18) as f32, 50, &t, s.label.0, 14, wichtig.farbe, 'l', Some(100.0), lab);
        } else {
            let wort = klein(&tr(&capitalize(wetter_wort(c.weather_code))));
            sf.setzen(&mut bild, 13.0, 50, &wort, s.label.0, 14, s.label_farbe, 'l', Some(118.0), lab);
        }
        sf.setzen(&mut bild, (w - 12) as f32, 50, &klein(&tr("Luftfeuchte")), s.label.0, 14, s.label_farbe, 'r',
            Some(118.0), lab);
    }

    // große Wechselzeile unten: Versalhöhe 34 px, nie unter 30 px
    let eintraege = laufschrift(&s, e);
    let (text, farbe) = &eintraege[e.seite % eintraege.len()];
    let band = h - (fuss + 6);
    let datei = s.zeile.0;
    let mut kappe = 34;
    while kappe > 30 && {
        let g = sf.groesse_fuer(datei, kappe);
        sf.breite(text, datei, g, 0) > (w - 20) as f32
    } {
        kappe -= 1;
    }
    sf.setzen(&mut bild, (w / 2) as f32, fuss + 6 + (band - kappe) / 2, text, datei, kappe, *farbe, 'm', None, art);
    (bild, eintraege.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::data::{Aktuell, Taeglich};

    fn wetter(code: i64, boeen: f64, uv: f64, temp: f64) -> Wetter {
        Wetter {
            current: Aktuell {
                temperature_2m: temp, apparent_temperature: temp - 1.0, weather_code: code, wind_speed_10m: 12.0,
                relative_humidity_2m: 70.0, is_day: Some(1), uv_index: Some(uv), wind_gusts_10m: Some(boeen),
            },
            daily: Taeglich {
                temperature_2m_max: vec![24.0], temperature_2m_min: vec![12.0], precipitation_probability_max: vec![Some(10.0)],
            },
        }
    }

    #[test]
    fn warnungen_nach_schwellen() {
        assert!(warnungen(Some(&wetter(1, 10.0, 3.0, 20.0))).is_empty());
        let w = warnungen(Some(&wetter(95, 62.0, 9.0, 36.0)));
        let texte: Vec<_> = w.iter().map(|w| w.text.as_str()).collect();
        assert_eq!(texte.len(), 4);
        assert!(texte[0].contains("GEWITTER") && texte[1].contains("Sturmböen") && texte[2].contains("sehr hoch"));
        assert_eq!(w[2].farbe, WARN_ROT);
        assert_eq!(w[3].farbe, WARN_GELB);
    }

    #[test]
    fn uebersetzung_behaelt_schreibweise() {
        assert_eq!(uebersetze("Gefühlt 19° · Wind 12"), "Feels 19° · Wind 12");
        assert_eq!(uebersetze("GEWITTER"), "THUNDERSTORM");
        assert_eq!(uebersetze("Pi-hole · 53 geblockt"), "Pi-hole · 53 blocked");
        assert_eq!(uebersetze("Läuft 3 T 2 h"), "Uptime 3 d 2 h");
        assert_eq!(uebersetze("Heiter"), "Fair");
        assert_eq!(uebersetze("UV 9 sehr hoch"), "UV 9 very high");
        assert_eq!(uebersetze("OFFLINE seit 18:31"), "OFFLINE since 18:31");
    }

    #[test]
    fn wechselzeile_hat_erwartete_eintraege() {
        let s = stil("glut");
        let w = wetter(2, 10.0, 3.0, 20.0);
        let netz = Netz { ms: Some(4.2), offline_seit: None, speed: Some(crate::data::Speed { down_mbit: 227.3, up_mbit: 70.1 }), pi_ip: Some("192.168.1.9".into()), router_ip: Some("192.168.1.1".into()) };
        let e = Eingabe {
            jetzt: Local::now(), wetter: Some(&w), pihole: Some((100, 55)), netz: Some(&netz), warnungen: &[],
            cpu: Some(66.6), laufzeit: "5 h 12 min".into(), seite: 0,
        };
        let z = laufschrift(&s, &e);
        let t: Vec<&str> = z.iter().map(|x| x.0.as_str()).collect();
        assert_eq!(t[0], "Wolkig 12–24°");
        assert!(t.contains(&"Ping 4 ms") && t.contains(&"↓227  ↑70 Mbit/s") && t.contains(&"Pi-hole · 55 geblockt"));
        assert!(t.contains(&"CPU 67 °C") && t.contains(&"Läuft 5 h 12 min"));
        assert!(t.contains(&"Router 192.168.1.1") && t.contains(&"Pi 192.168.1.9"));
    }

    #[test]
    fn offline_verdraengt_alles() {
        let s = stil("glut");
        let netz = Netz { ms: None, offline_seit: Some(Local::now()), speed: None, pi_ip: None, router_ip: None };
        let e = Eingabe {
            jetzt: Local::now(), wetter: None, pihole: None, netz: Some(&netz), warnungen: &[], cpu: None,
            laufzeit: String::new(), seite: 0,
        };
        let z = laufschrift(&s, &e);
        assert_eq!(z.len(), 1);
        assert!(z[0].0.starts_with("OFFLINE seit "));
    }

    #[test]
    fn oberflaeche_wird_gezeichnet() {
        let mut sf = Schriften::neu("/nicht/da");
        let w = wetter(1, 10.0, 3.0, 17.0);
        let e = Eingabe {
            jetzt: Local::now(), wetter: Some(&w), pihole: Some((10, 5)), netz: None, warnungen: &[], cpu: None,
            laufzeit: "1 h 0 min".into(), seite: 0,
        };
        let (bild, n) = zeichnen(&mut sf, "glut", 480, 320, 94, 248, &e);
        assert!(n >= 3);
        assert_eq!((bild.w, bild.h), (480, 320));
        assert_eq!(bild.px(240, 200)[3], 0, "die Szene bleibt frei");
        assert_eq!(bild.px(5, 5)[3], 255, "Kopfleiste deckt");
    }
}
