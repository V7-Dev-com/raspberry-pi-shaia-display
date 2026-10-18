//! Schrift: TTF-Dateien laden, mit Ersatzschrift für fehlende Zeichen, Maße wie bei PIL
//! (Größe = Em in Pixeln, ganzzahlige Zeichenabstände), Text mit Anker, Schatten und Kontur.

use crate::gfx::Img;
use ab_glyph::{point, Font, FontVec, PxScale, ScaleFont};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

pub const DEJAVU_FETT: &str = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf";
pub const DEJAVU: &str = "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf";

/// Zeichen, die diese Schrift laut Entwurf immer aus der Ersatzschrift nehmen soll.
fn ersatz_immer(datei: &str) -> &'static str {
    if datei.starts_with("Silkscreen") { "°%" } else { "" }
}

pub struct Schriften {
    ordner: PathBuf,
    cache: HashMap<String, Option<Rc<FontVec>>>,
}

/// Eine Schrift in einer Pixelgröße.
#[derive(Clone)]
pub struct Sz {
    pub font: Rc<FontVec>,
    pub scale: PxScale,
    pub groesse: i32,
}

impl Sz {
    pub fn ascent(&self) -> f32 {
        self.font.as_scaled(self.scale).ascent().ceil()
    }

    /// Zeichenvorschub in Pixeln (gebrochen wie bei PIL; gezeichnet wird auf ganze Pixel gerundet).
    pub fn vorschub(&self, c: char) -> f32 {
        let f = self.font.as_scaled(self.scale);
        f.h_advance(f.glyph_id(c))
    }

    pub fn kerning(&self, a: char, b: char) -> f32 {
        let f = self.font.as_scaled(self.scale);
        f.kern(f.glyph_id(a), f.glyph_id(b))
    }

    pub fn hat(&self, c: char) -> bool {
        self.font.glyph_id(c).0 != 0
    }
}

impl Schriften {
    pub fn neu(ordner: impl Into<PathBuf>) -> Schriften {
        Schriften { ordner: ordner.into(), cache: HashMap::new() }
    }

    fn laden(&mut self, datei: &str) -> Rc<FontVec> {
        let pfad = if datei.starts_with('/') { PathBuf::from(datei) } else { self.ordner.join(datei) };
        let schluessel = pfad.to_string_lossy().into_owned();
        if let Some(Some(f)) = self.cache.get(&schluessel) {
            return f.clone();
        }
        let font = std::fs::read(&pfad)
            .ok()
            .and_then(|b| FontVec::try_from_vec(b).ok())
            .map(Rc::new);
        self.cache.insert(schluessel.clone(), font.clone());
        match font {
            Some(f) => f,
            None => {
                if datei != DEJAVU_FETT && datei != DEJAVU {
                    // Schrift fehlt (liegt nicht im Repo): Ersatzschrift
                    self.laden(DEJAVU_FETT)
                } else {
                    panic!("keine Schrift gefunden: {datei}");
                }
            }
        }
    }

    pub fn sz(&mut self, datei: &str, groesse: i32) -> Sz {
        let font = self.laden(datei);
        let h = font.height_unscaled();
        let u = font.units_per_em().unwrap_or(h);
        Sz { scale: PxScale::from(groesse as f32 * h / u), font, groesse }
    }

    /// Text in Abschnitte zerlegen: (Teiltext, nutzt_Ersatzschrift).
    fn laeufe(&mut self, text: &str, datei: &str, groesse: i32) -> Vec<(String, bool)> {
        let haupt = self.sz(datei, groesse);
        let immer = ersatz_immer(datei);
        let mut v: Vec<(String, bool)> = Vec::new();
        for c in text.chars() {
            let ersatz = immer.contains(c) || !haupt.hat(c);
            match v.last_mut() {
                Some(l) if l.1 == ersatz => l.0.push(c),
                _ => v.push((c.to_string(), ersatz)),
            }
        }
        v
    }

    fn abschnitt(&mut self, datei: &str, groesse: i32, ersatz: bool) -> Sz {
        if ersatz { self.sz(DEJAVU_FETT, (groesse as f32 * 0.8) as i32) } else { self.sz(datei, groesse) }
    }

    fn laenge(&self, s: &Sz, text: &str) -> f32 {
        let mut b = 0.0;
        let mut vor: Option<char> = None;
        for c in text.chars() {
            if let Some(v) = vor {
                b += s.kerning(v, c);
            }
            b += s.vorschub(c);
            vor = Some(c);
        }
        b
    }

    /// Breite des Textes samt Ersatzschrift und Sperrung.
    pub fn breite(&mut self, text: &str, datei: &str, groesse: i32, sperren: i32) -> f32 {
        let mut b = 0.0;
        for (teil, ersatz) in self.laeufe(text, datei, groesse) {
            let s = self.abschnitt(datei, groesse, ersatz);
            b += self.laenge(&s, &teil) + (sperren * teil.chars().count() as i32) as f32;
        }
        b
    }

    /// Höhe eines Großbuchstabens und sein Abstand zur Text-Oberkante (wie PIL getbbox("H")).
    /// PIL setzt die Höhe durch Hinting auf ganze Pixel; `floor(Höhe + 0,8)` trifft das in allen
    /// gemessenen Größen (Rajdhani 14 bis 100 px).
    pub fn kappen_hoehe(&mut self, datei: &str, groesse: i32) -> (i32, i32) {
        let s = self.sz(datei, groesse);
        match s.font.outline(s.font.glyph_id('H')) {
            Some(o) => {
                let upem = s.font.units_per_em().unwrap_or(1000.0);
                let cap = (o.bounds.max.y - o.bounds.min.y).abs() * groesse as f32 / upem;
                let h = (cap + 0.8).floor() as i32;
                (h, s.ascent() as i32 - h)
            }
            None => (groesse, 0),
        }
    }

    /// Schriftgröße, bei der Großbuchstaben `kappe_px` hoch sind.
    pub fn groesse_fuer(&mut self, datei: &str, kappe_px: i32) -> i32 {
        let (h, _) = self.kappen_hoehe(datei, 100);
        ((100.0 * kappe_px as f32 / h as f32).round() as i32).max(8)
    }
}

/// Zeichenoptionen für `schreiben`.
#[derive(Clone, Copy, Default)]
pub struct Art {
    pub sperren: i32,
    pub kontur: i32,
    pub kontur_farbe: Option<[u8; 3]>,
    pub schatten: Option<[u8; 3]>,
}

struct Maske {
    x0: i32,
    y0: i32,
    w: usize,
    h: usize,
    d: Vec<u8>,
}

impl Maske {
    fn neu(x0: i32, y0: i32, w: usize, h: usize) -> Maske {
        Maske { x0, y0, w, h, d: vec![0; w * h] }
    }

    fn aufdecken(&mut self, x: i32, y: i32, c: f32) {
        let (ix, iy) = (x - self.x0, y - self.y0);
        if ix >= 0 && iy >= 0 && (ix as usize) < self.w && (iy as usize) < self.h {
            let i = iy as usize * self.w + ix as usize;
            self.d[i] = self.d[i].max((c * 255.0 + 0.5) as u8);
        }
    }

    /// Verdickt die Maske um `r` Pixel (Kreis) – für die Kontur.
    fn verdickt(&self, r: i32) -> Maske {
        let mut m = Maske::neu(self.x0 - r, self.y0 - r, self.w + 2 * r as usize, self.h + 2 * r as usize);
        for y in 0..self.h as i32 {
            for x in 0..self.w as i32 {
                let c = self.d[y as usize * self.w + x as usize];
                if c == 0 {
                    continue;
                }
                for dy in -r..=r {
                    for dx in -r..=r {
                        if dx * dx + dy * dy <= r * r + r {
                            let i = (y + dy + r) as usize * m.w + (x + dx + r) as usize;
                            m.d[i] = m.d[i].max(c);
                        }
                    }
                }
            }
        }
        m
    }
}

/// Tinte mit Maske auf das Bild bringen. Echte Alpha-Mischung (Farbe bleibt voll, nur die Deckung
/// folgt der Maske) und eine leichte Aufhellung der Randdeckung: die ungehintete Rasterung wirkt
/// sonst dünner und blasser als PIL/FreeType.
fn tinte(img: &mut Img, m: &Maske, f: [u8; 4], dx: i32, dy: i32) {
    let tabelle = deckung_tabelle();
    for y in 0..m.h {
        for x in 0..m.w {
            let c = m.d[y * m.w + x];
            if c == 0 {
                continue;
            }
            let (tx, ty) = (m.x0 + x as i32 + dx, m.y0 + y as i32 + dy);
            if tx < 0 || ty < 0 || tx as usize >= img.w || ty as usize >= img.h {
                continue;
            }
            let i = (ty as usize * img.w + tx as usize) * 4;
            let a = ((tabelle[c as usize] as u32 * f[3] as u32 + 127) / 255) as u8;
            crate::gfx::mische(&mut img.d[i..i + 4], &[f[0], f[1], f[2], a]);
        }
    }
}

/// Deckung^0,45: schwache Randpixel werden kräftiger, volle bleiben voll.
fn deckung_tabelle() -> &'static [u8; 256] {
    static T: std::sync::OnceLock<[u8; 256]> = std::sync::OnceLock::new();
    T.get_or_init(|| {
        let mut t = [0u8; 256];
        for (i, v) in t.iter_mut().enumerate() {
            *v = ((i as f32 / 255.0).powf(0.45) * 255.0 + 0.5) as u8;
        }
        t
    })
}

impl Schriften {
    fn maske(&mut self, text: &str, datei: &str, groesse: i32, x: f32, y: f32, art: &Art) -> Maske {
        let basis = self.sz(datei, groesse).ascent();
        let breit = self.breite(text, datei, groesse, art.sperren).ceil() as usize + 8 + groesse as usize;
        let hoch = (groesse as f32 * 2.0) as usize + 8;
        let mut m = Maske::neu(x.floor() as i32 - 2, y.floor() as i32 - 2, breit + 4, hoch + 4);
        let mut pen = x;
        for (teil, ersatz) in self.laeufe(text, datei, groesse) {
            let s = self.abschnitt(datei, groesse, ersatz);
            let versatz = if ersatz { basis - s.ascent() } else { 0.0 };
            let grund = (y + basis - versatz).round();
            let mut vor: Option<char> = None;
            for c in teil.chars() {
                if let Some(v) = vor {
                    pen += s.kerning(v, c);
                }
                let g = s.font.glyph_id(c).with_scale_and_position(s.scale, point(pen.round(), grund));
                if let Some(o) = s.font.outline_glyph(g) {
                    let b = o.px_bounds();
                    let (bx, by) = (b.min.x as i32, b.min.y as i32);
                    o.draw(|gx, gy, cov| m.aufdecken(bx + gx as i32, by + gy as i32, cov));
                }
                pen += s.vorschub(c) + art.sperren as f32;
                vor = Some(c);
            }
        }
        m
    }

    /// Text zeichnen. `anker`: 'l' links, 'm' Mitte, 'r' rechts (x), y ist die Oberkante der Zeile.
    #[allow(clippy::too_many_arguments)]
    pub fn schreiben(
        &mut self, img: &mut Img, x: f32, y: f32, text: &str, datei: &str, groesse: i32, farbe: [u8; 3],
        anker: char, art: Art, max_breite: Option<f32>,
    ) {
        let mut groesse = groesse;
        if let Some(mb) = max_breite {
            while groesse > 8 && self.breite(text, datei, groesse, art.sperren) > mb {
                groesse -= 1;
            }
        }
        let gesamt = self.breite(text, datei, groesse, art.sperren);
        let x = match anker {
            'm' => x - gesamt / 2.0,
            'r' => x - gesamt,
            _ => x,
        };
        let m = self.maske(text, datei, groesse, x, y, &art);
        if let Some(s) = art.schatten {
            tinte(img, &m, [s[0], s[1], s[2], 255], 2, 2);
        }
        if art.kontur > 0 {
            let k = art.kontur_farbe.unwrap_or([5, 8, 20]);
            tinte(img, &m.verdickt(art.kontur), [k[0], k[1], k[2], 255], 0, 0);
        }
        tinte(img, &m, [farbe[0], farbe[1], farbe[2], 255], 0, 0);
    }

    /// Text so setzen, dass die Oberkante der Großbuchstaben bei `kappe_oben` liegt.
    #[allow(clippy::too_many_arguments)]
    pub fn setzen(
        &mut self, img: &mut Img, x: f32, kappe_oben: i32, text: &str, datei: &str, kappe_px: i32,
        farbe: [u8; 3], anker: char, max_breite: Option<f32>, art: Art,
    ) -> i32 {
        let mut gr = self.groesse_fuer(datei, kappe_px);
        if let Some(mb) = max_breite {
            while gr > 8 && self.breite(text, datei, gr, art.sperren) > mb {
                gr -= 1;
            }
        }
        let (_, versatz) = self.kappen_hoehe(datei, gr);
        self.schreiben(img, x, (kappe_oben - versatz) as f32, text, datei, gr, farbe, anker, art, None);
        gr
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn schriften() -> Schriften {
        Schriften::neu("/nicht/da")
    }

    #[test]
    fn fehlende_schrift_faellt_auf_dejavu_zurueck() {
        let mut s = schriften();
        let b = s.breite("Hallo", "gibtsnicht.ttf", 20, 0);
        assert!(b > 30.0 && b < 120.0, "{b}");
    }

    #[test]
    fn kappenhoehe_passt_zur_wunschgroesse() {
        let mut s = schriften();
        let gr = s.groesse_fuer("x.ttf", 34);
        let (h, _) = s.kappen_hoehe("x.ttf", gr);
        assert!((h - 34).abs() <= 1, "{h}");
    }

    #[test]
    fn schreiben_malt_pixel_in_die_naehe_des_ankers() {
        let mut s = schriften();
        let mut bild = Img::neu(200, 60);
        s.setzen(&mut bild, 100.0, 10, "12:34", "x.ttf", 30, [255, 255, 255], 'm', None, Art::default());
        let (x0, y0, x1, y1) = bild.inhalt().unwrap();
        assert!(x0 > 20 && x1 < 180, "{x0} {x1}");
        assert!(y0 >= 9 && y1 <= 45, "{y0} {y1}");
        assert!((((x0 + x1) / 2) as i32 - 100).abs() < 6);
    }

    #[test]
    fn max_breite_verkleinert() {
        let mut s = schriften();
        let mut bild = Img::neu(300, 60);
        s.schreiben(&mut bild, 0.0, 0.0, "Hallo Welt", "x.ttf", 30, [255; 3], 'l', Art::default(), Some(100.0));
        let (_, _, x1, _) = bild.inhalt().unwrap();
        assert!(x1 <= 102, "{x1}");
    }
}
