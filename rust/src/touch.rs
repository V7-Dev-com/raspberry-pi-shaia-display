//! Touchscreen (ADS7846): meldet jedes Antippen, ohne zu blockieren.

use std::fs::File;
use std::io::Read;
use std::os::unix::fs::OpenOptionsExt;

/// struct input_event auf 64-Bit-Linux: timeval (2 x 8 Byte), type u16, code u16, value i32
const GROESSE: usize = 24;
const EV_KEY: u16 = 1;
const BTN_TOUCH: u16 = 0x14A;
const O_NONBLOCK: i32 = 0o4000;

pub struct Beruehrung {
    datei: Option<File>,
}

/// Name des event-Geräts zum Touchscreen aus dem Inhalt von /proc/bus/input/devices.
pub fn touch_geraet(devices: &str) -> Option<String> {
    devices.split("\n\n").find(|b| b.contains("ADS7846")).and_then(|b| {
        b.split_whitespace().find(|t| t.starts_with("event")).map(|t| t.to_string())
    })
}

impl Beruehrung {
    pub fn neu() -> Beruehrung {
        let datei = std::fs::read_to_string("/proc/bus/input/devices")
            .ok()
            .and_then(|s| touch_geraet(&s))
            .and_then(|e| {
                std::fs::OpenOptions::new().read(true).custom_flags(O_NONBLOCK).open(format!("/dev/input/{e}")).ok()
            });
        Beruehrung { datei }
    }

    /// true, wenn seit dem letzten Aufruf angetippt wurde.
    pub fn angetippt(&mut self) -> bool {
        let Some(f) = self.datei.as_mut() else { return false };
        let mut getippt = false;
        let mut puffer = [0u8; GROESSE * 16];
        loop {
            match f.read(&mut puffer) {
                Ok(0) => break,
                Ok(n) => {
                    for e in puffer[..n].chunks_exact(GROESSE) {
                        let typ = u16::from_le_bytes([e[16], e[17]]);
                        let code = u16::from_le_bytes([e[18], e[19]]);
                        let wert = i32::from_le_bytes([e[20], e[21], e[22], e[23]]);
                        if typ == EV_KEY && code == BTN_TOUCH && wert == 1 {
                            getippt = true;
                        }
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(_) => {
                    self.datei = None;
                    break;
                }
            }
        }
        getippt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn findet_das_touch_geraet() {
        let s = "I: Bus=0000 Vendor=0000\nN: Name=\"vc4-hdmi\"\nH: Handlers=event0\n\n\
                 I: Bus=0019\nN: Name=\"ADS7846 Touchscreen\"\nH: Handlers=mouse0 event1 \nB: PROP=0\n";
        assert_eq!(touch_geraet(s).as_deref(), Some("event1"));
        assert_eq!(touch_geraet("nichts"), None);
    }
}
