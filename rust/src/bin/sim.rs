//! Dauerlauf ohne Display: simuliert viele Stunden Szene mit Skeletten, Antippen, Offline und
//! Tag/Nacht und prüft, dass nichts abstürzt, nichts endlos wächst und alle Zustände vorkommen.
use shaia_display::assets::{assets_ordner, Assets};
use shaia_display::scene::*;
use std::collections::BTreeMap;
use std::rc::Rc;

fn main() {
    let stunden: u32 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(24);
    let seed: u64 = std::env::args().nth(2).and_then(|s| s.parse().ok()).unwrap_or(1);
    let a = Rc::new(Assets::laden(&assets_ordner().join("shaia")).expect("Grafiken"));
    let mut sz = Szene::neu(a, Some(seed), 0, 1000.0);
    let mut zufall = fastrand::Rng::with_seed(seed + 99);
    let (dt, mut t) = (1.0 / 15.0, 1000.0);
    let mut zaehler: BTreeMap<String, u64> = BTreeMap::new();
    let (mut max_skelette, mut max_wartend) = (0, 0);
    let mut offline_bis = 0.0;
    let schritte = (stunden as u64) * 3600 * 15;
    for i in 0..schritte {
        let sim_s = i as f64 / 15.0;
        let stunde = ((sim_s / 3600.0) as u32) % 24;
        sz.nacht = stunde >= NACHT.0 || stunde < NACHT.1;
        if zufall.f64() < 1.0 / (15.0 * 25.0) {
            sz.skelette_dazu(zufall.u32(1..=3));
        }
        if sim_s > offline_bis && zufall.f64() < 1.0 / (15.0 * 1800.0) {
            offline_bis = sim_s + 120.0;
        }
        let getippt = zufall.f64() < 1.0 / (15.0 * 200.0);
        sz.schritt(dt, stunde, t, sim_s < offline_bis, getippt);
        t += dt;
        if let Some(a) = sz.shaia.anim() {
            *zaehler.entry(format!("shaia:{a}")).or_default() += 1;
        }
        for s in &sz.skelette {
            if let Some(a) = s.a.anim() {
                *zaehler.entry(format!("skelett:{a}")).or_default() += 1;
            }
        }
        if sz.truhe_besuch {
            *zaehler.entry("truhe_besuch".into()).or_default() += 1;
        }
        max_skelette = max_skelette.max(sz.skelette.len());
        max_wartend = max_wartend.max(sz.wartend);
        assert!(sz.shaia.x >= 50.0 && sz.shaia.x <= 1486.0, "Shaia außerhalb: {}", sz.shaia.x);
        assert!(sz.kamera >= 0.0 && sz.kamera <= 1056.0, "Kamera außerhalb: {}", sz.kamera);
        assert!(sz.shaia.hoehe >= 0.0 && sz.shaia.hoehe < 200.0, "Höhe: {}", sz.shaia.hoehe);
        if i % 1500 == 0 {
            let b = sz.zeichnen(t);
            assert_eq!((b.w, b.h), (480, 320));
        }
    }
    println!("{stunden} h simuliert (Seed {seed}). max. Skelette gleichzeitig {max_skelette}, wartend {max_wartend}");
    for (k, v) in &zaehler {
        println!("{k:32} {:>9.1} s", *v as f64 * dt);
    }
}
