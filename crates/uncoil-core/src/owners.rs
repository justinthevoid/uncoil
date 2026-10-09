//! Programs that light PC parts themselves (iCUE, Armoury Crate, ...): while one runs, uncoil leaves it the
//! OpenRGB devices it claims instead of fighting it for them (no frames, and their OpenRGB detectors off),
//! and takes them back when it quits. The table is data, `owners.toml` next to this crate's `Cargo.toml`;
//! this file only reads and applies it.

use serde::Deserialize;
use std::sync::OnceLock;

/// One program and what it claims.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Owner {
    /// Its name as people know it ("Corsair iCUE").
    pub name: String,
    /// Executable names; any one running counts.
    pub processes: Vec<String>,
    /// OpenRGB device types it claims whatever their make ("dram").
    #[serde(default)]
    pub kinds: Vec<String>,
    /// Parts of a device's name or vendor it claims.
    #[serde(default)]
    pub names: Vec<String>,
    #[serde(default)]
    pub everything: bool,
}

/// A device as OpenRGB describes it: its type as a word ("motherboard", "dram", "gpu", "other"), name and
/// vendor.
#[derive(Debug, Clone, Copy)]
pub struct Part<'a> {
    pub kind: &'a str,
    pub name: &'a str,
    pub vendor: &'a str,
}

impl Owner {
    /// Is one of its executables among these process image names?
    pub fn running<S: AsRef<str>>(&self, processes: &[S]) -> bool {
        processes.iter().any(|p| self.processes.iter().any(|e| e.eq_ignore_ascii_case(p.as_ref())))
    }

    pub fn claims(&self, part: Part) -> bool {
        self.everything
            || self.kinds.iter().any(|k| k.eq_ignore_ascii_case(part.kind))
            || self.names.iter().any(|n| has(part.name, n) || has(part.vendor, n))
    }

    /// Does it claim the devices an OpenRGB detector finds, judged by the detector's name? Detector names
    /// start with the make ("Corsair DRAM", "ASUS Aura Motherboard"), and RAM and motherboard detectors say
    /// so ("Kingston Fury DDR5 DRAM"); a GPU detector names only its card, so `gpu` matches none.
    pub fn claims_detector(&self, detector: &str) -> bool {
        let word = |kind: &str| match kind.to_lowercase().as_str() {
            "dram" => Some("DRAM"),
            "motherboard" => Some("Motherboard"),
            _ => None,
        };
        self.everything
            || self.names.iter().any(|n| has(detector, n))
            || self.kinds.iter().filter_map(|k| word(k)).any(|w| has(detector, w))
    }
}

/// Does `text` contain `part`, ignoring case (a blank part never matches)?
fn has(text: &str, part: &str) -> bool {
    let part = part.trim();
    !part.is_empty() && text.to_lowercase().contains(&part.to_lowercase())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    owner: Vec<Owner>,
}

/// Read an owners table.
pub fn parse(text: &str) -> Result<Vec<Owner>, toml::de::Error> {
    toml::from_str::<File>(text).map(|f| f.owner)
}

/// The table shipped with uncoil.
pub fn shipped() -> &'static [Owner] {
    static OWNERS: OnceLock<Vec<Owner>> = OnceLock::new();
    OWNERS.get_or_init(|| parse(include_str!("../owners.toml")).expect("owners.toml parses"))
}

/// Which running program, if any, has this device: the first owner in the table that is running and
/// claims it.
pub fn holder<'a, S: AsRef<str>>(owners: &'a [Owner], processes: &[S], part: Part) -> Option<&'a Owner> {
    owners.iter().find(|o| o.claims(part) && o.running(processes))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part<'a>(kind: &'a str, name: &'a str, vendor: &'a str) -> Part<'a> {
        Part { kind, name, vendor }
    }

    #[test]
    fn the_shipped_table_reads_and_names_executables() {
        let owners = shipped();
        assert!(owners.len() >= 4);
        for o in owners {
            assert!(!o.name.is_empty() && !o.processes.is_empty(), "{o:?}");
            assert!(o.processes.iter().all(|p| p.to_lowercase().ends_with(".exe")), "{o:?}");
            assert!(o.everything || !o.kinds.is_empty() || !o.names.is_empty(), "{} claims nothing", o.name);
        }
    }

    #[test]
    fn icue_holds_corsair_devices_and_all_ram_only_while_it_runs() {
        let owners = shipped();
        let icue = ["explorer.exe", "ICUE.EXE"];
        let by = |p: Part| holder(owners, &icue, p).map(|o| o.name.as_str());
        assert_eq!(by(part("other", "Corsair iCUE Link System Hub", "Corsair")), Some("Corsair iCUE"));
        assert_eq!(by(part("dram", "Kingston Fury Beast DDR5", "Kingston")), Some("Corsair iCUE"));
        assert_eq!(by(part("motherboard", "ASUS ROG STRIX B650E-F GAMING WIFI", "ASUS")), None);
        assert_eq!(by(part("gpu", "PNY GeForce RTX 4070 Ti Super", "NVIDIA")), None);
        let nothing: [&str; 1] = ["explorer.exe"];
        assert!(holder(owners, &nothing, part("dram", "Corsair Vengeance RGB RS", "Corsair")).is_none());
    }

    #[test]
    fn armoury_crate_and_signalrgb() {
        let owners = shipped();
        let asus = ["LightingService.exe"];
        let board = part("motherboard", "ASUS ROG STRIX B650E-F GAMING WIFI", "ASUS");
        assert_eq!(holder(owners, &asus, board).unwrap().name, "ASUS Armoury Crate");
        assert!(holder(owners, &asus, part("gpu", "PNY GeForce RTX 4070 Ti Super", "")).is_none());
        let signal = ["SignalRgb.exe"];
        assert_eq!(holder(owners, &signal, part("other", "Some LED strip", "")).unwrap().name, "SignalRGB");
    }

    #[test]
    fn detectors_by_name() {
        let owners = shipped();
        let icue = owners.iter().find(|o| o.name == "Corsair iCUE").unwrap();
        // names as OpenRGB 1.0 lists its detectors
        for d in ["Corsair iCUE Link System Hub", "Corsair DRAM", "Kingston Fury DDR5 DRAM", "ENE SMBus DRAM"] {
            assert!(icue.claims_detector(d), "{d}");
        }
        for d in ["ASUS Aura Addressable", "ASUS Aura Motherboard", "PNY GeForce RTX 4070 Ti Super XLR8 VERTO OC"] {
            assert!(!icue.claims_detector(d), "{d}");
        }
        let asus = owners.iter().find(|o| o.name == "ASUS Armoury Crate").unwrap();
        assert!(asus.claims_detector("ASUS Aura SMBus Motherboard") && asus.claims_detector("HyperX DRAM"));
        assert!(!asus.claims_detector("Corsair iCUE Link System Hub"));
        let signal = owners.iter().find(|o| o.everything).unwrap();
        assert!(signal.claims_detector("Anything at all"));
    }

    #[test]
    fn blank_names_claim_nothing_and_bad_tables_are_refused() {
        let owners = parse("[[owner]]\nname = \"X\"\nprocesses = [\"x.exe\"]\nnames = [\" \"]\n").unwrap();
        assert!(!owners[0].claims(part("other", "anything", "anyone")));
        assert!(!owners[0].claims_detector("anything"));
        assert!(parse("[[owner]]\nname = \"X\"\nprocesses = [\"x.exe\"]\nnmaes = [\"typo\"]\n").is_err());
    }
}
