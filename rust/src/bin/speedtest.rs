//! Misst die Internet-Geschwindigkeit über Cloudflare und legt das Ergebnis für die Anzeige in
//! /var/lib/pi-anzeige/internet.json ab (wie speedtest.py). Läuft per systemd-Timer.
//!
//! Mehrere parallele Downloads/Uploads über einige Sekunden, weil Cloudflare einzelne Downloads auf
//! etwa 25 MB begrenzt.

use std::io::Read;
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DAUER: Duration = Duration::from_secs(6); // je Richtung
const FAEDEN: usize = 4; // parallele Verbindungen
const UA: &str = "pi-anzeige";

fn ping_ms() -> Option<f64> {
    let adr: SocketAddr = "1.1.1.1:443".parse().unwrap();
    let mut zeiten = vec![];
    for _ in 0..3 {
        let t = Instant::now();
        TcpStream::connect_timeout(&adr, Duration::from_secs(5)).ok()?;
        zeiten.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    zeiten.into_iter().reduce(f64::min)
}

fn download() -> Result<u64, String> {
    let antwort = ureq::get("https://speed.cloudflare.com/__down?bytes=25000000")
        .set("User-Agent", UA)
        .timeout(Duration::from_secs(30))
        .call()
        .map_err(|e| e.to_string())?;
    let mut leser = antwort.into_reader();
    let (mut n, mut block) = (0u64, vec![0u8; 1 << 16]);
    loop {
        let k = leser.read(&mut block).map_err(|e| e.to_string())?;
        if k == 0 {
            return Ok(n);
        }
        n += k as u64;
    }
}

fn upload(stueck: &[u8]) -> Result<u64, String> {
    ureq::post("https://speed.cloudflare.com/__up")
        .set("User-Agent", UA)
        .set("Content-Type", "application/octet-stream")
        .timeout(Duration::from_secs(30))
        .send_bytes(stueck)
        .map_err(|e| e.to_string())?;
    Ok(stueck.len() as u64)
}

/// Mbit/s über `DAUER` mit `FAEDEN` parallelen Verbindungen.
fn messen(arbeit: impl Fn() -> Result<u64, String> + Send + Sync + 'static) -> Result<f64, String> {
    let (summe, arbeit) = (Arc::new(AtomicU64::new(0)), Arc::new(arbeit));
    let fehler = Arc::new(std::sync::Mutex::new(None::<String>));
    let start = Instant::now();
    let faeden: Vec<_> = (0..FAEDEN)
        .map(|_| {
            let (summe, arbeit, fehler) = (summe.clone(), arbeit.clone(), fehler.clone());
            std::thread::spawn(move || {
                while start.elapsed() < DAUER {
                    match arbeit() {
                        Ok(n) => {
                            summe.fetch_add(n, Ordering::Relaxed);
                        }
                        Err(e) => {
                            *fehler.lock().unwrap() = Some(e);
                            return;
                        }
                    }
                }
            })
        })
        .collect();
    for f in faeden {
        let _ = f.join();
    }
    let n = summe.load(Ordering::Relaxed);
    if n == 0 {
        return Err(format!("keine Daten übertragen: {}", fehler.lock().unwrap().clone().unwrap_or("unbekannt".into())));
    }
    Ok(n as f64 * 8.0 / start.elapsed().as_secs_f64() / 1e6)
}

fn runden1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn jetzt() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

fn fehler_ergebnis(text: &str, fehler: &str, zeit: u64) -> serde_json::Value {
    let mut alt: serde_json::Value = serde_json::from_str(text).ok()
        .filter(serde_json::Value::is_object).unwrap_or_else(|| serde_json::json!({}));
    alt["fehler"] = fehler.chars().take(200).collect::<String>().into();
    alt["fehler_zeit"] = zeit.into();
    alt
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ziel = std::env::var("PI_SPEED_DATEI").unwrap_or_else(|_| "/var/lib/pi-anzeige/internet.json".into());
    let stueck = Arc::new(vec![b'0'; 5_000_000]);
    let ergebnis: Result<serde_json::Value, String> = (|| {
        let ping = ping_ms().ok_or("kein Ping")?;
        let down = messen(download)?;
        let st = stueck.clone();
        let up = messen(move || upload(&st))?;
        Ok(serde_json::json!({"zeit": jetzt(), "ping_ms": runden1(ping), "down_mbit": runden1(down), "up_mbit": runden1(up)}))
    })();
    let j = match ergebnis {
        Ok(j) => j,
        Err(e) => {
            // Letzte gute Messung behalten, nur den Fehler dazuschreiben.
            let alt = std::fs::read_to_string(&ziel).unwrap_or_default();
            fehler_ergebnis(&alt, &e, jetzt())
        }
    };
    let tmp = format!("{ziel}.tmp");
    std::fs::write(&tmp, j.to_string())?;
    std::fs::rename(&tmp, &ziel)?;
    println!("{j}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fehler_behaelt_die_letzte_messung() {
        let j = fehler_ergebnis(r#"{"down_mbit":123.4,"zeit":7}"#, "offline", 42);
        assert_eq!(j["down_mbit"], 123.4);
        assert_eq!(j["zeit"], 7);
        assert_eq!(j["fehler"], "offline");
        assert_eq!(j["fehler_zeit"], 42);
    }

    #[test]
    fn fehler_vertraegt_fremdes_oder_kaputtes_json() {
        for text in ["", "{", "null", "[]", "123", r#""text""#] {
            let j = fehler_ergebnis(text, &"ü".repeat(300), 42);
            assert_eq!(j["fehler"].as_str().unwrap().chars().count(), 200);
            assert_eq!(j["fehler_zeit"], 42);
        }
    }
}
