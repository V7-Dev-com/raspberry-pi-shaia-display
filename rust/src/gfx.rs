//! Kleine Zeichenbibliothek: RGBA-Bild mit dem Verhalten von PIL (alpha_composite, paste, Linien,
//! Polygone, Gauß-Weichzeichner), genau so viel, wie die Anzeige braucht.

use image::imageops::FilterType;

pub type Farbe = [u8; 4];

#[derive(Clone)]
pub struct Img {
    pub w: usize,
    pub h: usize,
    /// RGBA, nicht vormultipliziert, Zeile für Zeile
    pub d: Vec<u8>,
}

impl Img {
    pub fn neu(w: usize, h: usize) -> Img {
        Img { w, h, d: vec![0; w * h * 4] }
    }

    pub fn gefuellt(w: usize, h: usize, f: Farbe) -> Img {
        let mut b = Img::neu(w, h);
        for p in b.d.chunks_exact_mut(4) {
            p.copy_from_slice(&f);
        }
        b
    }

    pub fn aus_datei(pfad: &std::path::Path) -> Result<Img, String> {
        let b = image::open(pfad).map_err(|e| format!("{}: {e}", pfad.display()))?.to_rgba8();
        Ok(Img { w: b.width() as usize, h: b.height() as usize, d: b.into_raw() })
    }

    #[inline]
    pub fn px(&self, x: usize, y: usize) -> Farbe {
        let i = (y * self.w + x) * 4;
        [self.d[i], self.d[i + 1], self.d[i + 2], self.d[i + 3]]
    }

    #[inline]
    pub fn setze(&mut self, x: i32, y: i32, f: Farbe) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            let i = (y as usize * self.w + x as usize) * 4;
            self.d[i..i + 4].copy_from_slice(&f);
        }
    }

    /// Ausschnitt; was außerhalb liegt, bleibt durchsichtig schwarz.
    pub fn ausschnitt(&self, x: i32, y: i32, w: usize, h: usize) -> Img {
        let mut o = Img::neu(w, h);
        for oy in 0..h {
            let sy = y + oy as i32;
            if sy < 0 || sy as usize >= self.h {
                continue;
            }
            let von = (x).max(0);
            let bis = (x + w as i32).min(self.w as i32);
            if bis <= von {
                continue;
            }
            let n = (bis - von) as usize * 4;
            let s = (sy as usize * self.w + von as usize) * 4;
            let t = (oy * w + (von - x) as usize) * 4;
            o.d[t..t + n].copy_from_slice(&self.d[s..s + n]);
        }
        o
    }

    pub fn gespiegelt(&self) -> Img {
        let mut o = Img::neu(self.w, self.h);
        for y in 0..self.h {
            for x in 0..self.w {
                let s = (y * self.w + x) * 4;
                let t = (y * self.w + (self.w - 1 - x)) * 4;
                o.d[t..t + 4].copy_from_slice(&self.d[s..s + 4]);
            }
        }
        o
    }

    /// Kleinstes Rechteck mit sichtbaren Pixeln (x0, y0, x1, y1), x1/y1 ausschließlich – wie PIL getbbox.
    pub fn inhalt(&self) -> Option<(usize, usize, usize, usize)> {
        let (mut x0, mut y0, mut x1, mut y1) = (self.w, self.h, 0, 0);
        for y in 0..self.h {
            for x in 0..self.w {
                if self.d[(y * self.w + x) * 4 + 3] != 0 {
                    x0 = x0.min(x);
                    y0 = y0.min(y);
                    x1 = x1.max(x + 1);
                    y1 = y1.max(y + 1);
                }
            }
        }
        (x1 > x0).then_some((x0, y0, x1, y1))
    }

    /// Wie PIL resize bei RGBA: vormultipliziert skalieren, dann zurückrechnen.
    pub fn skaliert(&self, w: usize, h: usize, filter: FilterType) -> Img {
        let mut b = image::RgbaImage::new(self.w as u32, self.h as u32);
        for (i, p) in b.pixels_mut().enumerate() {
            let a = self.d[i * 4 + 3] as u32;
            let m = |c: u8| ((c as u32 * a + 127) / 255) as u8;
            *p = image::Rgba([m(self.d[i * 4]), m(self.d[i * 4 + 1]), m(self.d[i * 4 + 2]), a as u8]);
        }
        let r = image::imageops::resize(&b, w as u32, h as u32, filter);
        let mut o = Img::neu(w, h);
        for (i, p) in r.pixels().enumerate() {
            let a = p[3] as u32;
            let u = |c: u8| if a == 0 { 0 } else { ((c as u32 * 255 + a / 2) / a).min(255) as u8 };
            o.d[i * 4..i * 4 + 4].copy_from_slice(&[u(p[0]), u(p[1]), u(p[2]), a as u8]);
        }
        o
    }

    /// Ersetzt Pixel samt Alpha (PIL paste ohne Maske), mit Beschneidung.
    pub fn einsetzen(&mut self, q: &Img, x: i32, y: i32) {
        for sy in 0..q.h {
            let ty = y + sy as i32;
            if ty < 0 || ty as usize >= self.h {
                continue;
            }
            let von = x.max(0);
            let bis = (x + q.w as i32).min(self.w as i32);
            if bis <= von {
                continue;
            }
            let n = (bis - von) as usize * 4;
            let s = (sy * q.w + (von - x) as usize) * 4;
            let t = (ty as usize * self.w + von as usize) * 4;
            self.d[t..t + n].copy_from_slice(&q.d[s..s + n]);
        }
    }

    /// Alpha-Compositing (Quelle über Ziel), mit Beschneidung – PIL alpha_composite / `einfuegen`.
    pub fn darueber(&mut self, q: &Img, x: i32, y: i32) {
        for sy in 0..q.h {
            let ty = y + sy as i32;
            if ty < 0 || ty as usize >= self.h {
                continue;
            }
            let von = x.max(0);
            let bis = (x + q.w as i32).min(self.w as i32);
            if bis <= von {
                continue;
            }
            for tx in von..bis {
                let s = (sy * q.w + (tx - x) as usize) * 4;
                let t = (ty as usize * self.w + tx as usize) * 4;
                mische(&mut self.d[t..t + 4], &q.d[s..s + 4]);
            }
        }
    }

    pub fn rechteck(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, f: Farbe) {
        for y in y0.max(0)..=y1.min(self.h as i32 - 1) {
            for x in x0.max(0)..=x1.min(self.w as i32 - 1) {
                let i = (y as usize * self.w + x as usize) * 4;
                self.d[i..i + 4].copy_from_slice(&f);
            }
        }
    }

    /// Waagerechte Linie wie PIL `line` mit `width` (Zeilen y .. y+width-1).
    pub fn linie_waagerecht(&mut self, y: i32, x0: i32, x1: i32, f: Farbe, breite: i32) {
        self.rechteck(x0, y, x1, y + breite - 1, f);
    }

    /// Gefülltes Polygon (gerade/ungerade Regel an Pixelmitten), danach optional Umriss.
    pub fn polygon(&mut self, pts: &[(f32, f32)], fuell: Farbe, umriss: Option<Farbe>) {
        let miny = pts.iter().map(|p| p.1).fold(f32::MAX, f32::min).floor() as i32;
        let maxy = pts.iter().map(|p| p.1).fold(f32::MIN, f32::max).ceil() as i32;
        for y in miny..=maxy {
            let yc = y as f32 + 0.5;
            let mut xs: Vec<f32> = Vec::new();
            for i in 0..pts.len() {
                let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                if (a.1 <= yc && b.1 > yc) || (b.1 <= yc && a.1 > yc) {
                    xs.push(a.0 + (yc - a.1) / (b.1 - a.1) * (b.0 - a.0));
                }
            }
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap());
            for paar in xs.chunks_exact(2) {
                for x in (paar[0] - 0.5).round() as i32..=(paar[1] - 0.5).round() as i32 {
                    self.setze(x, y, fuell);
                }
            }
        }
        if let Some(u) = umriss {
            for i in 0..pts.len() {
                let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
                self.linie(a.0.round() as i32, a.1.round() as i32, b.0.round() as i32, b.1.round() as i32, u);
            }
        }
    }

    /// Bresenham-Linie, 1 px.
    pub fn linie(&mut self, mut x0: i32, mut y0: i32, x1: i32, y1: i32, f: Farbe) {
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let mut fehler = dx + dy;
        loop {
            self.setze(x0, y0, f);
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * fehler;
            if e2 >= dy {
                fehler += dy;
                x0 += sx;
            }
            if e2 <= dx {
                fehler += dx;
                y0 += sy;
            }
        }
    }

    /// Ring wie PIL `ellipse(bbox, outline, width)`; bbox einschließlich der Randpixel.
    pub fn ellipse_ring(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, f: Farbe, breite: i32) {
        let (cx, cy) = ((x0 + x1) as f32 / 2.0 + 0.5, (y0 + y1) as f32 / 2.0 + 0.5);
        let (rx, ry) = ((x1 - x0 + 1) as f32 / 2.0, (y1 - y0 + 1) as f32 / 2.0);
        let (ix, iy) = ((rx - breite as f32).max(0.0), (ry - breite as f32).max(0.0));
        for y in y0..=y1 {
            for x in x0..=x1 {
                let (px, py) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                let aussen = (px / rx).powi(2) + (py / ry).powi(2) <= 1.0;
                let innen = ix > 0.0 && iy > 0.0 && (px / ix).powi(2) + (py / iy).powi(2) < 1.0;
                if aussen && !innen {
                    self.setze(x, y, f);
                }
            }
        }
    }

    /// Gauß-Weichzeichner wie Pillow: drei Kasten-Durchläufe mit Bruchteil-Radius, Randpixel wiederholt.
    /// Gerechnet wird nur im Bereich um die sichtbaren Pixel (Rand = Reichweite der drei Durchläufe),
    /// das Ergebnis ist dasselbe wie auf dem ganzen Bild.
    pub fn weichzeichnen(&self, radius: f32) -> Img {
        let passes = 3.0f32;
        let sigma2 = radius * radius / passes;
        let l_gross = (12.0 * sigma2 + 1.0).sqrt();
        let l = ((l_gross - 1.0) / 2.0).floor();
        let a = (2.0 * l + 1.0) * (l * (l + 1.0) - 3.0 * sigma2) / (6.0 * (sigma2 - (l + 1.0) * (l + 1.0)));
        let r = l + a;
        let Some((x0, y0, x1, y1)) = self.inhalt() else { return self.clone() };
        let rand = 3 * (r.floor() as i32 + 2);
        let (bx0, by0) = ((x0 as i32 - rand).max(0), (y0 as i32 - rand).max(0));
        let (bx1, by1) = ((x1 as i32 + rand).min(self.w as i32), (y1 as i32 + rand).min(self.h as i32));
        let mut teil = self.ausschnitt(bx0, by0, (bx1 - bx0) as usize, (by1 - by0) as usize);
        for _ in 0..3 {
            kasten(&mut teil, r, true);
            kasten(&mut teil, r, false);
        }
        let mut aus = self.clone();
        for d in aus.d.iter_mut() {
            *d = 0;
        }
        aus.einsetzen(&teil, bx0, by0);
        aus
    }

    pub fn nach_rgb565(&self, ziel: &mut [u16]) {
        for (o, p) in ziel.iter_mut().zip(self.d.chunks_exact(4)) {
            *o = ((p[0] as u16 >> 3) << 11) | ((p[1] as u16 >> 2) << 5) | (p[2] as u16 >> 3);
        }
    }

    pub fn speichern(&self, pfad: &std::path::Path) -> Result<(), String> {
        let mut rgb = Vec::with_capacity(self.w * self.h * 3);
        for p in self.d.chunks_exact(4) {
            rgb.extend_from_slice(&p[..3]);
        }
        image::save_buffer(pfad, &rgb, self.w as u32, self.h as u32, image::ColorType::Rgb8)
            .map_err(|e| format!("{}: {e}", pfad.display()))
    }
}

/// Quelle über Ziel (nicht vormultipliziert), wie PIL.
#[inline]
pub fn mische(ziel: &mut [u8], q: &[u8]) {
    let sa = q[3] as u32;
    if sa == 0 {
        return;
    }
    if sa == 255 {
        ziel.copy_from_slice(q);
        return;
    }
    let da = ziel[3] as u32;
    if da == 0 {
        ziel.copy_from_slice(q);
        return;
    }
    let ra = sa * 255 + da * (255 - sa); // Alpha mal 255
    for i in 0..3 {
        let c = (q[i] as u32 * sa * 255 + ziel[i] as u32 * da * (255 - sa)) / ra;
        ziel[i] = c.min(255) as u8;
    }
    ziel[3] = ((ra + 127) / 255).min(255) as u8;
}

/// Ein Kasten-Durchlauf in einer Richtung mit Radius r (Bruchteil für die Randpixel); gleitende Summe.
fn kasten(q: &mut Img, r: f32, waagerecht: bool) {
    let ri = r.floor() as i32;
    let fr = r - ri as f32;
    let fenster = (2 * ri + 1) as f32 + 2.0 * fr;
    let (laenge, zeilen) = if waagerecht { (q.w, q.h) } else { (q.h, q.w) };
    let (schritt, zeilenschritt) = if waagerecht { (4, q.w * 4) } else { (q.w * 4, 4) };
    let mut linie = vec![0u8; laenge + 2 * (ri as usize + 1)];
    let pad = ri as usize + 1;
    for z in 0..zeilen {
        let basis = z * zeilenschritt;
        for k in 0..4 {
            for i in 0..laenge {
                linie[pad + i] = q.d[basis + i * schritt + k];
            }
            let (erst, letzt) = (linie[pad], linie[pad + laenge - 1]);
            for i in 0..pad {
                linie[i] = erst;
                linie[pad + laenge + i] = letzt;
            }
            // Summe des Fensters für i = 0: Indizes pad-ri ..= pad+ri
            let mut summe: i32 = linie[pad - ri as usize..=pad + ri as usize].iter().map(|v| *v as i32).sum();
            for i in 0..laenge {
                let rand = linie[pad + i - ri as usize - 1] as f32 + linie[pad + i + ri as usize + 1] as f32;
                let wert = (summe as f32 + fr * rand) / fenster + 0.5;
                q.d[basis + i * schritt + k] = wert.clamp(0.0, 255.0) as u8;
                summe += linie[pad + i + ri as usize + 1] as i32 - linie[pad + i - ri as usize] as i32;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn darueber_deckend_und_durchsichtig() {
        let mut b = Img::gefuellt(2, 1, [10, 20, 30, 255]);
        let q = Img::gefuellt(1, 1, [200, 100, 50, 255]);
        b.darueber(&q, 1, 0);
        assert_eq!(b.px(1, 0), [200, 100, 50, 255]);
        let h = Img::gefuellt(1, 1, [255, 255, 255, 0]);
        b.darueber(&h, 0, 0);
        assert_eq!(b.px(0, 0), [10, 20, 30, 255]);
    }

    #[test]
    fn darueber_halbdurchsichtig_mischt() {
        let mut b = Img::gefuellt(1, 1, [0, 0, 0, 255]);
        b.darueber(&Img::gefuellt(1, 1, [255, 255, 255, 128]), 0, 0);
        let p = b.px(0, 0);
        assert!((p[0] as i32 - 128).abs() <= 1 && p[3] == 255);
    }

    #[test]
    fn beschneidung_ausserhalb_ist_harmlos() {
        let mut b = Img::neu(4, 4);
        let q = Img::gefuellt(3, 3, [1, 2, 3, 255]);
        b.darueber(&q, -2, -2);
        b.darueber(&q, 3, 3);
        b.darueber(&q, 100, 100);
        assert_eq!(b.px(0, 0), [1, 2, 3, 255]);
        assert_eq!(b.px(3, 3), [1, 2, 3, 255]);
    }

    #[test]
    fn inhalt_findet_den_sichtbaren_teil() {
        let mut b = Img::neu(8, 8);
        b.setze(2, 3, [1, 1, 1, 255]);
        b.setze(5, 6, [1, 1, 1, 255]);
        assert_eq!(b.inhalt(), Some((2, 3, 6, 7)));
    }

    #[test]
    fn waagerechte_linie_zwei_zeilen() {
        let mut b = Img::neu(5, 6);
        b.linie_waagerecht(2, 0, 4, [9, 9, 9, 255], 2);
        assert_eq!(b.px(0, 1)[3], 0);
        assert_eq!(b.px(0, 2)[3], 255);
        assert_eq!(b.px(0, 3)[3], 255);
        assert_eq!(b.px(0, 4)[3], 0);
    }

    #[test]
    fn weichzeichnen_erhaelt_helligkeit_in_der_mitte() {
        let b = Img::gefuellt(30, 30, [200, 100, 50, 255]);
        let w = b.weichzeichnen(7.0);
        assert_eq!(w.px(15, 15), [200, 100, 50, 255]);
    }

    #[test]
    fn rgb565_umrechnung() {
        let b = Img::gefuellt(1, 1, [255, 255, 255, 255]);
        let mut z = [0u16; 1];
        b.nach_rgb565(&mut z);
        assert_eq!(z[0], 0xFFFF);
    }
}
