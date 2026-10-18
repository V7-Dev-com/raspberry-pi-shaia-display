//! Bilder aufs Display: bevorzugt direkt über DRM/KMS (Treiber ili9486, Dumb-Buffer RG16,
//! `dirty_framebuffer` mit Rechteck), sonst über /dev/fb0; für Tests als PNG-Datei.

use crate::assets::{H, W};
use crate::gfx::Img;
use drm::buffer::{Buffer, DrmFourcc};
use drm::control::{connector, framebuffer, ClipRect, Device as ControlDevice};
use drm::Device;
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::os::fd::{AsFd, BorrowedFd};
use std::path::PathBuf;

struct Karte(File);
impl AsFd for Karte {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}
impl Device for Karte {}
impl ControlDevice for Karte {}

pub struct Kms {
    karte: Karte,
    db: drm::control::dumbbuffer::DumbBuffer,
    fb: framebuffer::Handle,
}

pub enum Ausgabe {
    Kms(Kms),
    Fb0(File),
    /// Test: schreibt das Bild etwa einmal pro Sekunde in diese Datei
    Png { pfad: PathBuf, letzte: std::time::Instant },
}

/// Takt der Display-Leitung, wie ihn das System gesetzt hat (dtoverlay=piscreen,...,speed=...).
pub fn spi_takt() -> f64 {
    std::fs::read("/sys/bus/spi/devices/spi0.0/of_node/spi-max-frequency")
        .ok()
        .filter(|b| b.len() >= 4)
        .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]) as f64)
        .filter(|f| *f > 0.0)
        .unwrap_or(24_000_000.0)
}

/// /dev/dri/cardN, dessen Treiber `treiber` heißt.
fn karte_finden(treiber: &str) -> Option<String> {
    let mut namen: Vec<String> = std::fs::read_dir("/sys/class/drm").ok()?.filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("card") && !n.contains('-'))
        .collect();
    namen.sort();
    namen.into_iter().find(|n| {
        std::fs::read_link(format!("/sys/class/drm/{n}/device/driver"))
            .ok()
            .and_then(|p| p.file_name().map(|f| f.to_string_lossy() == treiber))
            .unwrap_or(false)
    }).map(|n| format!("/dev/dri/{n}"))
}

fn kms_oeffnen() -> Result<Kms, String> {
    let treiber = std::env::var("PI_DRM_TREIBER").unwrap_or_else(|_| "ili9486".into());
    let pfad = karte_finden(&treiber).ok_or_else(|| format!("keine DRM-Karte mit Treiber {treiber}"))?;
    let karte = Karte(std::fs::OpenOptions::new().read(true).write(true).open(&pfad).map_err(|e| format!("{pfad}: {e}"))?);
    let res = karte.resource_handles().map_err(|e| e.to_string())?;
    let con = res.connectors().iter().filter_map(|c| karte.get_connector(*c, true).ok())
        .find(|i| i.state() == connector::State::Connected && !i.modes().is_empty())
        .ok_or("kein verbundener Anschluss")?;
    let mode = con.modes()[0];
    let crtc = karte.get_connector(con.handle(), false).ok()
        .and_then(|i| i.current_encoder())
        .and_then(|e| karte.get_encoder(e).ok())
        .and_then(|e| e.crtc())
        .or_else(|| res.crtcs().first().copied())
        .ok_or("keine CRTC")?;
    let mut db = karte.create_dumb_buffer((W as u32, H as u32), DrmFourcc::Rgb565, 16).map_err(|e| e.to_string())?;
    let fb = karte.add_framebuffer(&db, 16, 16).map_err(|e| e.to_string())?;
    {
        let mut map = karte.map_dumb_buffer(&mut db).map_err(|e| e.to_string())?;
        map.as_mut().fill(0);
    }
    karte.set_crtc(crtc, Some(fb), (0, 0), &[con.handle()], Some(mode)).map_err(|e| format!("set_crtc: {e}"))?;
    Ok(Kms { karte, db, fb })
}

impl Ausgabe {
    /// Wählt KMS, sonst fb0; `PI_AUSGABE=png:/pfad.png` schreibt Testbilder.
    pub fn neu() -> Ausgabe {
        if let Ok(v) = std::env::var("PI_AUSGABE") {
            if let Some(p) = v.strip_prefix("png:") {
                return Ausgabe::Png { pfad: p.into(), letzte: std::time::Instant::now() - std::time::Duration::from_secs(9) };
            }
        }
        match kms_oeffnen() {
            Ok(k) => Ausgabe::Kms(k),
            Err(e) => {
                eprintln!("KMS nicht verfügbar ({e}), nehme /dev/fb0");
                let fb = std::fs::OpenOptions::new().read(true).write(true).open("/dev/fb0").expect("/dev/fb0 öffnen");
                Ausgabe::Fb0(fb)
            }
        }
    }

    /// Zeigt die Zeilen `oben..unten`; liefert die geschätzte Übertragungszeit (0 bei KMS, das wartet selbst).
    pub fn zeigen(&mut self, bild: &Img, neu: &[u16], oben: usize, unten: usize, spi_hz: f64) -> f64 {
        match self {
            Ausgabe::Kms(k) => {
                let pitch = k.db.pitch() as usize;
                if let Ok(mut map) = k.karte.map_dumb_buffer(&mut k.db) {
                    let ziel = map.as_mut();
                    for y in oben..unten {
                        for x in 0..W {
                            let v = neu[y * W + x].to_le_bytes();
                            ziel[y * pitch + x * 2] = v[0];
                            ziel[y * pitch + x * 2 + 1] = v[1];
                        }
                    }
                }
                let _ = k.karte.dirty_framebuffer(k.fb, &[ClipRect::new(0, oben as u16, W as u16, unten as u16)]);
                0.0
            }
            Ausgabe::Fb0(f) => {
                let bytes: Vec<u8> = neu[oben * W..unten * W].iter().flat_map(|v| v.to_le_bytes()).collect();
                let _ = f.seek(SeekFrom::Start((oben * W * 2) as u64));
                let _ = f.write_all(&bytes);
                (unten - oben) as f64 * W as f64 * 16.0 / spi_hz * 1.15
            }
            Ausgabe::Png { pfad, letzte } => {
                if letzte.elapsed().as_secs_f64() >= 1.0 {
                    let _ = bild.speichern(pfad);
                    *letzte = std::time::Instant::now();
                }
                0.0
            }
        }
    }
}
