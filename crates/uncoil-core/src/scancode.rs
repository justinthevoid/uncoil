//! PC scan codes (set 1, as Windows Raw Input reports them) to desk layout shape names.
//!
//! Scan codes name physical key positions, independent of the Windows keyboard layout, which is what a
//! lighting effect wants. Used only to turn a key press into a desk position for the reactive and ripple
//! effects; the daemon never keeps the code or the name.

/// Shape name for a make code, `e0` = the extended-key prefix. `None` for keys no layout names (keypad,
/// media keys) and for the fake shifts Windows wraps around some extended keys.
pub fn shape_name(make: u16, e0: bool) -> Option<&'static str> {
    let name = if e0 {
        match make {
            0x1C => "Keypad Enter",
            0x1D => "Right Control",
            0x37 => "Print Screen",
            0x38 => "Right Alt",
            0x47 => "Home",
            0x48 => "Up Arrow",
            0x49 => "Page Up",
            0x4B => "Left Arrow",
            0x4D => "Right Arrow",
            0x4F => "End",
            0x50 => "Down Arrow",
            0x51 => "Page Down",
            0x52 => "Insert",
            0x53 => "Delete",
            0x5B => "Left Windows",
            0x5C => "Right Windows",
            0x5D => "Menu",
            _ => return None,
        }
    } else {
        match make {
            0x01 => "Escape",
            0x02 => "1",
            0x03 => "2",
            0x04 => "3",
            0x05 => "4",
            0x06 => "5",
            0x07 => "6",
            0x08 => "7",
            0x09 => "8",
            0x0A => "9",
            0x0B => "0",
            0x0C => "-",
            0x0D => "=",
            0x0E => "Backspace",
            0x0F => "Tab",
            0x10 => "Q",
            0x11 => "W",
            0x12 => "E",
            0x13 => "R",
            0x14 => "T",
            0x15 => "Y",
            0x16 => "U",
            0x17 => "I",
            0x18 => "O",
            0x19 => "P",
            0x1A => "[",
            0x1B => "]",
            0x1C => "Enter",
            0x1D => "Left Control",
            0x1E => "A",
            0x1F => "S",
            0x20 => "D",
            0x21 => "F",
            0x22 => "G",
            0x23 => "H",
            0x24 => "J",
            0x25 => "K",
            0x26 => "L",
            0x27 => ";",
            0x28 => "'",
            0x29 => "`",
            0x2A => "Left Shift",
            0x2B => "\\",
            0x2C => "Z",
            0x2D => "X",
            0x2E => "C",
            0x2F => "V",
            0x30 => "B",
            0x31 => "N",
            0x32 => "M",
            0x33 => ",",
            0x34 => ".",
            0x35 => "/",
            0x36 => "Right Shift",
            0x38 => "Left Alt",
            0x39 => "Space",
            0x3A => "Caps Lock",
            0x3B => "F1",
            0x3C => "F2",
            0x3D => "F3",
            0x3E => "F4",
            0x3F => "F5",
            0x40 => "F6",
            0x41 => "F7",
            0x42 => "F8",
            0x43 => "F9",
            0x44 => "F10",
            0x45 => "Num Lock",
            0x46 => "Scroll Lock",
            0x57 => "F11",
            0x58 => "F12",
            _ => return None,
        }
    };
    Some(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::device::builtin;

    #[test]
    fn common_keys() {
        assert_eq!(shape_name(0x11, false), Some("W"));
        assert_eq!(shape_name(0x2A, false), Some("Left Shift"));
        assert_eq!(shape_name(0x2B, false), Some("\\"));
        assert_eq!(shape_name(0x48, true), Some("Up Arrow"));
        assert_eq!(shape_name(0x2A, true), None, "fake shift around extended keys");
        assert_eq!(shape_name(0x48, false), None, "keypad 8");
    }

    #[test]
    fn every_blackwidow_key_but_fn_has_a_scan_code() {
        let defs = builtin();
        let kb = defs.iter().find(|d| d.id == "razer-blackwidow-v4-pro-75").unwrap();
        let mapped: Vec<&str> =
            (0u16..0x80).flat_map(|m| [shape_name(m, false), shape_name(m, true)]).flatten().collect();
        let keys = kb
            .matrix
            .as_ref()
            .unwrap()
            .names
            .iter()
            .flatten()
            .filter(|n| !n.is_empty() && !n.starts_with("LU") && !n.starts_with("RU"));
        let missing: Vec<&String> = keys.filter(|n| !mapped.contains(&n.as_str())).collect();
        assert_eq!(missing, ["Right Fn"], "only Fn (handled in firmware) has no scan code");
    }
}
