//! Grafiken laden: Sprite-Reihen (zugeschnitten und gespiegelt), Effekte, Beute, Hintergrund.
//! Die Grafiken liegen nicht im Repo (siehe README); Ordner über `PI_ASSETS`.

use crate::gfx::Img;
use image::imageops::FilterType;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub const W: usize = 480;
pub const H: usize = 320;
pub const FUSS: i32 = 243;
pub const HALLE: usize = 1536;
pub const HINTERGRUND_ZOOM: f32 = 0.80;
pub const HINTERGRUND_HOCH: i32 = 19;
pub const BODEN_ZEILE: f32 = 283.0;
/// Unterordner des Shaia-Pakets (`sprites/`), die vorhanden sein müssen.
pub const BENOETIGT: [&str; 6] = ["shaia", "skeleton", "vfx", "prop", "item", "background"];
pub const SHAIA_URL: &str = "https://picopico256.itch.io/shaia-ruin-runnerscharacter";

/// Auf den sichtbaren Teil zugeschnittenes Einzelbild mit Versatz im 256er-Raster.
pub struct Bild {
    pub bild: Img,
    pub dx: i32,
    pub dy: i32,
}

pub struct Reihe {
    pub rechts: Vec<Bild>,
    pub links: Vec<Bild>,
}

pub struct Anim {
    pub reihe: Reihe,
    pub fps: f32,
}

pub type AnimSatz = HashMap<&'static str, Anim>;

pub fn assets_ordner() -> PathBuf {
    PathBuf::from(std::env::var("PI_ASSETS").unwrap_or_else(|_| "/opt/pi-anzeige/assets".into()))
}

/// Namen der fehlenden Pflichtordner.
pub fn grafiken_fehlen(sprites: &Path) -> Vec<&'static str> {
    BENOETIGT.iter().copied().filter(|n| !sprites.join(n).is_dir()).collect()
}

/// Dateien eines Musters wie `ordner/name_*.png` (Stern nur im Dateinamen), sortiert.
pub fn glob(sprites: &Path, muster: &str) -> Vec<PathBuf> {
    let (ordner, name) = match muster.rfind('/') {
        Some(i) => (&muster[..i], &muster[i + 1..]),
        None => ("", muster),
    };
    let teile: Vec<&str> = name.split('*').collect();
    let mut v: Vec<PathBuf> = std::fs::read_dir(sprites.join(ordner))
        .map(|r| r.filter_map(|e| e.ok()).map(|e| e.path()).collect())
        .unwrap_or_default();
    v.retain(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| passt(n, &teile)));
    v.sort();
    v
}

fn passt(name: &str, teile: &[&str]) -> bool {
    if teile.len() == 1 {
        return name == teile[0];
    }
    let (erst, letzt) = (teile[0], teile[teile.len() - 1]);
    if !name.starts_with(erst) || !name.ends_with(letzt) || name.len() < erst.len() + letzt.len() {
        return false;
    }
    let mut rest = &name[erst.len()..name.len() - letzt.len()];
    for t in &teile[1..teile.len() - 1] {
        match rest.find(t) {
            Some(i) => rest = &rest[i + t.len()..],
            None => return false,
        }
    }
    true
}

/// Einzelbilder (schauen nach rechts) plus gespiegelte, zugeschnitten auf ihren Inhalt.
pub fn lade_reihe(sprites: &Path, muster: &str) -> Result<Reihe, String> {
    let dateien = glob(sprites, muster);
    if dateien.is_empty() {
        return Err(format!("keine Bilder für {muster}"));
    }
    let (mut rechts, mut links) = (Vec::new(), Vec::new());
    for p in dateien {
        let b = Img::aus_datei(&p)?;
        let (x0, y0, x1, y1) = b.inhalt().unwrap_or((0, 0, 1, 1));
        let teil = b.ausschnitt(x0 as i32, y0 as i32, x1 - x0, y1 - y0);
        links.push(Bild { bild: teil.gespiegelt(), dx: (b.w - x1) as i32, dy: y0 as i32 });
        rechts.push(Bild { bild: teil, dx: x0 as i32, dy: y0 as i32 });
    }
    Ok(Reihe { rechts, links })
}

pub fn lade_einfach(sprites: &Path, muster: &str) -> Result<Rc<Vec<Img>>, String> {
    let v: Result<Vec<Img>, String> = glob(sprites, muster).iter().map(|p| Img::aus_datei(p)).collect();
    let v = v?;
    if v.is_empty() {
        return Err(format!("keine Bilder für {muster}"));
    }
    Ok(Rc::new(v))
}

// Animationen: Name -> (Muster, Bilder pro Sekunde)
const SHAIA_ANIM: &[(&str, &str, f32)] = &[
    ("stehen_a", "shaia/sprites_common/common_00_idle_stand_A*.png", 10.0),
    ("stehen_b", "shaia/sprites_common/common_00_idle_stand_B*.png", 10.0),
    ("gehen", "shaia/sprites_common/common_11_walk*.png", 14.0),
    ("rennen", "shaia/sprites_common/common_12_run*.png", 14.0),
    ("drehen", "shaia/sprites_common/common_03_turn_stand_A*.png", 14.0),
    ("ducken_rein", "shaia/sprites_common/common_01_idle_to_crouch*.png", 12.0),
    ("ducken", "shaia/sprites_common/common_01_idle_crouch.png", 4.0),
    ("ducken_raus", "shaia/sprites_common/common_01_crouch_to_idle*.png", 12.0),
    ("absprung", "shaia/sprites_common/common_21_jump_begin*.png", 12.0),
    ("steigen", "shaia/sprites_common/common_21_jump_up*.png", 12.0),
    ("fallen", "shaia/sprites_common/common_21_jump_down*.png", 10.0),
    ("landen", "shaia/sprites_common/common_22_landing*.png", 14.0),
    ("abwehr", "shaia/sprites_common/common_31_guard_stand*.png", 10.0),
    ("kombo1", "shaia/sprites_attack/attack_01_cobination*.png", 16.0),
    ("kombo2", "shaia/sprites_attack/attack_02_cobination*.png", 16.0),
    ("kombo3", "shaia/sprites_attack/attack_03_cobination*.png", 16.0),
    ("kombo4", "shaia/sprites_attack/attack_04_cobination*.png", 16.0),
    ("knie", "shaia/sprites_attack/attack_05_knee*.png", 14.0),
    ("duckangriff", "shaia/sprites_attack/attack_11_crounch_attack*.png", 16.0),
    ("sprungangriff", "shaia/sprites_attack/attack_21_jump_attack*.png", 14.0),
    ("tritt_rein", "shaia/sprites_attack/attack_06_run_kick_begin*.png", 12.0),
    ("tritt", "shaia/sprites_attack/attack_06_run_kick_0*.png", 10.0),
    ("tritt_raus", "shaia/sprites_attack/attack_06_run_kick_end*.png", 12.0),
    ("schild_rein", "shaia/special/special_01_shield_dash_begin_*.png", 12.0),
    ("schild", "shaia/special/special_01_shield_dash_loop_*.png", 14.0),
    ("schild_raus", "shaia/special/special_01_shield_dash_end_*.png", 12.0),
    ("aufwaerts_rein", "shaia/special/special_02_rising_begin_*.png", 14.0),
    ("aufwaerts", "shaia/special/special_02_rising_attack_*.png", 12.0),
    ("aufwaerts_raus", "shaia/special/special_02_rising_end_*.png", 10.0),
    ("kreisschlag", "shaia/special/special_03_circle_slash_*.png", 18.0),
    ("verzaubern", "shaia/special/special_06_enchant_*.png", 10.0),
    ("zauber_rein", "shaia/special/special_07_magic_begin_*.png", 8.0),
    ("zauber", "shaia/special/special_07_magic_loop_*.png", 10.0),
    ("zauber_raus", "shaia/special/special_07_magic_end_*.png", 8.0),
    ("charme_rein", "shaia/special/special_10_charm_magic_begin_*.png", 10.0),
    ("charme", "shaia/special/special_10_charm_magic_loop_*.png", 4.0),
    ("charme_raus", "shaia/special/special_10_charm_magic_end_*.png", 10.0),
    ("ultimate", "shaia/special/special_11_Ultimate_*.png", 8.0),
    ("treffer_kopf", "shaia/sprites_damage/damage_01_damage_head.png", 4.0),
    ("treffer_koerper", "shaia/sprites_damage/damage_02_damage_body.png", 4.0),
    ("umgeworfen", "shaia/through/special_05_through*.png", 9.0),
    ("aufstehen", "shaia/sprites_damage/damage_13_standup*.png", 10.0),
    ("strom", "shaia/sprites_damage/damage_23_electric*.png", 12.0),
    ("provozieren", "shaia/misc/misc_06_taunt_*.png", 6.0),
    ("lachen", "shaia/misc/misc_03_laugh_*.png", 6.0),
    ("suchen", "shaia/misc/misc_02_search_*.png", 4.0),
    ("nachdenken", "shaia/misc/misc_05_think_*.png", 2.0),
    ("lagerfeuer", "shaia/misc/misc_08_camp_*.png", 3.0),
    ("truhe_rein", "shaia/misc/misk_07_open_chest_begin_*.png", 8.0),
    ("truhe_raus", "shaia/misc/misk_07_open_chest_end_*.png", 8.0),
];

const SKELETT_ANIM: &[(&str, &str, f32)] = &[
    ("stehen", "skeleton/common_01_idle*.png", 4.0),
    ("gehen", "skeleton/common_11_walk*.png", 10.0),
    ("angriff", "skeleton/attack_01_sword*.png", 12.0),
    ("treffer_kopf", "skeleton/damage_01_damage_head.png", 4.0),
    ("treffer_koerper", "skeleton/damage_02_damage_body.png", 4.0),
    ("wurf_rein", "skeleton/damage_11_blow_begin*.png", 12.0),
    ("flug", "skeleton/damage_11_blow_air*.png", 12.0),
    ("liegen", "skeleton/damage_11_blow_landing*.png", 4.0),
    ("aufstehen", "skeleton/damage_13_standup*.png", 10.0),
    ("sprung_rein", "skeleton/common_21_jump_begin*.png", 12.0),
    ("sprung_hoch", "skeleton/common_21_jump_up*.png", 8.0),
    ("sprung_runter", "skeleton/common_21_jump_down*.png", 8.0),
    ("landung", "skeleton/common_22_landing*.png", 12.0),
];

fn lade_satz(sprites: &Path, tabelle: &[(&'static str, &str, f32)]) -> Result<AnimSatz, String> {
    let mut m = HashMap::new();
    for (name, muster, fps) in tabelle {
        m.insert(*name, Anim { reihe: lade_reihe(sprites, muster)?, fps: *fps });
    }
    Ok(m)
}

pub const ELEMENTE: [&str; 5] = ["fire", "ice", "thunder", "poison", "cure"];
pub const KUGEL_ELEMENTE: [&str; 4] = ["fire", "ice", "thunder", "poison"];
pub const ESSEN: [&str; 6] = ["chicken", "cheese", "bread", "potion_red", "potion_blue", "potion_green"];
pub const VFX_NAMEN: [&str; 8] = ["hit", "coin", "guard", "stun", "fire_banish", "ground_shock", "twinkle", "smoke"];
pub const TRUEMMER_NAMEN: [&str; 6] = ["bone", "skul", "shiield", "sword", "waist", "chest"];

pub struct Assets {
    pub shaia: AnimSatz,
    pub skelett: AnimSatz,
    pub truemmer: Vec<Rc<Vec<Img>>>,
    pub vfx: HashMap<&'static str, Rc<Vec<Img>>>,
    pub feuer: Rc<Vec<Img>>,
    pub feuer_klein: Rc<Vec<Img>>,
    pub truhe: Rc<Vec<Img>>,
    pub orbs: HashMap<&'static str, Rc<Vec<Img>>>,
    pub essen: HashMap<&'static str, Rc<Vec<Img>>>,
    /// fliegende Kugeln: Orbs ohne Bild 4 (fast reines Weiß, wirkte wie ein Mond)
    pub kugel_bilder: HashMap<&'static str, Rc<Vec<Img>>>,
    pub funkeln_klein: Rc<Vec<Img>>,
    pub halle: Img,
}

impl Assets {
    pub fn laden(sprites: &Path) -> Result<Assets, String> {
        let shaia = lade_satz(sprites, SHAIA_ANIM)?;
        let skelett = lade_satz(sprites, SKELETT_ANIM)?;
        let mut truemmer = Vec::new();
        for n in TRUEMMER_NAMEN {
            let r = lade_reihe(sprites, &format!("skeleton/debris_{n}*.png"))?;
            truemmer.push(Rc::new(r.rechts.into_iter().map(|b| b.bild).collect()));
        }
        let mut vfx = HashMap::new();
        for n in VFX_NAMEN {
            vfx.insert(n, lade_einfach(sprites, &format!("vfx/vfx_{n}/vfx_{n}*.png"))?);
        }
        let mut orbs = HashMap::new();
        for e in ELEMENTE {
            orbs.insert(e, lade_einfach(sprites, &format!("item/item_orb/item_orb_{e}*.png"))?);
        }
        let mut essen = HashMap::new();
        for n in ESSEN {
            essen.insert(n, Rc::new(vec![Img::aus_datei(&sprites.join(format!("item/item_food/item_{n}.png")))?]));
        }
        let mut kugel_bilder = HashMap::new();
        for e in KUGEL_ELEMENTE {
            let v: Vec<Img> = orbs[e].iter().enumerate().filter(|(i, _)| *i != 3).map(|(_, b)| b.clone()).collect();
            kugel_bilder.insert(e, Rc::new(v));
        }
        let funkeln_klein = Rc::new(
            vfx["twinkle"].iter().map(|b| b.skaliert((b.w / 2).max(1), (b.h / 2).max(1), FilterType::Nearest)).collect(),
        );
        Ok(Assets {
            shaia,
            skelett,
            truemmer,
            feuer: lade_einfach(sprites, "shaia/misc/misc_08_fire_*.png")?,
            feuer_klein: lade_einfach(sprites, "vfx/vfx_fire/vfx_fire*.png")?,
            truhe: lade_einfach(sprites, "prop/chest_open*.png")?,
            vfx,
            orbs,
            essen,
            kugel_bilder,
            funkeln_klein,
            halle: lade_halle(sprites)?,
        })
    }
}

/// Hintergrund etwas herausgezoomt, damit oben und unten mehr von der Halle zu sehen ist.
/// Der Boden (Bildzeile `BODEN_ZEILE` des Originals) bleibt bei `FUSS`, die Figuren stehen also gleich.
pub fn lade_halle(sprites: &Path) -> Result<Img, String> {
    let z = HINTERGRUND_ZOOM;
    let lade = |n: &str| -> Result<Img, String> {
        let t = Img::aus_datei(&sprites.join(format!("background/sprites_dungeon/{n}.png")))?;
        let (nw, nh) = ((t.w as f32 * z).round() as usize, (t.h as f32 * z).round() as usize);
        Ok(t.skaliert(nw, nh, FilterType::Lanczos3))
    };
    let (links_t, mitte, rechts_t) = (lade("01_dungeon_left")?, lade("02_dungeon_center")?, lade("03_dungeon_right")?);
    let mut reihe: Vec<&Img> = vec![&links_t];
    while reihe.iter().map(|b| b.w).sum::<usize>() + rechts_t.w < HALLE {
        reihe.push(&mitte);
    }
    reihe.push(&rechts_t);
    let breite: usize = reihe.iter().map(|b| b.w).sum();
    let mut panorama = Img::neu(breite, links_t.h);
    let mut x = 0;
    for b in &reihe {
        panorama.einsetzen(b, x as i32, 0);
        x += b.w;
    }
    let links = ((breite - HALLE) / 2) as i32;
    let versatz_y = FUSS - (BODEN_ZEILE * z).round() as i32 - HINTERGRUND_HOCH;
    let mut halle = Img::neu(HALLE, H);
    halle.einsetzen(&panorama.ausschnitt(links, 0, HALLE, panorama.h), 0, versatz_y);
    Ok(halle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn muster_mit_stern() {
        assert!(passt("common_11_walk01.png", &["common_11_walk", ".png"]));
        assert!(!passt("common_11_run01.png", &["common_11_walk", ".png"]));
        assert!(passt("a.png", &["a.png"]));
        assert!(passt("attack_06_run_kick_01.png", &["attack_06_run_kick_0", ".png"]));
        assert!(!passt("attack_06_run_kick_end01.png", &["attack_06_run_kick_0", ".png"]));
    }

    #[test]
    fn fehlende_ordner_werden_genannt() {
        let f = grafiken_fehlen(Path::new("/gibt/es/nicht"));
        assert_eq!(f.len(), 6);
    }
}
