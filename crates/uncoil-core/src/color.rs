//! Colour maths tuned for LEDs.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    pub const BLACK: Rgb = Rgb(0, 0, 0);

    pub fn bytes(self) -> [u8; 3] {
        [self.0, self.1, self.2]
    }

    pub fn scale(self, v: f32) -> Rgb {
        let v = v.clamp(0.0, 1.0);
        let s = |c: u8| (c as f32 * v + 0.5) as u8;
        Rgb(s(self.0), s(self.1), s(self.2))
    }

    pub fn to_hex(self) -> String {
        format!("#{:02x}{:02x}{:02x}", self.0, self.1, self.2)
    }
}

/// FastLED's `hsv2rgb_rainbow` hue map, as floats.
///
/// A plain HSV wheel looks uneven on LEDs: yellow, cyan and magenta bands read wide and bright while
/// red, green and blue look thin. FastLED splits the wheel into 8 sections with hand-tuned endpoints
/// so every hue gets a visually fair share. `h` wraps to [0,1); `s`, `v` in [0,1].
pub fn rainbow(h: f32, s: f32, v: f32) -> Rgb {
    let h8 = h.rem_euclid(1.0) * 256.0;
    let section = ((h8 / 32.0) as u32) & 7;
    let off = (h8 % 32.0) * 8.0; // 0..256 within the section
    let third = off / 3.0;
    let two = off * 2.0 / 3.0;
    let (r, g, b) = match section {
        0 => (255.0 - third, third, 0.0),        // red -> orange
        1 => (171.0, 85.0 + third, 0.0),         // orange -> yellow
        2 => (171.0 - two, 170.0 + third, 0.0),  // yellow -> green
        3 => (0.0, 255.0 - third, third),        // green -> aqua
        4 => (0.0, 171.0 - two, 85.0 + two),     // aqua -> blue
        5 => (third, 0.0, 255.0 - third),        // blue -> purple
        6 => (85.0 + third, 0.0, 171.0 - third), // purple -> pink
        _ => (170.0 + third, 0.0, 85.0 - third), // pink -> red
    };
    let s = s.clamp(0.0, 1.0);
    let v = v.clamp(0.0, 1.0);
    let ch = |c: f32| {
        let c = c * s + 255.0 * (1.0 - s);
        (c * v + 0.5).clamp(0.0, 255.0) as u8
    };
    Rgb(ch(r), ch(g), ch(b))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rainbow_anchors() {
        assert_eq!(rainbow(0.0, 1.0, 1.0), Rgb(255, 0, 0));
        // section starts: orange (171,85,0), yellow-ish start of section 2 (171,170,0)
        assert_eq!(rainbow(1.0 / 8.0, 1.0, 1.0), Rgb(171, 85, 0));
        assert_eq!(rainbow(2.0 / 8.0, 1.0, 1.0), Rgb(171, 170, 0));
        assert_eq!(rainbow(4.0 / 8.0, 1.0, 1.0), Rgb(0, 171, 85));
        assert_eq!(rainbow(1.0, 1.0, 1.0), rainbow(0.0, 1.0, 1.0)); // wraps
    }

    #[test]
    fn rainbow_is_continuous() {
        // no channel jumps by more than a few steps between adjacent hues (no hard seams)
        let mut prev = rainbow(0.0, 1.0, 1.0);
        for i in 1..=4096 {
            let c = rainbow(i as f32 / 4096.0, 1.0, 1.0);
            for (a, b) in [(prev.0, c.0), (prev.1, c.1), (prev.2, c.2)] {
                assert!((a as i32 - b as i32).abs() <= 3, "seam at {i}: {prev:?} -> {c:?}");
            }
            prev = c;
        }
    }

    #[test]
    fn brightness_and_saturation() {
        assert_eq!(rainbow(0.3, 1.0, 0.0), Rgb::BLACK);
        assert_eq!(rainbow(0.3, 0.0, 1.0), Rgb(255, 255, 255));
    }
}
