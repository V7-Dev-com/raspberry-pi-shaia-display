//! Die Szene: Shaia im Dungeon. Zustandsautomat aus Schritten (Animation, Bewegung, Treffer,
//! Ende-Bedingung) für Shaia und die Skelette; Kampf, Alltag, Kugeln, Kiste, Lagerfeuer, Kamera.
//! Entspricht `Szene`, `Akteur` und `Skelett` aus `ruinrunner.py`.
//!
//! Wo v1 Closures benutzt (`beim_start`, `fertig_wenn`), stehen hier Aktions- und Bedingungs-Enums.

use crate::assets::*;
use crate::gfx::Img;
use std::collections::VecDeque;
use std::rc::Rc;

pub const ANKER_X: i32 = 118;
pub const ANKER_Y: i32 = 241;
pub const SCHWERKRAFT: f64 = 1100.0;
pub const RAND: f64 = 150.0;
pub const KAMERA_TEMPO: f64 = 240.0;
pub const KAMERA_BESCHL: f64 = 360.0;
pub const NACHT: (u32, u32) = (22, 6);
pub const TRUHE_X: f64 = HALLE as f64 - 150.0;
pub const LAGER_X: f64 = 330.0;
const WF: f64 = W as f64;
const HF: f64 = HALLE as f64;
const FUSSF: f64 = FUSS as f64;

/// Animationen, bei denen ein auftauchender Gegner Shaias Alltag unterbrechen darf (None = frei).
const ALLTAG: [&str; 14] = ["stehen_a", "stehen_b", "gehen", "ducken_rein", "ducken", "ducken_raus", "suchen",
    "nachdenken", "verzaubern", "drehen", "lagerfeuer", "lachen", "provozieren", "truhe_rein"];

fn im_alltag(anim: Option<&str>) -> bool {
    match anim {
        None => true,
        Some(a) => ALLTAG.contains(&a) || a == "truhe_raus",
    }
}

// --- Treffer, Aktionen, Bedingungen, Schritte -------------------------------------------------
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Art {
    Normal,
    Wurf,
    Hoch,
    Ultimate,
    Charme,
    Ice,
    Thunder,
    Poison,
    Fire,
}

#[derive(Clone, Copy, Debug)]
pub struct Treffer {
    pub schaden: i32,
    pub reichweite: f64,
    pub art: Art,
}

fn tr(schaden: i32, reichweite: f64, art: Art) -> Treffer {
    Treffer { schaden, reichweite, art }
}

fn element_art(e: &str) -> Art {
    match e {
        "fire" => Art::Fire,
        "ice" => Art::Ice,
        "thunder" => Art::Thunder,
        _ => Art::Poison,
    }
}

/// Was beim Beginn eines Schritts geschieht (in v1: `beim_start`).
#[derive(Clone, Debug)]
pub enum Aktion {
    Richtung(i32),
    Umkehren,
    /// Blick zum Ziel, wie `gehen_nach`
    Ausrichten(f64),
    Hoch(f64),
    Springen,
    Abheben { vy: f64, vx: f64 },
    SkelettSprung { weite: f64 },
    Liegt,
    Steht,
    Zerfallen,
    TruheBesuch(bool),
    TruheOeffnen,
    TruheSchatz,
    TruheZu,
    Aufheben(u32),
    Abschiessen { element: &'static str, ziel: u32 },
}

/// Wann ein Schritt endet (in v1: `fertig_wenn`).
#[derive(Clone, Debug)]
pub enum Fertig {
    Ziel(f64),
    /// auf dem Boden und ohne Schwung
    Boden,
    Gelandet,
    Gipfel,
    Nahe { gegner: u32, abstand: f64 },
    /// kein Gegner mehr oder keiner kommt mehr herein
    GegnerKommtNicht,
}

#[derive(Clone, Debug)]
pub struct Schritt {
    pub anim: &'static str,
    pub dauer: Option<f64>,
    pub vx: f64,
    pub treffer: Vec<(u32, Treffer)>,
    pub schleife: bool,
    pub start: Vec<Aktion>,
    pub fertig: Option<Fertig>,
    explizit: bool,
}

impl Schritt {
    pub fn neu(anim: &'static str) -> Schritt {
        Schritt { anim, dauer: None, vx: 0.0, treffer: vec![], schleife: false, start: vec![], fertig: None, explizit: false }
    }
    pub fn dauer(mut self, d: f64) -> Self {
        self.dauer = Some(d);
        if !self.explizit {
            self.schleife = true;
        }
        self
    }
    pub fn vx(mut self, v: f64) -> Self {
        self.vx = v;
        self
    }
    pub fn treffer(mut self, nr: u32, t: Treffer) -> Self {
        self.treffer.push((nr, t));
        self
    }
    pub fn start(mut self, a: Aktion) -> Self {
        self.start.push(a);
        self
    }
    pub fn fertig(mut self, f: Fertig) -> Self {
        self.fertig = Some(f);
        if !self.explizit {
            self.schleife = true;
        }
        self
    }
    pub fn schleife(mut self, s: bool) -> Self {
        self.schleife = s;
        self.explizit = true;
        self
    }
}

/// Zustand der Gegner für die Ende-Bedingungen.
pub struct Kontext {
    /// (id, x, kampffähig)
    pub skelette: Vec<(u32, f64, bool)>,
    /// erster lebender Gegner: kommt er noch herein?
    pub gegner_kommt: Option<bool>,
}

pub struct Akteur {
    pub x: f64,
    pub hoehe: f64,
    pub vy: f64,
    pub vx_luft: f64,
    pub richtung: i32,
    plan: VecDeque<Schritt>,
    pub schritt: Option<Schritt>,
    pub seit: f64,
    erledigt: Vec<u32>,
    pub ausstehend: Vec<Aktion>,
    heben: i32,
    standard: &'static str,
}

impl Akteur {
    pub fn neu(x: f64, richtung: i32, heben: i32, standard: &'static str) -> Akteur {
        Akteur {
            x, hoehe: 0.0, vy: 0.0, vx_luft: 0.0, richtung, plan: VecDeque::new(), schritt: None, seit: 0.0,
            erledigt: vec![], ausstehend: vec![], heben, standard,
        }
    }

    /// Ersetzt die laufende Handlung.
    pub fn planen(&mut self, schritte: Vec<Schritt>, jetzt: f64) {
        self.plan = schritte.into();
        self.weiter(jetzt);
    }

    fn weiter(&mut self, jetzt: f64) {
        self.schritt = self.plan.pop_front();
        self.seit = jetzt;
        self.erledigt.clear();
        if let Some(s) = &self.schritt {
            self.ausstehend.extend(s.start.iter().cloned());
        }
    }

    pub fn frei(&self) -> bool {
        self.schritt.is_none()
    }

    pub fn anim(&self) -> Option<&'static str> {
        self.schritt.as_ref().map(|s| s.anim)
    }

    /// (Bild im Durchlauf, rohe Bildnummer, Bildzahl)
    fn bildnummer(&self, jetzt: f64, anims: &AnimSatz) -> (usize, usize, usize) {
        let s = self.schritt.as_ref().unwrap();
        let a = &anims[s.anim];
        let n = a.reihe.rechts.len();
        let roh = ((jetzt - self.seit) * a.fps as f64).max(0.0) as usize;
        (if s.schleife { roh % n } else { roh.min(n - 1) }, roh, n)
    }

    fn physik(&mut self, dt: f64) {
        if self.hoehe > 0.0 || self.vy > 0.0 {
            self.vy -= SCHWERKRAFT * dt;
            self.hoehe = (self.hoehe + self.vy * dt).max(0.0);
            self.x += self.vx_luft * dt;
            if self.hoehe == 0.0 {
                self.vy = 0.0;
                self.vx_luft = 0.0;
            }
        }
    }

    fn ende(&self, f: &Fertig, ctx: &Kontext) -> bool {
        match f {
            Fertig::Ziel(z) => (self.x - z).abs() < 4.0 || (z - self.x) * (self.richtung as f64) < 0.0,
            Fertig::Boden => self.hoehe == 0.0 && self.vy == 0.0,
            Fertig::Gelandet => self.hoehe == 0.0,
            Fertig::Gipfel => self.vy <= 0.0,
            Fertig::Nahe { gegner, abstand } => match ctx.skelette.iter().find(|s| s.0 == *gegner) {
                Some(&(_, sx, kampf)) => (sx - self.x).abs() <= *abstand || !kampf,
                None => true,
            },
            Fertig::GegnerKommtNicht => ctx.gegner_kommt != Some(true),
        }
    }

    /// Bewegt, prüft das Schritt-Ende und liefert die ausgelösten Treffer.
    pub fn tick(&mut self, dt: f64, jetzt: f64, anims: &AnimSatz, ctx: &Kontext) -> Vec<Treffer> {
        self.physik(dt);
        let Some(s) = self.schritt.as_ref() else { return vec![] };
        self.x += s.vx * self.richtung as f64 * dt;
        let (_, roh, n) = self.bildnummer(jetzt, anims);
        let mut ausgeloest = vec![];
        for (nr, t) in &s.treffer {
            if !self.erledigt.contains(nr) && roh >= *nr as usize {
                self.erledigt.push(*nr);
                ausgeloest.push(*t);
            }
        }
        let fertig = if let Some(f) = &s.fertig {
            self.ende(f, ctx)
        } else if let Some(d) = s.dauer {
            jetzt - self.seit >= d
        } else {
            roh >= n
        };
        if fertig {
            self.weiter(jetzt);
        }
        ausgeloest
    }

    /// Bild samt Position (links oben) für die Halle.
    pub fn bild<'a>(&self, jetzt: f64, anims: &'a AnimSatz, heben_extra: i32) -> (&'a Img, i32, i32) {
        let (a, nr) = match &self.schritt {
            Some(s) => (&anims[s.anim], self.bildnummer(jetzt, anims).0),
            None => (&anims[self.standard], 0),
        };
        let b = if self.richtung > 0 { &a.reihe.rechts[nr] } else { &a.reihe.links[nr] };
        let ax = if self.richtung > 0 { ANKER_X } else { 256 - ANKER_X };
        let heben = self.heben + heben_extra;
        (&b.bild, (self.x - ax as f64 + b.dx as f64) as i32, (FUSSF - self.hoehe - ANKER_Y as f64 + b.dy as f64 - heben as f64) as i32)
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Zustand {
    Kommt,
    Steht,
    Getroffen,
    Luft,
    Liegt,
    Betaeubt,
    Tot,
}

pub struct Teil {
    x: f64,
    y: f64,
    vx: f64,
    vy: f64,
    reihe: Rc<Vec<Img>>,
}

pub struct Skelett {
    pub id: u32,
    pub a: Akteur,
    pub lp: i32,
    pub zustand: Zustand,
    pub angriff_ab: f64,
    pub springer: bool,
    pub sprung_ab: f64,
    pub teile: Vec<Teil>,
    pub tot_seit: Option<f64>,
}

impl Skelett {
    pub fn kampffaehig(&self) -> bool {
        matches!(self.zustand, Zustand::Kommt | Zustand::Steht | Zustand::Getroffen | Zustand::Betaeubt)
    }
}

pub struct Effekt {
    reihe: Rc<Vec<Img>>,
    x: f64,
    y: f64,
    start: f64,
    fps: f64,
}

pub struct Beute {
    id: u32,
    x: f64,
    bilder: Rc<Vec<Img>>,
    seit: f64,
}

pub struct Geschoss {
    x: f64,
    y: f64,
    ziel: u32,
    element: &'static str,
    bilder: Rc<Vec<Img>>,
}

#[derive(Clone, Copy, PartialEq)]
pub enum KugelArt {
    Kopf,
    Mitte,
    Boden,
}

impl KugelArt {
    /// Flughöhe über der Standlinie (Kopf bleibt unter der Leiste)
    fn hoehe(self) -> f64 {
        match self {
            KugelArt::Kopf => 122.0,
            KugelArt::Mitte => 88.0,
            KugelArt::Boden => 18.0,
        }
    }
}

pub struct Kugel {
    x: f64,
    art: KugelArt,
    richtung: i32,
    reagiert: bool,
    element: &'static str,
}

pub struct Szene {
    pub a: Rc<Assets>,
    pub rng: fastrand::Rng,
    pub jetzt: f64,
    pub shaia: Akteur,
    pub shaia_offline: bool,
    pub im_kampf: bool,
    pub skelette: Vec<Skelett>,
    pub wartend: u32,
    effekte: Vec<Effekt>,
    beute: Vec<Beute>,
    geschosse: Vec<Geschoss>,
    kugeln: Vec<Kugel>,
    pub kugel_ab: f64,
    pub nacht: bool,
    pub truhe_x: f64,
    pub truhe_seit: Option<f64>,
    pub truhe_zu_seit: Option<f64>,
    pub truhe_besuch: bool,
    pub truhe_ab: f64,
    pub truhe_fuer_stunde: u32,
    pub spaziergang_ab: f64,
    pub bonus_ab: f64,
    letzter_angriff: Option<&'static str>,
    sieg: bool,
    pub lager_ab: f64,
    verzaubert_im_kampf: bool,
    pub kamera: f64,
    pub kamera_v: f64,
    pub statisch: Option<Img>,
    /// je Zeile der Oberfläche: 0 leer, 1 ganz deckend, 2 gemischt
    statisch_zeilen: Vec<u8>,
    naechste_id: u32,
}

impl Szene {
    pub fn neu(a: Rc<Assets>, seed: Option<u64>, stunde: u32, jetzt: f64) -> Szene {
        let mut rng = seed.map_or_else(fastrand::Rng::new, fastrand::Rng::with_seed);
        let mut u = |lo: f64, hi: f64| lo + rng.f64() * (hi - lo);
        let (kugel_ab, truhe_ab, spaziergang_ab) = (jetzt + u(25.0, 50.0), jetzt + u(40.0, 90.0), jetzt + u(40.0, 90.0));
        Szene {
            a, rng, jetzt,
            shaia: Akteur::neu(HF / 2.0, 1, 0, "stehen_a"),
            shaia_offline: false, im_kampf: false, skelette: vec![], wartend: 0,
            effekte: vec![], beute: vec![], geschosse: vec![], kugeln: vec![], kugel_ab,
            nacht: false, truhe_x: TRUHE_X, truhe_seit: None, truhe_zu_seit: None, truhe_besuch: false, truhe_ab,
            truhe_fuer_stunde: stunde, spaziergang_ab, bonus_ab: jetzt + 45.0, letzter_angriff: None, sieg: false,
            lager_ab: 0.0, verzaubert_im_kampf: false, kamera: (HF - WF) / 2.0, kamera_v: 0.0, statisch: None,
            statisch_zeilen: vec![], naechste_id: 1,
        }
    }

    /// Neue Oberfläche (Kopf, Fuß, Text) setzen und je Zeile merken, was beim Zeichnen nötig ist.
    pub fn statisch_setzen(&mut self, img: Img) {
        self.statisch_zeilen = (0..img.h)
            .map(|y| {
                let alpha = (0..img.w).map(|x| img.d[(y * img.w + x) * 4 + 3]);
                let (mut leer, mut voll) = (true, true);
                for a in alpha {
                    leer &= a == 0;
                    voll &= a == 255;
                }
                if leer { 0 } else if voll { 1 } else { 2 }
            })
            .collect();
        self.statisch = Some(img);
    }

    fn uniform(&mut self, lo: f64, hi: f64) -> f64 {
        lo + self.rng.f64() * (hi - lo)
    }

    fn wahl<T: Copy>(&mut self, v: &[T]) -> T {
        v[self.rng.usize(..v.len())]
    }

    /// Wie `random.choices` mit Gewichten.
    fn gewichtet<T: Copy>(&mut self, v: &[(T, f64)]) -> T {
        let summe: f64 = v.iter().map(|x| x.1).sum();
        let mut r = self.rng.f64() * summe;
        for (t, g) in v {
            if r < *g {
                return *t;
            }
            r -= g;
        }
        v.last().unwrap().0
    }

    fn effekt(&mut self, name: &str, x: f64, y: f64, fps: f64) {
        let reihe = self.a.vfx[name].clone();
        self.effekte.push(Effekt { reihe, x, y, start: self.jetzt, fps });
    }

    pub fn gegner_idx(&self) -> Option<usize> {
        self.skelette.iter().position(|s| s.zustand != Zustand::Tot)
    }

    fn idx(&self, id: u32) -> Option<usize> {
        self.skelette.iter().position(|s| s.id == id)
    }

    /// Für Testbilder: ein Eis-Orb liegt 120 px vor Shaia.
    pub fn beute_hinzu_test(&mut self) {
        let (x, bilder) = (self.shaia.x + 120.0, self.a.orbs["ice"].clone());
        self.neue_beute(x, bilder);
    }

    pub fn skelette_dazu(&mut self, anzahl: u32) {
        if self.shaia_offline {
            return;
        }
        self.wartend = self.wartend.saturating_add(anzahl).min(6);
    }

    fn kontext(&self) -> Kontext {
        Kontext {
            skelette: self.skelette.iter().map(|s| (s.id, s.a.x, s.kampffaehig())).collect(),
            gegner_kommt: self.gegner_idx().map(|i| self.skelette[i].zustand == Zustand::Kommt),
        }
    }

    // ---------- Aktionen ausführen ----------
    fn shaia_aktionen(&mut self) {
        let akt: Vec<Aktion> = std::mem::take(&mut self.shaia.ausstehend);
        for a in akt {
            self.aktion_shaia(a);
        }
    }

    fn skelett_aktionen(&mut self, i: usize) {
        let akt: Vec<Aktion> = std::mem::take(&mut self.skelette[i].a.ausstehend);
        for a in akt {
            self.aktion_skelett(i, a);
        }
    }

    fn aktion_shaia(&mut self, a: Aktion) {
        match a {
            Aktion::Richtung(r) => self.shaia.richtung = r,
            Aktion::Umkehren => self.shaia.richtung = -self.shaia.richtung,
            Aktion::Ausrichten(z) => self.shaia.richtung = if z > self.shaia.x { 1 } else { -1 },
            Aktion::Hoch(v) => self.shaia.vy = v,
            Aktion::Springen => {
                self.shaia.vy = 310.0;
                self.shaia.vx_luft = 95.0 * self.shaia.richtung as f64;
            }
            Aktion::TruheBesuch(b) => self.truhe_besuch = b,
            Aktion::TruheOeffnen => {
                self.truhe_seit = Some(self.jetzt);
                self.truhe_zu_seit = None;
            }
            Aktion::TruheSchatz => {
                let (tx, seite) = (self.truhe_x, if self.truhe_x > HF / 2.0 { 1.0 } else { -1.0 });
                self.effekt("twinkle", tx - 32.0, FUSSF - 100.0, 10.0);
                self.effekt("coin", tx - 8.0, FUSSF - 70.0, 10.0);
                let art = self.wahl(&["orb", "orb", "cheese", "bread", "potion_red", "potion_blue", "potion_green"]);
                let bilder = if art == "orb" {
                    let e = self.wahl(&ELEMENTE);
                    self.a.orbs[e].clone()
                } else {
                    self.a.essen[art].clone()
                };
                self.neue_beute(tx - seite * 170.0, bilder);
            }
            Aktion::TruheZu => self.truhe_zu_seit = Some(self.jetzt),
            Aktion::Aufheben(id) => {
                if let Some(p) = self.beute.iter().position(|b| b.id == id) {
                    let x = self.beute.remove(p).x;
                    self.effekt("twinkle", x - 32.0, FUSSF - 70.0, 12.0);
                    self.effekt("coin", x - 8.0, FUSSF - 90.0, 10.0);
                }
            }
            Aktion::Abschiessen { element, ziel } => {
                let (x, bilder) = (self.shaia.x + 50.0 * self.shaia.richtung as f64, self.a.orbs[element].clone());
                self.geschosse.push(Geschoss { x, y: FUSSF - 125.0, ziel, element, bilder });
            }
            // gelten nur für Skelette
            Aktion::Abheben { .. } | Aktion::SkelettSprung { .. } | Aktion::Liegt | Aktion::Steht | Aktion::Zerfallen => {}
        }
    }

    fn aktion_skelett(&mut self, i: usize, a: Aktion) {
        match a {
            Aktion::Richtung(r) => self.skelette[i].a.richtung = r,
            Aktion::Abheben { vy, vx } => {
                let s = &mut self.skelette[i].a;
                s.vy = vy;
                s.vx_luft = vx;
                s.hoehe = 1.0;
            }
            Aktion::SkelettSprung { weite } => {
                let flug = 2.0 * 300.0 / SCHWERKRAFT;
                let s = &mut self.skelette[i];
                s.zustand = Zustand::Luft;
                s.a.vy = 300.0;
                s.a.vx_luft = s.a.richtung as f64 * weite / flug;
            }
            Aktion::Liegt => {
                self.skelette[i].zustand = Zustand::Liegt;
                let x = self.skelette[i].a.x;
                self.effekt("smoke", x - 16.0, FUSSF - 28.0, 16.0);
            }
            Aktion::Steht => self.skelette[i].zustand = Zustand::Steht,
            Aktion::Zerfallen => self.zerfallen(i),
            _ => {}
        }
    }

    fn neue_beute(&mut self, x: f64, bilder: Rc<Vec<Img>>) {
        let id = self.naechste_id;
        self.naechste_id += 1;
        self.beute.push(Beute { id, x, bilder, seit: self.jetzt });
    }

    // ---------- Treffer auf ein Skelett ----------
    fn treffe(&mut self, i: usize, t: Treffer) {
        let (sx, rl) = (self.skelette[i].a.x, self.shaia.richtung as f64);
        let dx = sx - self.shaia.x;
        if !self.skelette[i].kampffaehig() || dx.abs() > t.reichweite || (dx * rl < -20.0 && t.reichweite < 1000.0) {
            return;
        }
        let weg = if dx >= 0.0 { 1.0 } else { -1.0 };
        let jetzt = self.jetzt;
        if t.art == Art::Charme {
            self.skelette[i].zustand = Zustand::Betaeubt;
            self.skelette[i].a.planen(vec![Schritt::neu("stehen").dauer(2.8)], jetzt);
            self.skelett_aktionen(i);
            return;
        }
        if t.art == Art::Ice || t.art == Art::Thunder {
            self.effekt("hit", sx - 64.0, FUSSF - 144.0, 18.0);
            self.effekt(if t.art == Art::Ice { "twinkle" } else { "stun" }, sx - 32.0, FUSSF - 150.0, 12.0);
            self.skelette[i].lp -= t.schaden;
            if self.skelette[i].lp <= 0 {
                self.umwerfen(i, weg * 190.0, 280.0);
            } else {
                self.skelette[i].zustand = Zustand::Betaeubt;
                self.skelette[i].a.planen(vec![Schritt::neu("treffer_koerper").dauer(1.4)], jetzt);
                self.skelett_aktionen(i);
            }
            return;
        }
        if t.art == Art::Poison {
            self.effekt("smoke", sx - 16.0, FUSSF - 140.0, 10.0);
            self.effekt("smoke", sx - 26.0, FUSSF - 110.0, 8.0);
        }
        if t.art == Art::Fire {
            self.effekt("fire_banish", sx - 32.0, FUSSF - 110.0, 12.0);
            let reihe = self.a.feuer_klein.clone();
            self.effekte.push(Effekt { reihe, x: sx - 16.0, y: FUSSF - 64.0, start: jetzt, fps: 16.0 });
        }
        if t.art == Art::Ultimate {
            self.effekt("ground_shock", sx - 64.0, FUSSF - 26.0, 12.0);
            self.effekt("hit", sx - 64.0, FUSSF - 144.0, 16.0);
        } else {
            self.effekt("hit", sx - 64.0 - weg * 10.0, FUSSF - 144.0, 18.0);
        }
        self.skelette[i].lp -= t.schaden;
        if self.skelette[i].lp <= 0 || matches!(t.art, Art::Wurf | Art::Hoch | Art::Ultimate) {
            let (vx, vy) = match t.art {
                Art::Hoch => (70.0, 520.0),
                Art::Ultimate => (300.0, 420.0),
                _ => (190.0, 280.0),
            };
            self.umwerfen(i, weg * vx, vy);
        } else {
            let anim = self.wahl(&["treffer_kopf", "treffer_koerper"]);
            let s = &mut self.skelette[i];
            s.zustand = Zustand::Getroffen;
            s.a.x += weg * 12.0;
            s.a.planen(vec![Schritt::neu(anim).dauer(0.3)], jetzt);
            self.skelett_aktionen(i);
        }
    }

    fn umwerfen(&mut self, i: usize, vx: f64, vy: f64) {
        let jetzt = self.jetzt;
        let lp = self.skelette[i].lp;
        let s = &mut self.skelette[i];
        s.a.richtung = if vx > 0.0 { -1 } else { 1 }; // fliegt rückwärts, Blick zu Shaia
        s.zustand = Zustand::Luft;
        let mut plan = vec![
            Schritt::neu("wurf_rein").dauer(0.08),
            Schritt::neu("flug").start(Aktion::Abheben { vy, vx }).fertig(Fertig::Boden),
            Schritt::neu("liegen").dauer(0.9).start(Aktion::Liegt),
        ];
        if lp > 0 {
            plan.push(Schritt::neu("aufstehen"));
            plan.push(Schritt::neu("stehen").dauer(0.3).start(Aktion::Steht));
        } else {
            plan.push(Schritt::neu("liegen").dauer(0.01).start(Aktion::Zerfallen));
        }
        s.a.planen(plan, jetzt);
        self.skelett_aktionen(i);
    }

    fn zerfallen(&mut self, i: usize) {
        let jetzt = self.jetzt;
        let sx = self.skelette[i].a.x;
        self.skelette[i].zustand = Zustand::Tot;
        self.skelette[i].tot_seit = Some(jetzt);
        for k in 0..self.a.truemmer.len() {
            let (vx, vy) = (self.uniform(-170.0, 170.0), self.uniform(-340.0, -150.0));
            let reihe = self.a.truemmer[k].clone();
            self.skelette[i].teile.push(Teil { x: sx, y: FUSSF - 50.0, vx, vy, reihe });
        }
        self.effekt("coin", sx - 8.0, FUSSF - 120.0, 10.0);
        self.effekt("twinkle", sx - 32.0, FUSSF - 150.0, 12.0);
        self.sieg = true;
        if self.rng.f64() < 0.6 && self.beute.len() < 3 {
            let orb = self.wahl(&[true, true, true, false, false]);
            let bilder = if orb {
                let e = self.wahl(&ELEMENTE);
                self.a.orbs[e].clone()
            } else {
                let n = self.wahl(&["cheese", "bread", "potion_red", "potion_blue", "potion_green"]);
                self.a.essen[n].clone()
            };
            self.neue_beute(sx.clamp(80.0, HF - 80.0), bilder);
        }
    }

    // ---------- Angriff des Skeletts ----------
    fn skelett_springt(&mut self, i: usize) {
        let jetzt = self.jetzt;
        let weite = ((self.shaia.x - self.skelette[i].a.x).abs() - 85.0).max(55.0);
        let ab = jetzt + self.uniform(5.0, 9.0);
        let s = &mut self.skelette[i];
        s.sprung_ab = ab;
        s.a.planen(
            vec![
                Schritt::neu("sprung_rein").dauer(0.25),
                Schritt::neu("sprung_hoch").start(Aktion::SkelettSprung { weite }).fertig(Fertig::Gipfel),
                Schritt::neu("sprung_runter").fertig(Fertig::Gelandet),
                Schritt::neu("landung").start(Aktion::Steht).treffer(0, tr(1, 150.0, Art::Normal)),
            ],
            jetzt,
        );
        self.skelett_aktionen(i);
    }

    fn skelett_schlaegt(&mut self, i: usize) {
        let jetzt = self.jetzt;
        let (sx, sh_x) = (self.skelette[i].a.x, self.shaia.x);
        if (sx - sh_x).abs() > 150.0 || self.shaia_offline || self.shaia.hoehe > 0.0 {
            return;
        }
        let anim = self.shaia.anim();
        if !matches!(anim, Some("stehen_a" | "stehen_b" | "abwehr" | "gehen" | "rennen" | "drehen") | None) {
            return; // Shaia greift gerade an oder liegt schon
        }
        let mitte = (sx + sh_x) / 2.0;
        self.shaia.richtung = if sx > sh_x { 1 } else { -1 };
        let wurf = self.rng.f64();
        if wurf < 0.6 {
            self.effekt("guard", mitte - 32.0, FUSSF - 110.0, 16.0);
            self.shaia.planen(vec![Schritt::neu("abwehr").dauer(0.45).vx(-25.0)], jetzt);
        } else if wurf < 0.9 {
            self.effekt("hit", sh_x - 64.0, FUSSF - 150.0, 18.0);
            let t = self.wahl(&["treffer_kopf", "treffer_koerper"]);
            self.shaia.planen(vec![Schritt::neu(t).dauer(0.35).vx(-40.0), Schritt::neu("stehen_b").dauer(0.3)], jetzt);
        } else {
            self.effekt("hit", sh_x - 64.0, FUSSF - 150.0, 18.0);
            self.shaia.planen(vec![Schritt::neu("umgeworfen").vx(-35.0), Schritt::neu("stehen_b").dauer(0.4)], jetzt);
        }
        self.shaia_aktionen();
    }

    // ---------- Shaias Angriffe ----------
    fn angriff_planen(&mut self, i: usize) {
        let jetzt = self.jetzt;
        let (sx, sid, lp, zustand) = (self.skelette[i].a.x, self.skelette[i].id, self.skelette[i].lp, self.skelette[i].zustand);
        let d = (sx - self.shaia.x).abs();
        let mut zuege: Vec<(&'static str, f64)> = vec![
            ("kombo", 3.0), ("knie", 1.3), ("duck", 0.0), ("sprung", 1.1), ("tritt", if d > 170.0 { 3.0 } else { 0.8 }),
            ("kreis", 1.2), ("schild", 1.0), ("aufwaerts", 1.0), ("zauber", if d > 160.0 { 1.6 } else { 0.5 }),
            ("charme", if zustand == Zustand::Betaeubt { 0.0 } else { 0.6 }), ("ultimate", if lp <= 2 { 2.5 } else { 0.0 }),
        ];
        if let Some(l) = self.letzter_angriff {
            if let Some(z) = zuege.iter_mut().find(|z| z.0 == l) {
                z.1 *= 0.15;
            }
        }
        let zug = self.gewichtet(&zuege);
        self.letzter_angriff = Some(zug);
        let reichweite = match zug {
            "zauber" | "charme" => 300.0,
            "ultimate" => 190.0,
            "tritt" => 130.0,
            "kreis" => 140.0,
            "schild" => 260.0,
            "sprung" => 120.0,
            _ => 110.0,
        };
        let mut plan: Vec<Schritt> = vec![];
        if !self.verzaubert_im_kampf && self.rng.f64() < 0.35 {
            self.verzaubert_im_kampf = true;
            plan.push(Schritt::neu("verzaubern"));
        }
        if d > reichweite {
            let ziel = reichweite - 10.0;
            let weit = d - ziel > 70.0;
            plan.push(
                Schritt::neu(if weit { "rennen" } else { "gehen" })
                    .vx(if weit { 170.0 } else { 60.0 })
                    .fertig(Fertig::Nahe { gegner: sid, abstand: ziel }),
            );
        }
        let n = |t: &'static str| Schritt::neu(t);
        match zug {
            "kombo" => {
                let anzahl = self.rng.i32(2..=4);
                for k in 1..=anzahl {
                    let name: &'static str = ["", "kombo1", "kombo2", "kombo3", "kombo4"][k as usize];
                    let (nr, t) = if k == 4 { (5, tr(1, 125.0, Art::Wurf)) } else { (2, tr(1, 125.0, Art::Normal)) };
                    plan.push(n(name).vx(25.0).treffer(nr, t));
                }
            }
            "knie" => plan.push(n("knie").vx(40.0).treffer(3, tr(1, 105.0, Art::Normal))),
            "duck" => plan.extend([
                n("ducken_rein"),
                n("duckangriff").treffer(4, tr(1, 135.0, Art::Normal)).treffer(10, tr(1, 135.0, Art::Normal)),
                n("ducken_raus"),
            ]),
            "sprung" => plan.extend([
                n("absprung").dauer(0.08),
                n("sprungangriff").start(Aktion::Springen).schleife(false).fertig(Fertig::Boden)
                    .treffer(3, tr(1, 130.0, Art::Normal)),
                n("landen"),
            ]),
            "tritt" => plan.extend([
                n("tritt_rein").vx(160.0),
                n("tritt").vx(200.0).treffer(0, tr(1, 140.0, Art::Wurf)),
                n("tritt_raus").vx(40.0),
            ]),
            "kreis" => plan.push(
                n("kreisschlag").treffer(4, tr(1, 150.0, Art::Normal)).treffer(9, tr(1, 150.0, Art::Normal))
                    .treffer(14, tr(1, 150.0, Art::Wurf)),
            ),
            "schild" => plan.extend([
                n("schild_rein"),
                n("schild").vx(280.0).dauer(0.45).treffer(1, tr(1, 90.0, Art::Wurf)),
                n("schild_raus"),
            ]),
            "aufwaerts" => plan.extend([
                n("aufwaerts_rein"),
                n("aufwaerts").treffer(1, tr(2, 125.0, Art::Hoch)),
                n("aufwaerts_raus"),
            ]),
            "zauber" => {
                let element = self.wahl(&KUGEL_ELEMENTE);
                plan.extend([
                    n("zauber_rein"),
                    n("zauber").dauer(0.7).start(Aktion::Abschiessen { element, ziel: sid }),
                    n("zauber_raus"),
                ]);
            }
            "charme" => plan.extend([
                n("charme_rein"),
                n("charme").dauer(1.0).treffer(1, tr(0, 320.0, Art::Charme)),
                n("charme_raus"),
            ]),
            "ultimate" => plan.extend([n("verzaubern"), n("ultimate").dauer(0.8).treffer(3, tr(99, 200.0, Art::Ultimate))]),
            _ => {}
        }
        let ende = self.wahl(&["stehen_b", "abwehr"]);
        let pause = self.uniform(0.3, 0.8);
        plan.push(Schritt::neu(ende).dauer(pause));
        self.shaia.planen(plan, jetzt);
        self.shaia_aktionen();
    }

    // ---------- Alltag ----------
    fn gehen_nach(&self, ziel: f64) -> Schritt {
        Schritt::neu("gehen").vx(60.0).start(Aktion::Ausrichten(ziel)).fertig(Fertig::Ziel(ziel))
    }

    fn truhe_weg(&mut self) {
        self.truhe_seit = None;
        self.truhe_zu_seit = None;
        self.truhe_besuch = false;
    }

    fn alltag_planen(&mut self, stunde: u32) {
        let jetzt = self.jetzt;
        let sh_x = self.shaia.x;
        if self.sieg {
            self.sieg = false;
            self.verzaubert_im_kampf = false;
            let a = self.wahl(&["lachen", "provozieren"]);
            let d = self.uniform(1.4, 2.4);
            self.shaia.planen(vec![Schritt::neu(a).dauer(d), Schritt::neu("stehen_a").dauer(1.0)], jetzt);
            self.shaia_aktionen();
            return;
        }
        if !self.beute.is_empty() {
            let b = self.beute.iter().min_by(|a, b| (a.x - sh_x).abs().partial_cmp(&(b.x - sh_x).abs()).unwrap()).unwrap();
            let (id, bx) = (b.id, b.x);
            let richtung = if bx > sh_x { 1 } else { -1 };
            let plan = vec![
                self.gehen_nach(bx - 45.0 * richtung as f64),
                Schritt::neu("drehen").start(Aktion::Richtung(richtung)),
                Schritt::neu("suchen").schleife(false), // bückt sich, sucht den Boden ab
                Schritt::neu("ducken").dauer(0.4).start(Aktion::Aufheben(id)),
                Schritt::neu("ducken_raus"),
                Schritt::neu("lachen").dauer(1.0),
            ];
            self.shaia.planen(plan, jetzt);
            self.shaia_aktionen();
            return;
        }
        // Truhe: zur vollen Stunde und ab und zu zwischendurch geht sie hin und holt etwas raus
        if (stunde != self.truhe_fuer_stunde || jetzt > self.truhe_ab) && self.kugeln.is_empty() {
            self.truhe_fuer_stunde = stunde;
            self.truhe_ab = jetzt + self.uniform(120.0, 240.0);
            let tx = self.truhe_x;
            let seite = if tx > HF / 2.0 { 1 } else { -1 }; // Truhe rechts: Shaia steht links davon
            let ziel = tx - seite as f64 * 88.0;
            let plan = vec![
                self.gehen_nach(ziel),
                Schritt::neu("drehen").start(Aktion::TruheBesuch(true)).start(Aktion::Richtung(seite)),
                Schritt::neu("truhe_rein").start(Aktion::TruheOeffnen),
                Schritt::neu("truhe_raus").start(Aktion::TruheSchatz),
                Schritt::neu("lachen").dauer(1.2),
                Schritt::neu("stehen_a").dauer(0.4).start(Aktion::TruheZu),
                Schritt::neu("stehen_a").dauer(0.6).start(Aktion::TruheBesuch(false)),
            ];
            self.shaia.planen(plan, jetzt);
            self.shaia_aktionen();
            return;
        }
        if stunde >= NACHT.0 || stunde < NACHT.1 {
            if jetzt < self.lager_ab {
                // erst durchatmen, nicht gleich ans Feuer
                self.shaia.planen(vec![Schritt::neu("stehen_a").dauer(1.5)], jetzt);
                return;
            }
            if !self.kugeln.is_empty() {
                // erst lassen, was noch fliegt
                self.shaia.planen(vec![Schritt::neu("stehen_a").dauer(0.5)], jetzt);
                return;
            }
            if jetzt > self.spaziergang_ab {
                // nachts: eine Runde durch die Halle, dann zurück ans Feuer
                self.spaziergang_ab = jetzt + self.uniform(20.0, 50.0);
                let (z1, z2) = (self.uniform(120.0, HF - 120.0), self.uniform(120.0, HF - 120.0));
                let plan = vec![self.gehen_nach(z1), self.gehen_nach(z2), Schritt::neu("stehen_a").dauer(1.0)];
                self.shaia.planen(plan, jetzt);
                self.shaia_aktionen();
                return;
            }
            let mut plan = if (sh_x - LAGER_X).abs() > 8.0 { vec![self.gehen_nach(LAGER_X)] } else { vec![] };
            plan.extend([
                Schritt::neu("stehen_a").dauer(0.4),
                Schritt::neu("drehen").start(Aktion::Richtung(1)),
                Schritt::neu("lagerfeuer").dauer(60.0),
            ]);
            self.shaia.planen(plan, jetzt);
            self.shaia_aktionen();
            return;
        }
        // ab und zu ein Spaziergang durch die Halle - die Kamera läuft mit
        if jetzt > self.spaziergang_ab {
            self.spaziergang_ab = jetzt + self.uniform(40.0, 100.0);
            let mut ziel = self.uniform(120.0, HF - 120.0);
            while (ziel - sh_x).abs() < 400.0 {
                ziel = self.uniform(120.0, HF - 120.0);
            }
            let plan = vec![self.gehen_nach(ziel), Schritt::neu("stehen_a").dauer(1.5)];
            self.shaia.planen(plan, jetzt);
            self.shaia_aktionen();
            return;
        }
        let auswahl = self.gewichtet(&[
            ("stehen_a", 4.0), ("stehen_b", 2.0), ("gehen", 3.0), ("nachdenken", 1.0), ("verzaubern", 0.5), ("drehen", 1.0),
        ]);
        match auswahl {
            "gehen" => {
                let ziel = (sh_x + self.uniform(-220.0, 220.0)).clamp(120.0, HF - 120.0);
                let plan = vec![self.gehen_nach(ziel), Schritt::neu("stehen_a").dauer(1.0)];
                self.shaia.planen(plan, jetzt);
            }
            "drehen" => self.shaia.planen(vec![Schritt::neu("drehen").start(Aktion::Umkehren), Schritt::neu("stehen_a").dauer(2.0)], jetzt),
            "verzaubern" => self.shaia.planen(vec![Schritt::neu("verzaubern"), Schritt::neu("stehen_b").dauer(1.5)], jetzt),
            a => {
                let d = self.uniform(2.5, 7.0);
                self.shaia.planen(vec![Schritt::neu(a).dauer(d)], jetzt);
            }
        }
        self.shaia_aktionen();
    }

    // ---------- Zauberkugeln aus der Ferne ----------
    fn kugeln_bewegen(&mut self, dt: f64) {
        let jetzt = self.jetzt;
        let frei = !self.shaia_offline
            && self.gegner_idx().is_none()
            && self.kugeln.is_empty()
            && im_alltag(self.shaia.anim())
            && self.shaia.anim() != Some("lagerfeuer")
            && self.shaia.hoehe == 0.0;
        if frei && jetzt > self.kugel_ab {
            self.kugel_ab = jetzt + self.uniform(35.0, 80.0);
            // meist von vorn (sie sieht sie kommen), manchmal von hinten (dann trifft sie)
            let von_vorn = self.rng.f64() < 0.75;
            let von_links = if von_vorn { self.shaia.richtung < 0 } else { self.shaia.richtung > 0 };
            let art = self.wahl(&[KugelArt::Kopf, KugelArt::Mitte, KugelArt::Boden]);
            let element = self.wahl(&KUGEL_ELEMENTE);
            self.kugeln.push(Kugel {
                x: if von_links { self.kamera - 40.0 } else { self.kamera + WF + 40.0 },
                art, richtung: if von_links { 1 } else { -1 }, reagiert: false, element,
            });
        }
        let mut k = 0;
        while k < self.kugeln.len() {
            self.kugeln[k].x += self.kugeln[k].richtung as f64 * 260.0 * dt;
            let (kx, kr, art, reagiert) = (self.kugeln[k].x, self.kugeln[k].richtung, self.kugeln[k].art, self.kugeln[k].reagiert);
            let sh_x = self.shaia.x;
            let abstand = (sh_x - kx) * kr as f64; // > 0: Kugel kommt noch auf Shaia zu
            let sieht_sie = kr == -self.shaia.richtung; // Kugel fliegt auf ihr Gesicht zu
            if !reagiert && abstand > 0.0 && abstand < 170.0 && !self.shaia_offline && sieht_sie {
                self.kugeln[k].reagiert = true;
                self.ausweichen(art);
            } else if !reagiert && abstand > 0.0 && abstand < 30.0 && !self.shaia_offline {
                // von hinten: sie hat sie nicht gesehen und wird getroffen
                let ruecken = sh_x - self.shaia.richtung as f64 * 22.0; // Kontaktpunkt am Rücken
                self.effekt("hit", ruecken - 64.0, FUSSF - 64.0 - art.hoehe(), 18.0);
                self.shaia.hoehe = 0.0;
                self.shaia.vy = 0.0;
                self.shaia.vx_luft = 0.0;
                let t = self.wahl(&["treffer_kopf", "treffer_koerper"]);
                self.shaia.planen(
                    vec![
                        Schritt::neu(t).dauer(0.4).vx(50.0), // die Kugel schiebt sie nach vorn
                        Schritt::neu("drehen").start(Aktion::Umkehren),
                        Schritt::neu("stehen_b").dauer(1.0),
                    ],
                    jetzt,
                );
                self.shaia_aktionen();
                self.kugeln.remove(k);
                continue;
            }
            let reagiert = self.kugeln[k].reagiert;
            if art == KugelArt::Mitte && reagiert && abstand > 0.0 && abstand < 45.0 {
                // am Schild zerplatzt
                self.effekt("guard", sh_x - 32.0 - kr as f64 * 30.0, FUSSF - 132.0, 16.0);
                self.effekt("hit", sh_x - 64.0 - kr as f64 * 30.0, FUSSF - 164.0, 18.0);
                self.kugeln.remove(k);
                continue;
            } else if !(self.kamera - 80.0 < kx && kx < self.kamera + WF + 80.0) {
                self.kugeln.remove(k);
                continue;
            }
            k += 1;
        }
    }

    /// Kopfhöhe: ducken, Boden: springen, Mitte: mit dem Schild blocken.
    fn ausweichen(&mut self, art: KugelArt) {
        let jetzt = self.jetzt;
        let plan = match art {
            KugelArt::Kopf => vec![
                Schritt::neu("ducken_rein"), Schritt::neu("ducken").dauer(0.55), Schritt::neu("ducken_raus"),
                Schritt::neu("stehen_b").dauer(0.6),
            ],
            KugelArt::Boden => vec![
                Schritt::neu("absprung").dauer(0.06),
                Schritt::neu("steigen").start(Aktion::Hoch(310.0)).fertig(Fertig::Gipfel),
                Schritt::neu("fallen").fertig(Fertig::Gelandet),
                Schritt::neu("landen"),
                Schritt::neu("stehen_b").dauer(0.5),
            ],
            KugelArt::Mitte => vec![Schritt::neu("abwehr").dauer(0.7), Schritt::neu("stehen_b").dauer(0.5)],
        };
        self.shaia.planen(plan, jetzt);
        self.shaia_aktionen();
    }

    fn kampfbereit(&mut self, von_links: bool) {
        // Weg vom Rand, zu dem das Skelett hereinkommt, und in seine Richtung schauen.
        let (sh_x, k) = (self.shaia.x, self.kamera);
        let ziel = if von_links { sh_x.max(k + 250.0) } else { sh_x.min(k + WF - 250.0) };
        let blick = if von_links { -1 } else { 1 };
        let mut plan = if (ziel - sh_x).abs() > 8.0 { vec![self.gehen_nach(ziel)] } else { vec![] };
        plan.push(Schritt::neu("drehen").start(Aktion::Richtung(blick)));
        plan.push(Schritt::neu("stehen_b").fertig(Fertig::GegnerKommtNicht));
        self.im_kampf = true;
        self.shaia.planen(plan, self.jetzt);
        self.shaia_aktionen();
    }

    fn shaia_planen(&mut self, stunde: u32) {
        let jetzt = self.jetzt;
        if !self.shaia.frei() {
            return;
        }
        if self.shaia_offline {
            // Kein Netz: Shaia wartet geduldig, schaut sich um und denkt nach - Feinde gibt es keine
            let wahl = self.rng.f64();
            let plan = if wahl < 0.45 {
                let d = self.uniform(5.0, 9.0);
                vec![Schritt::neu("stehen_a").dauer(d)]
            } else if wahl < 0.75 {
                let d = self.uniform(2.5, 4.0);
                vec![Schritt::neu("nachdenken").dauer(d)]
            } else {
                let neu = -self.shaia.richtung;
                vec![Schritt::neu("drehen").start(Aktion::Richtung(neu)), Schritt::neu("suchen").dauer(3.0)]
            };
            self.shaia.planen(plan, jetzt);
            self.shaia_aktionen();
            return;
        }
        match self.gegner_idx() {
            Some(i) if self.skelette[i].kampffaehig() && self.skelette[i].zustand != Zustand::Kommt => {
                self.im_kampf = true;
                let richtung = if self.skelette[i].a.x > self.shaia.x { 1 } else { -1 };
                if richtung != self.shaia.richtung {
                    self.shaia.planen(vec![Schritt::neu("drehen").start(Aktion::Richtung(richtung))], jetzt);
                    self.shaia_aktionen();
                } else {
                    self.angriff_planen(i);
                }
            }
            Some(_) => self.shaia.planen(vec![Schritt::neu("stehen_b").dauer(0.4)], jetzt),
            None => self.alltag_planen(stunde),
        }
    }

    fn skelett_planen(&mut self, i: usize) {
        let jetzt = self.jetzt;
        if !self.skelette[i].a.frei() || self.skelette[i].zustand == Zustand::Tot {
            return;
        }
        if matches!(self.skelette[i].zustand, Zustand::Getroffen | Zustand::Betaeubt) {
            self.skelette[i].zustand = Zustand::Steht;
        }
        let (sx, sh_x, kamera) = (self.skelette[i].a.x, self.shaia.x, self.kamera);
        let dx = sh_x - sx;
        self.skelette[i].a.richtung = if dx > 0.0 { 1 } else { -1 };
        if self.skelette[i].zustand == Zustand::Kommt {
            if kamera + 90.0 <= sx && sx <= kamera + WF - 90.0 || dx.abs() <= 125.0 {
                self.skelette[i].zustand = Zustand::Steht;
            } else {
                self.skelette[i].a.planen(vec![Schritt::neu("gehen").vx(55.0).dauer(0.25)], jetzt);
                return;
            }
        }
        let (springer, sprung_ab, angriff_ab) = (self.skelette[i].springer, self.skelette[i].sprung_ab, self.skelette[i].angriff_ab);
        if springer && dx.abs() > 140.0 && dx.abs() < 330.0 && jetzt > sprung_ab && !self.shaia_offline
            && self.shaia.hoehe == 0.0 && self.rng.f64() < 0.5
        {
            self.skelett_springt(i);
            return;
        }
        if dx.abs() > 125.0 {
            self.skelette[i].a.planen(vec![Schritt::neu("gehen").vx(48.0).dauer(0.25)], jetzt);
        } else if jetzt > angriff_ab && self.rng.f64() < 0.55 && !self.shaia_offline {
            let nxt = jetzt + self.uniform(1.4, 3.0);
            self.skelette[i].angriff_ab = nxt;
            self.skelette[i].a.planen(vec![Schritt::neu("angriff").treffer(3, tr(1, 150.0, Art::Normal))], jetzt);
        } else {
            self.skelette[i].a.planen(vec![Schritt::neu("stehen").dauer(0.4)], jetzt);
        }
        self.skelett_aktionen(i);
    }

    // ---------- ein Takt ----------
    pub fn schritt(&mut self, dt: f64, stunde: u32, jetzt: f64, offline: bool, getippt: bool) {
        self.jetzt = jetzt;
        if offline && !self.shaia_offline {
            self.shaia_offline = true;
            self.shaia.hoehe = 0.0;
            self.shaia.vy = 0.0;
            self.shaia.vx_luft = 0.0;
            self.shaia.planen(vec![Schritt::neu("nachdenken").dauer(3.0)], jetzt);
            // Ohne Netz kein Kampf: Feinde und Geschosse verschwinden im Rauch, nichts staut sich auf
            self.wartend = 0;
            self.kugeln.clear();
            self.geschosse.clear();
            self.im_kampf = false;
            let weg: Vec<f64> = self.skelette.iter().filter(|s| s.zustand != Zustand::Tot).map(|s| s.a.x).collect();
            for x in weg {
                self.effekt("smoke", x - 64.0, FUSSF - 128.0, 12.0);
            }
            self.skelette.clear();
        } else if !offline && self.shaia_offline {
            self.shaia_offline = false;
            self.shaia.planen(vec![Schritt::neu("lachen").dauer(1.2), Schritt::neu("stehen_a").dauer(1.0)], jetzt);
        }

        // Gegner losschicken: für jede geblockte Anfrage eins, sonst ab und zu ein Bonus-Skelett
        if !offline && self.shaia.anim() != Some("lagerfeuer") && self.gegner_idx().is_none()
            && (self.wartend > 0 || jetzt > self.bonus_ab)
        {
            if self.wartend > 0 {
                self.wartend -= 1;
            } else {
                self.bonus_ab = jetzt + self.uniform(60.0, 120.0);
            }
            let von_links = self.rng.f64() < 0.5;
            let start_x = if von_links { self.kamera - 70.0 } else { self.kamera + WF + 70.0 };
            let springer = self.rng.f64() < 0.5;
            let lp = self.rng.i32(3..=6);
            let id = self.naechste_id;
            self.naechste_id += 1;
            self.skelette.push(Skelett {
                id, a: Akteur::neu(start_x, if von_links { 1 } else { -1 }, 1, "stehen"), lp, zustand: Zustand::Kommt,
                angriff_ab: 0.0, springer, sprung_ab: 0.0, teile: vec![], tot_seit: None,
            });
            self.truhe_weg();
            if !self.shaia_offline && im_alltag(self.shaia.anim()) && self.shaia.hoehe == 0.0 {
                self.kampfbereit(von_links);
            }
        }

        if getippt && !self.shaia_offline && self.shaia.hoehe == 0.0 && !self.im_kampf && im_alltag(self.shaia.anim())
            && self.shaia.anim() != Some("lagerfeuer")
        {
            self.shaia.planen(
                vec![
                    Schritt::neu("absprung").dauer(0.08),
                    Schritt::neu("steigen").start(Aktion::Hoch(330.0)).fertig(Fertig::Gipfel),
                    Schritt::neu("fallen").fertig(Fertig::Gelandet),
                    Schritt::neu("landen"),
                ],
                jetzt,
            );
            self.shaia_aktionen();
        }

        if self.gegner_idx().is_none() {
            if self.im_kampf {
                self.lager_ab = jetzt + 8.0;
            }
            self.im_kampf = false;
        }
        self.shaia_planen(stunde);

        let ctx = self.kontext();
        let treffer = self.shaia.tick(dt, jetzt, &self.a.shaia, &ctx);
        self.shaia_aktionen();
        for t in treffer {
            if let Some(i) = self.gegner_idx() {
                self.treffe(i, t);
            }
        }
        self.shaia.x = self.shaia.x.clamp(50.0, HF - 50.0);
        self.shaia_planen(stunde);

        // Skelette
        for i in 0..self.skelette.len() {
            if self.skelette[i].zustand == Zustand::Tot {
                for t in &mut self.skelette[i].teile {
                    t.x += t.vx * dt;
                    t.y += t.vy * dt;
                    t.vy += 700.0 * dt;
                    if t.y > FUSSF - 8.0 {
                        t.y = FUSSF - 8.0;
                        t.vy = -t.vy * 0.3;
                        t.vx *= 0.6;
                    }
                }
                continue;
            }
            self.skelett_planen(i);
            let ctx = self.kontext();
            let treffer = self.skelette[i].a.tick(dt, jetzt, &self.a.skelett, &ctx);
            self.skelett_aktionen(i);
            for _ in treffer {
                self.skelett_schlaegt(i);
            }
            if self.skelette[i].zustand != Zustand::Kommt {
                self.skelette[i].a.x = self.skelette[i].a.x.clamp(50.0, HF - 50.0); // Hallenwände
            }
            self.skelett_planen(i);
        }
        self.skelette.retain(|s| !s.tot_seit.is_some_and(|t| jetzt - t > 1.8));
        self.kugeln_bewegen(dt);
        let mut g = 0;
        while g < self.geschosse.len() {
            let ziel_idx = self.idx(self.geschosse[g].ziel);
            let (zx, kampf) = ziel_idx.map_or((0.0, false), |i| (self.skelette[i].a.x, self.skelette[i].kampffaehig()));
            let richtung = if zx > self.geschosse[g].x { 1.0 } else { -1.0 };
            self.geschosse[g].x += richtung * 420.0 * dt;
            self.geschosse[g].y += (FUSSF - 100.0 - self.geschosse[g].y) * (3.0 * dt).min(1.0);
            if (zx - self.geschosse[g].x).abs() < 30.0 || !kampf {
                let element = self.geschosse[g].element;
                self.geschosse.remove(g);
                if let (true, Some(i)) = (kampf, ziel_idx) {
                    self.treffe(i, tr(2, 10000.0, element_art(element)));
                }
                continue;
            }
            g += 1;
        }
        self.kamera_fuehren(dt);
        self.effekte.retain(|e| (jetzt - e.start) * e.fps < e.reihe.len() as f64);
    }

    /// Im Kampf beide im Bild halten, sonst nur nachziehen, wenn Shaia dem Rand nahe kommt.
    /// Die Kamera beschleunigt und bremst sanft - gleichmäßige Schritte ruckeln weniger.
    fn kamera_fuehren(&mut self, dt: f64) {
        let (sh_x, k) = (self.shaia.x, self.kamera);
        let ziel = match self.gegner_idx() {
            Some(i) if self.im_kampf && self.skelette[i].zustand != Zustand::Kommt => (sh_x + self.skelette[i].a.x) / 2.0 - WF / 2.0,
            _ if self.shaia.anim() == Some("lagerfeuer") => sh_x + 65.0 * self.shaia.richtung as f64 - WF / 2.0,
            _ if self.truhe_besuch => (sh_x + self.truhe_x) / 2.0 - WF / 2.0,
            _ if sh_x - k < RAND => sh_x - RAND,
            _ if sh_x - k > WF - RAND => sh_x - (WF - RAND),
            _ => k,
        };
        let ziel = ziel.clamp(0.0, HF - WF);
        // Wunschtempo wächst mit dem Abstand, Tempoänderung ist begrenzt
        let wunsch = ((ziel - k) * 2.5).clamp(-KAMERA_TEMPO, KAMERA_TEMPO);
        let aenderung = KAMERA_BESCHL * dt;
        self.kamera_v += (wunsch - self.kamera_v).clamp(-aenderung, aenderung);
        self.kamera = (k + self.kamera_v * dt).clamp(0.0, HF - WF);
        if self.kamera == 0.0 || self.kamera == HF - WF {
            self.kamera_v = 0.0;
        }
    }

    // ---------- zeichnen ----------
    pub fn zeichnen(&mut self, jetzt: f64) -> Img {
        let k = self.kamera.round() as i32;
        let mut bild = if self.nacht { &self.a.halle_nacht } else { &self.a.halle }.ausschnitt(k, 0, W, H);
        for kugel in &self.kugeln {
            let reihe = &self.a.kugel_bilder[kugel.element];
            let b = &reihe[(jetzt * 14.0) as usize % reihe.len()];
            bild.darueber(b, (kugel.x - b.w as f64 / 2.0) as i32 - k, (FUSSF - kugel.art.hoehe() - b.h as f64 / 2.0) as i32);
        }
        for be in &self.beute {
            let b = &be.bilder[(jetzt * 10.0) as usize % be.bilder.len()];
            let phase = (jetzt - be.seit).rem_euclid(1.6); // kurzes Funkeln alle 1,6 s
            if phase < 0.75 {
                let fk = &self.a.funkeln_klein[(phase * 8.0) as usize % self.a.funkeln_klein.len()];
                bild.darueber(fk, (be.x + 6.0) as i32 - k, FUSS - b.h as i32 - 14);
            }
            let schweben = if be.bilder.len() > 1 { (3.0 * ((jetzt - be.seit) * 3.0).sin()) as i32 } else { 0 };
            bild.darueber(b, (be.x - b.w as f64 / 2.0) as i32 - k, FUSS - b.h as i32 - 2 - schweben);
        }
        let n = self.a.truhe.len();
        let i = if let Some(zu) = self.truhe_zu_seit {
            // schließt sich rückwärts
            let i = n as i32 - 1 - ((jetzt - zu) * 6.0) as i32;
            if i <= 0 {
                self.truhe_seit = None;
                self.truhe_zu_seit = None;
                0
            } else {
                i as usize
            }
        } else {
            self.truhe_seit.map_or(0, |s| (((jetzt - s) * 6.0) as usize).min(n - 1))
        };
        bild.darueber(&self.a.truhe[i], (self.truhe_x - 64.0) as i32 - k, FUSS - 110);
        if self.nacht {
            // festes Lager: Feuer brennt die ganze Nacht
            let f = &self.a.feuer[(jetzt * 10.0) as usize % self.a.feuer.len()];
            bild.darueber(f, LAGER_X as i32 + 70 - 32 - k, FUSS - 50);
        }
        for s in &self.skelette {
            if s.zustand != Zustand::Tot {
                let (b, x, y) = s.a.bild(jetzt, &self.a.skelett, 0);
                bild.darueber(b, x - k, y);
                if s.zustand == Zustand::Betaeubt {
                    let stern = &self.a.vfx["stun"][(jetzt * 12.0) as usize % self.a.vfx["stun"].len()];
                    bild.darueber(stern, (s.a.x - 32.0) as i32 - k, FUSS - 150);
                }
            }
            if let Some(tot) = s.tot_seit {
                for t in &s.teile {
                    let st = &t.reihe[((jetzt - tot) * 16.0) as usize % t.reihe.len()];
                    bild.darueber(st, (t.x - st.w as f64 / 2.0) as i32 - k, (t.y - st.h as f64 / 2.0) as i32);
                }
            }
        }
        let heben = if self.shaia.anim() == Some("lagerfeuer") { 5 } else { 0 };
        let (b, x, y) = self.shaia.bild(jetzt, &self.a.shaia, heben);
        bild.darueber(b, x - k, y);
        for g in &self.geschosse {
            let o = &g.bilder[(jetzt * 14.0) as usize % g.bilder.len()];
            bild.darueber(o, (g.x - o.w as f64 / 2.0) as i32 - k, (g.y - o.h as f64 / 2.0) as i32);
        }
        for e in &self.effekte {
            let nr = (((jetzt - e.start) * e.fps) as usize).min(e.reihe.len() - 1);
            bild.darueber(&e.reihe[nr], e.x as i32 - k, e.y as i32);
        }
        if let Some(s) = &self.statisch {
            for (y, art) in self.statisch_zeilen.iter().enumerate() {
                let (von, bis) = (y * W * 4, (y + 1) * W * 4);
                match art {
                    0 => {}
                    1 => bild.d[von..bis].copy_from_slice(&s.d[von..bis]),
                    _ => {
                        for (z, q) in bild.d[von..bis].chunks_exact_mut(4).zip(s.d[von..bis].chunks_exact(4)) {
                            crate::gfx::mische(z, q);
                        }
                    }
                }
            }
        }
        bild
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rueckstau_ist_auch_bei_grossen_zaehlern_begrenzt() {
        let mut s = Szene::neu(test_assets(), Some(1), 12, 1000.0);
        s.skelette_dazu(1);
        s.skelette_dazu(u32::MAX);
        assert_eq!(s.wartend, 6);
        s.shaia_offline = true;
        s.skelette_dazu(u32::MAX);
        assert_eq!(s.wartend, 6);
    }

    #[test]
    fn treffer_wird_nur_einmal_ausgeloest_und_schritt_endet() {
        let assets = test_assets();
        let mut a = Akteur::neu(100.0, 1, 0, "stehen_a");
        let ctx = Kontext { skelette: vec![], gegner_kommt: None };
        a.planen(vec![Schritt::neu("knie").dauer(1.0).treffer(1, tr(1, 100.0, Art::Normal))], 10.0);
        assert!(a.tick(0.01, 10.0, &assets.shaia, &ctx).is_empty());
        assert_eq!(a.tick(0.1, 10.2, &assets.shaia, &ctx).len(), 1);
        assert!(a.tick(0.1, 10.4, &assets.shaia, &ctx).is_empty());
        a.tick(0.1, 11.0, &assets.shaia, &ctx);
        assert!(a.frei());
    }

    #[test]
    fn offline_raeumt_kampf_auf_und_erholt_sich() {
        let mut s = Szene::neu(test_assets(), Some(1), 12, 1000.0);
        s.skelette_dazu(6);
        s.schritt(1.0 / 24.0, 12, 1000.0, false, false);
        assert!(!s.skelette.is_empty());
        s.schritt(1.0 / 24.0, 12, 1000.1, true, true);
        assert!(s.shaia_offline && !s.im_kampf);
        assert!(s.skelette.is_empty() && s.kugeln.is_empty() && s.geschosse.is_empty());
        assert_eq!(s.wartend, 0);
        assert_eq!(s.shaia.hoehe, 0.0);
        assert_eq!(s.shaia.anim(), Some("nachdenken"));
        s.schritt(1.0 / 24.0, 12, 1000.2, false, false);
        assert!(!s.shaia_offline);
        assert_eq!(s.shaia.anim(), Some("lachen"));
    }
}
