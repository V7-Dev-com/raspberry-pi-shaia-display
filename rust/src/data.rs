//! Daten für die Anzeige: Wetter (Open-Meteo), Pi-hole (FTL-Datenbank), Ping, Speedtest-Datei,
//! CPU-Temperatur, Laufzeit.

use chrono::{DateTime, Local, TimeZone};
use serde::Deserialize;
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

pub const WETTER_ALLE: f64 = 900.0;
pub const PIHOLE_ALLE: f64 = 60.0;
pub const NETZ_ALLE: f64 = 60.0;

const BREITE: f64 = 35.17; // Nikosia
const LAENGE: f64 = 33.35;
pub const FTL_DB: &str = "/etc/pihole/pihole-FTL.db";
pub const SPEED_DATEI: &str = "/var/lib/pi-anzeige/internet.json";
/// Pi-hole-v6-Status, die als "geblockt" zählen
const GEBLOCKT: [i64; 12] = [1, 4, 5, 6, 7, 8, 9, 10, 11, 15, 16, 18];

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Aktuell {
    pub temperature_2m: f64,
    pub apparent_temperature: f64,
    pub weather_code: i64,
    pub wind_speed_10m: f64,
    pub relative_humidity_2m: f64,
    pub is_day: Option<i64>,
    pub uv_index: Option<f64>,
    pub wind_gusts_10m: Option<f64>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Taeglich {
    pub temperature_2m_max: Vec<f64>,
    pub temperature_2m_min: Vec<f64>,
    #[serde(default)]
    pub precipitation_probability_max: Vec<Option<f64>>,
}

#[derive(Deserialize, Clone, Debug, Default)]
pub struct Wetter {
    pub current: Aktuell,
    pub daily: Taeglich,
}

impl Wetter {
    /// Python: c.get("is_day", 1)
    pub fn ist_tag(&self) -> bool {
        self.current.is_day.unwrap_or(1) != 0
    }
}

#[derive(Clone, Debug, Default)]
pub struct Speed {
    pub down_mbit: f64,
    pub up_mbit: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Netz {
    pub ms: Option<f64>,
    pub offline_seit: Option<DateTime<Local>>,
    pub speed: Option<Speed>,
    /// eigene Adresse und Router (Standardgateway), für die Anzeige
    pub pi_ip: Option<String>,
    pub router_ip: Option<String>,
}

impl Netz {
    pub fn offline(&self) -> bool {
        self.ms.is_none()
    }
}

pub fn wetter_holen() -> Result<Wetter, String> {
    let url = format!(
        "https://api.open-meteo.com/v1/forecast?latitude={BREITE}&longitude={LAENGE}\
         &current=temperature_2m,apparent_temperature,weather_code,wind_speed_10m,relative_humidity_2m,is_day,uv_index,wind_gusts_10m\
         &daily=temperature_2m_max,temperature_2m_min,precipitation_probability_max,uv_index_max,wind_gusts_10m_max\
         &timezone=auto&forecast_days=1"
    );
    let antwort = ureq::get(&url).timeout(Duration::from_secs(10)).call().map_err(|e| e.to_string())?;
    antwort.into_json::<Wetter>().map_err(|e| e.to_string())
}

/// (Anfragen heute, davon geblockt) aus der FTL-Datenbank, nur lesend.
pub fn pihole_heute() -> Result<(i64, i64), String> {
    let heute = Local::now().date_naive().and_hms_opt(0, 0, 0).unwrap();
    let seit = Local.from_local_datetime(&heute).earliest().map(|d| d.timestamp()).unwrap_or(0);
    let platzhalter = GEBLOCKT.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let con = rusqlite::Connection::open_with_flags(
        FTL_DB,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| e.to_string())?;
    con.busy_timeout(Duration::from_secs(5)).map_err(|e| e.to_string())?;
    let sql = format!(
        "SELECT COUNT(*), COALESCE(SUM(status IN ({platzhalter})), 0) FROM queries WHERE timestamp >= ?"
    );
    let mut stmt = con.prepare(&sql).map_err(|e| e.to_string())?;
    let mut params: Vec<i64> = GEBLOCKT.to_vec();
    params.push(seit);
    stmt.query_row(rusqlite::params_from_iter(params), |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))
        .map_err(|e| e.to_string())
}

pub fn cpu_temperatur() -> Option<f64> {
    let t = std::fs::read_to_string("/sys/class/thermal/thermal_zone0/temp").ok()?;
    Some(t.trim().parse::<f64>().ok()? / 1000.0)
}

/// Verbindungsaufbau zu zwei öffentlichen DNS-Diensten; liefert ms oder None.
pub fn online_pruefen() -> Option<f64> {
    for ziel in ["1.1.1.1:443", "9.9.9.9:443"] {
        let adr: SocketAddr = ziel.parse().unwrap();
        let t = Instant::now();
        if TcpStream::connect_timeout(&adr, Duration::from_secs(3)).is_ok() {
            return Some(t.elapsed().as_secs_f64() * 1000.0);
        }
    }
    None
}

/// Eigene Adresse im Heimnetz (UDP-Verbindung ohne Datenversand wählt nur die Schnittstelle).
pub fn eigene_ip() -> Option<String> {
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:9").ok()?;
    Some(s.local_addr().ok()?.ip().to_string())
}

/// Standardgateway aus dem Inhalt von /proc/net/route (Adresse steht als Hex, niederwertigstes Byte zuerst).
pub fn gateway_aus_route(route: &str) -> Option<String> {
    route.lines().skip(1).find_map(|z| {
        let f: Vec<&str> = z.split_whitespace().collect();
        if f.len() > 2 && f[1] == "00000000" && f[7] == "00000000" {
            let v = u32::from_str_radix(f[2], 16).ok()?;
            let b = v.to_le_bytes();
            Some(format!("{}.{}.{}.{}", b[0], b[1], b[2], b[3]))
        } else {
            None
        }
    })
}

pub fn router_ip() -> Option<String> {
    gateway_aus_route(&std::fs::read_to_string("/proc/net/route").ok()?)
}

pub fn speed_lesen() -> Option<Speed> {
    let text = std::fs::read_to_string(SPEED_DATEI).ok()?;
    let j: serde_json::Value = serde_json::from_str(&text).ok()?;
    Some(Speed { down_mbit: j.get("down_mbit")?.as_f64()?, up_mbit: j.get("up_mbit")?.as_f64()? })
}

/// "3 T 2 h" oder "5 h 12 min", wie in v1.
pub fn laufzeit() -> String {
    let s = std::fs::read_to_string("/proc/uptime").unwrap_or_default();
    let minuten = s.split_whitespace().next().and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0) as i64 / 60;
    let (tage, rest) = (minuten / 1440, minuten % 1440);
    let (stunden, minuten) = (rest / 60, rest % 60);
    if tage > 0 { format!("{tage} T {stunden} h") } else { format!("{stunden} h {minuten} min") }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wetter_json_wird_gelesen() {
        let j = r#"{"current":{"temperature_2m":16.4,"apparent_temperature":15.0,"weather_code":2,
            "wind_speed_10m":12.0,"relative_humidity_2m":73,"is_day":1,"uv_index":3.2,"wind_gusts_10m":20.5},
            "daily":{"temperature_2m_max":[24.1],"temperature_2m_min":[12.2],"precipitation_probability_max":[null]}}"#;
        let w: Wetter = serde_json::from_str(j).unwrap();
        assert_eq!(w.current.weather_code, 2);
        assert!(w.ist_tag());
        assert_eq!(w.daily.precipitation_probability_max[0], None);
    }

    #[test]
    fn gateway_wird_aus_der_routentabelle_gelesen() {
        let route = "Iface\tDestination\tGateway\tFlags\tRefCnt\tUse\tMetric\tMask\tMTU\tWindow\tIRTT\n\
                     eth0\t00000000\t0101A8C0\t0003\t0\t0\t100\t00000000\t0\t0\t0\n\
                     eth0\t0001A8C0\t00000000\t0001\t0\t0\t100\t00FFFFFF\t0\t0\t0\n";
        assert_eq!(gateway_aus_route(route).as_deref(), Some("192.168.1.1"));
        assert_eq!(gateway_aus_route("Iface\n"), None);
    }

    #[test]
    fn fehlendes_is_day_zaehlt_als_tag() {
        let w = Wetter::default();
        assert!(w.ist_tag());
    }
}
