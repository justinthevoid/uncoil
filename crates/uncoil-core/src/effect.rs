//! Effects are pure functions of (desk position, time). Every device samples the same field, so a wave
//! flows continuously from the keyboard onto the mouse and mat.
//!
//! A few effects also read [`Inputs`]: recent key presses (reactive, ripple), the system audio level (audio
//! meter) and the desk's extent (fire, audio meter, wheel). Inputs carry positions, times and a level only;
//! nothing here knows which key was pressed.
//!
//! Every effect yields a colour and an alpha. A plain effect is composited over black; a `studio` stacks
//! layers bottom (index 0) to top, each blended over the result where its [`Mask`] covers the LED:
//! `out = mix(out, layer_rgb, layer_alpha * opacity)`.

use crate::color::{rainbow, Rgb};
use serde::{Deserialize, Serialize};
use std::f32::consts::{PI, TAU};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Effect {
    /// Rainbow bands travelling across the desk at an angle.
    Wave {
        /// 0 = sweeps left->right, 90 = back->front.
        #[serde(default = "d_angle")]
        angle_deg: f32,
        /// Seconds for one full colour cycle to pass a point (higher = slower).
        #[serde(default = "d_period")]
        period_s: f32,
        /// Width of one full rainbow, in key units (1u = 19.05 mm).
        #[serde(default = "d_wavelength")]
        wavelength: f32,
        #[serde(default)]
        reverse: bool,
    },
    /// Whole desk cycles through the rainbow in unison.
    Spectrum {
        #[serde(default = "d_period")]
        period_s: f32,
    },
    Static {
        color: Rgb,
    },
    Off,
    /// Fades in and out. No colours = a new rainbow hue each breath; one = that colour; more = take turns.
    Breathing {
        #[serde(default)]
        colors: Vec<Rgb>,
        #[serde(default = "d_breath")]
        period_s: f32,
    },
    /// Random LEDs twinkle. No colours = random hues. `density` = share of LEDs lit at once (0..1).
    Starlight {
        #[serde(default)]
        colors: Vec<Rgb>,
        #[serde(default = "d_density")]
        density: f32,
        #[serde(default = "d_twinkle")]
        twinkle_s: f32,
    },
    /// Flames rising from the front edge of the desk. `height` 0..1 of the desk depth; `speed` 0.25..3.
    Fire {
        #[serde(default = "one")]
        speed: f32,
        #[serde(default = "d_fire_height")]
        height: f32,
    },
    /// A rainbow turning around a centre point (desk units; `None` = the keyboard's centre).
    Wheel {
        #[serde(default = "d_wheel")]
        period_s: f32,
        #[serde(default)]
        reverse: bool,
        #[serde(default)]
        center: Option<[f32; 2]>,
    },
    /// Keys light up when pressed and fade. `color` `None` = a new rainbow hue per press.
    Reactive {
        #[serde(default)]
        color: Option<Rgb>,
        #[serde(default = "d_fade")]
        fade_s: f32,
    },
    /// A ring spreads across the desk from each pressed key.
    Ripple {
        #[serde(default)]
        color: Option<Rgb>,
        /// Key units per second.
        #[serde(default = "d_ripple_speed")]
        speed: f32,
        /// Ring width in key units.
        #[serde(default = "d_ripple_width")]
        width: f32,
        #[serde(default = "d_fade")]
        fade_s: f32,
    },
    /// The desk fills left to right with the system audio peak level, green to yellow to red.
    AudioMeter {
        #[serde(default = "one")]
        sensitivity: f32,
    },
    /// Layers, bottom first. A studio inside a studio is ignored.
    Studio {
        #[serde(default)]
        layers: Vec<StudioLayer>,
    },
}

/// One layer of a [`Effect::Studio`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StudioLayer {
    #[serde(default)]
    pub name: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// 0..1. Effects with their own transparency (reactive, ripple, starlight, audio meter) multiply this.
    #[serde(default = "one")]
    pub opacity: f32,
    pub effect: Effect,
    #[serde(default)]
    pub mask: Mask,
}

/// Which LEDs a studio layer covers. Shape names are the desk layout's (`"W"`, `"Left Shift"`, `"Logo"`,
/// `"Edge"`, ...).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Mask {
    #[default]
    All,
    Devices {
        ids: Vec<String>,
    },
    Keys {
        device: String,
        shapes: Vec<String>,
    },
}

impl Mask {
    fn covers(&self, device: &str, shape: &str) -> bool {
        match self {
            Mask::All => true,
            Mask::Devices { ids } => ids.iter().any(|i| i == device),
            Mask::Keys { device: d, shapes } => d == device && shapes.iter().any(|s| s == shape),
        }
    }
}

fn d_angle() -> f32 {
    35.0
}
fn d_period() -> f32 {
    14.0
}
fn d_wavelength() -> f32 {
    26.0
}
fn d_breath() -> f32 {
    4.0
}
fn d_density() -> f32 {
    0.15
}
fn d_twinkle() -> f32 {
    1.5
}
fn d_fire_height() -> f32 {
    0.5
}
fn d_wheel() -> f32 {
    6.0
}
fn d_fade() -> f32 {
    1.0
}
fn d_ripple_speed() -> f32 {
    12.0
}
fn d_ripple_width() -> f32 {
    1.5
}
fn one() -> f32 {
    1.0
}
fn yes() -> bool {
    true
}

impl Default for Effect {
    fn default() -> Self {
        Effect::Wave { angle_deg: d_angle(), period_s: d_period(), wavelength: d_wavelength(), reverse: false }
    }
}

/// A key press, as the effects see it: where on the desk (key units) and when (seconds, same clock as `t`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Press {
    pub x: f32,
    pub y: f32,
    pub t: f32,
}

/// The desk's extent in key units (y grows toward the user, so `max_y` is the front edge).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min_x: f32,
    pub min_y: f32,
    pub max_x: f32,
    pub max_y: f32,
}

impl Bounds {
    /// The default desk (keyboard, mouse and extended mat) when the caller has no layout.
    pub const DEFAULT: Bounds = Bounds { min_x: -2.0, min_y: -1.5, max_x: 24.25, max_y: 8.25 };
}

/// Where the wheel turns when it has no centre and the caller knows no keyboard.
pub const DEFAULT_WHEEL_CENTER: (f32, f32) = (8.1, 3.1);
/// How far from a pressed key's centre the reactive effect lights LEDs, in key units.
pub const REACTIVE_RADIUS: f32 = 0.6;

/// What the world looks like this frame, beyond time.
#[derive(Debug, Clone, Copy, Default)]
pub struct Inputs<'a> {
    /// Recent key presses (older ones are ignored by each effect's own fade time).
    pub presses: &'a [Press],
    /// System audio peak level, 0..1.
    pub audio: f32,
    /// Desk extent; `None` = [`Bounds::DEFAULT`].
    pub bounds: Option<Bounds>,
    /// Centre of the keyboard on the desk; `None` = [`DEFAULT_WHEEL_CENTER`].
    pub keyboard_center: Option<(f32, f32)>,
}

impl Effect {
    /// True if this effect (or any enabled studio layer) needs key presses.
    pub fn uses_keys(&self) -> bool {
        self.any_live(|e| matches!(e, Effect::Reactive { .. } | Effect::Ripple { .. }))
    }

    /// True if this effect (or any enabled studio layer) needs the audio level.
    pub fn uses_audio(&self) -> bool {
        self.any_live(|e| matches!(e, Effect::AudioMeter { .. }))
    }

    fn any_live(&self, f: impl Fn(&Effect) -> bool) -> bool {
        match self {
            Effect::Studio { layers } => layers.iter().any(|l| l.enabled && l.opacity > 0.0 && f(&l.effect)),
            e => f(e),
        }
    }

    /// Prepare the effect for time `t` (seconds) at brightness `val` and saturation `sat`, with no inputs.
    pub fn at(&self, t: f32, sat: f32, val: f32) -> Frame<'_> {
        self.at_with(t, sat, val, &Inputs::default())
    }

    /// Prepare the effect for time `t` with key presses, audio level and desk geometry.
    pub fn at_with(&self, t: f32, sat: f32, val: f32, inputs: &Inputs) -> Frame<'_> {
        let layers = match self {
            Effect::Studio { layers } => layers
                .iter()
                .filter(|l| l.enabled && l.opacity > 0.0 && !matches!(l.effect, Effect::Studio { .. }))
                .map(|l| Layer {
                    kind: prepare(&l.effect, t, sat, val, inputs),
                    opacity: l.opacity.min(1.0),
                    mask: Some(&l.mask),
                })
                .collect(),
            e => vec![Layer { kind: prepare(e, t, sat, val, inputs), opacity: 1.0, mask: None }],
        };
        Frame { layers, sat, val }
    }
}

/// An effect frozen at one instant, cheap to evaluate per LED.
pub struct Frame<'a> {
    layers: Vec<Layer<'a>>,
    sat: f32,
    val: f32,
}

struct Layer<'a> {
    kind: Kind<'a>,
    opacity: f32,
    /// `None` = the whole desk (a plain, non-studio effect).
    mask: Option<&'a Mask>,
}

/// A press that is still lighting something: position, strength left (1 -> 0) and colour.
struct Spot {
    x: f32,
    y: f32,
    /// Ripple ring radius (unused by reactive).
    r: f32,
    strength: f32,
    color: Rgb,
}

enum Kind<'a> {
    Wave { ux: f32, uy: f32, inv_wl: f32, phase: f32 },
    Solid(Rgb),
    Starlight { colors: &'a [Rgb], density: f32, twinkle_s: f32, t: f32 },
    Fire { front: f32, inv_h: f32, tt: f32 },
    Wheel { cx: f32, cy: f32, phase: f32 },
    Reactive { spots: Vec<Spot> },
    Ripple { spots: Vec<Spot>, half_w: f32 },
    Meter { min_x: f32, inv_w: f32, fill: f32 },
}

fn prepare<'a>(e: &'a Effect, t: f32, sat: f32, val: f32, inp: &Inputs) -> Kind<'a> {
    let bounds = inp.bounds.unwrap_or(Bounds::DEFAULT);
    match e {
        &Effect::Wave { angle_deg, period_s, wavelength, reverse } => {
            let a = angle_deg.to_radians();
            let dir = if reverse { -1.0 } else { 1.0 };
            Kind::Wave {
                ux: a.cos(),
                uy: a.sin(),
                inv_wl: 1.0 / wavelength.max(1.0),
                phase: dir * t / period_s.max(0.5),
            }
        }
        &Effect::Spectrum { period_s } => Kind::Solid(rainbow(t / period_s.max(0.5), sat, val)),
        &Effect::Static { color } => Kind::Solid(color.scale(val)),
        Effect::Off | Effect::Studio { .. } => Kind::Solid(Rgb::BLACK),
        Effect::Breathing { colors, period_s } => {
            let ph = t / period_s.max(0.5);
            let n = ph.floor();
            let b = 0.5 - 0.5 * (TAU * (ph - n)).cos();
            let v = val * b * b;
            Kind::Solid(match colors.len() {
                0 => rainbow(n / 6.0, sat, v),
                len => colors[(n as i64).rem_euclid(len as i64) as usize].scale(v),
            })
        }
        Effect::Starlight { colors, density, twinkle_s } => {
            Kind::Starlight { colors, density: density.clamp(0.0, 1.0), twinkle_s: twinkle_s.max(0.1), t }
        }
        &Effect::Fire { speed, height } => {
            let depth = (bounds.max_y - bounds.min_y).max(1.0);
            let flame = (height.clamp(0.05, 1.0) * depth).max(0.5);
            Kind::Fire { front: bounds.max_y, inv_h: 1.0 / flame, tt: t * speed.clamp(0.25, 3.0) }
        }
        &Effect::Wheel { period_s, reverse, center } => {
            let (cx, cy) = center.map(|[x, y]| (x, y)).or(inp.keyboard_center).unwrap_or(DEFAULT_WHEEL_CENTER);
            let dir = if reverse { -1.0 } else { 1.0 };
            Kind::Wheel { cx, cy, phase: dir * t / period_s.max(0.5) }
        }
        &Effect::Reactive { color, fade_s } => {
            Kind::Reactive { spots: spots(inp.presses, t, fade_s, 0.0, color, sat, val) }
        }
        &Effect::Ripple { color, speed, width, fade_s } => Kind::Ripple {
            spots: spots(inp.presses, t, fade_s, speed.max(0.0), color, sat, val),
            half_w: (width * 0.5).max(0.1),
        },
        &Effect::AudioMeter { sensitivity } => {
            let level = (inp.audio * sensitivity.max(0.0)).clamp(0.0, 1.0);
            let w = (bounds.max_x - bounds.min_x).max(1.0);
            let fill = if level > 0.0 { bounds.min_x + level * w } else { f32::NEG_INFINITY };
            Kind::Meter { min_x: bounds.min_x, inv_w: 1.0 / w, fill }
        }
    }
}

/// Presses still fading at `t`, with their strength, ring radius and colour.
fn spots(presses: &[Press], t: f32, fade_s: f32, speed: f32, color: Option<Rgb>, sat: f32, val: f32) -> Vec<Spot> {
    let fade = fade_s.max(0.05);
    presses
        .iter()
        .filter_map(|p| {
            let age = t - p.t;
            if !(0.0..fade).contains(&age) {
                return None;
            }
            let color = match color {
                Some(c) => c.scale(val),
                None => rainbow(press_hue(p.t), sat, val),
            };
            Some(Spot { x: p.x, y: p.y, r: age * speed, strength: 1.0 - age / fade, color })
        })
        .collect()
}

/// A stable "random" hue for a press, from its time stamp.
fn press_hue(t: f32) -> f32 {
    unit(mix(t.to_bits()))
}

impl Kind<'_> {
    /// Colour and alpha (0..1) at a desk point.
    fn eval(&self, x: f32, y: f32, sat: f32, val: f32) -> (Rgb, f32) {
        match self {
            &Kind::Wave { ux, uy, inv_wl, phase } => (rainbow((x * ux + y * uy) * inv_wl - phase, sat, val), 1.0),
            &Kind::Solid(c) => (c, 1.0),
            &Kind::Starlight { colors, density, twinkle_s, t } => {
                let (qx, qy) = (quant(x), quant(y));
                let led = hash3(qx, qy, 0x5eed);
                // each LED's buckets start at its own offset, so twinkles don't all turn over together
                let tt = t / twinkle_s + unit(led);
                let bucket = tt.floor();
                let h = hash3(qx, qy, bucket as i32);
                if unit(h) >= density {
                    return (Rgb::BLACK, 0.0);
                }
                let pick = mix(h ^ 0x9e37_79b9);
                let c = match colors.len() {
                    0 => rainbow(unit(pick), sat, val),
                    len => colors[(pick % len as u32) as usize].scale(val),
                };
                (c, (PI * (tt - bucket)).sin().max(0.0))
            }
            &Kind::Fire { front, inv_h, tt } => {
                let d = ((front - y) * inv_h).max(0.0);
                let rise = y + tt * 2.2;
                let n =
                    0.65 * vnoise(x * 0.6, rise * 0.6, tt * 0.5) + 0.35 * vnoise(x * 1.3, rise * 1.3, tt * 0.9 + 17.0);
                let heat = (1.0 - d + (n - 0.5) * 1.2).clamp(0.0, 1.0);
                (fire_palette(heat).scale(val), 1.0)
            }
            &Kind::Wheel { cx, cy, phase } => (rainbow((y - cy).atan2(x - cx) / TAU - phase, sat, val), 1.0),
            Kind::Reactive { spots } => {
                let r2 = REACTIVE_RADIUS * REACTIVE_RADIUS;
                strongest(spots, |s| {
                    let d2 = (x - s.x) * (x - s.x) + (y - s.y) * (y - s.y);
                    if d2 >= r2 {
                        0.0
                    } else {
                        s.strength * (1.0 - d2 / r2)
                    }
                })
            }
            Kind::Ripple { spots, half_w } => strongest(spots, |s| {
                let d = ((x - s.x) * (x - s.x) + (y - s.y) * (y - s.y)).sqrt();
                let ring = 1.0 - (d - s.r).abs() / half_w;
                if ring <= 0.0 {
                    0.0
                } else {
                    s.strength * ring
                }
            }),
            &Kind::Meter { min_x, inv_w, fill } => {
                if x > fill {
                    return (Rgb::BLACK, 0.0);
                }
                (meter_color(((x - min_x) * inv_w).clamp(0.0, 1.0)).scale(val), 1.0)
            }
        }
    }
}

/// The colour and intensity of whichever spot lights this point most.
fn strongest(spots: &[Spot], f: impl Fn(&Spot) -> f32) -> (Rgb, f32) {
    let mut best = (Rgb::BLACK, 0.0);
    for s in spots {
        let a = f(s);
        if a > best.1 {
            best = (s.color, a);
        }
    }
    best
}

impl Frame<'_> {
    /// Colour at a desk point, for callers that don't know which LED it is: only layers covering the whole
    /// desk apply (device and key masks never match).
    pub fn color_at(&self, x: f32, y: f32) -> Rgb {
        self.color_led("", "", x, y)
    }

    /// Colour of one LED: `device` is the device id, `shape` its desk layout shape name, (x, y) its desk
    /// position. Studio layers whose mask doesn't cover this LED are skipped.
    pub fn color_led(&self, device: &str, shape: &str, x: f32, y: f32) -> Rgb {
        if let [l] = &self.layers[..] {
            if l.mask.is_none() {
                let (c, a) = l.kind.eval(x, y, self.sat, self.val);
                return if a >= 1.0 { c } else { c.scale(a) };
            }
        }
        let mut out = [0.0f32; 3];
        for l in &self.layers {
            if l.mask.is_some_and(|m| !m.covers(device, shape)) {
                continue;
            }
            let (c, a) = l.kind.eval(x, y, self.sat, self.val);
            let a = a * l.opacity;
            if a <= 0.0 {
                continue;
            }
            for (o, v) in out.iter_mut().zip(c.bytes()) {
                *o += (v as f32 - *o) * a.min(1.0);
            }
        }
        let ch = |v: f32| (v + 0.5).clamp(0.0, 255.0) as u8;
        Rgb(ch(out[0]), ch(out[1]), ch(out[2]))
    }
}

// ---- deterministic noise (ported 1:1 to apps/uncoil/src/lib/effect.ts) ----

/// 32-bit integer finaliser (lowbias32).
fn mix(mut h: u32) -> u32 {
    h ^= h >> 16;
    h = h.wrapping_mul(0x7feb_352d);
    h ^= h >> 15;
    h = h.wrapping_mul(0x846c_a68b);
    h ^ (h >> 16)
}

fn hash3(a: i32, b: i32, c: i32) -> u32 {
    mix((a as u32).wrapping_mul(0x9e37_79b1) ^ mix((b as u32).wrapping_mul(0x85eb_ca77) ^ mix(c as u32)))
}

/// 0..1 from the top 24 bits (exact in f32 and f64).
fn unit(h: u32) -> f32 {
    (h >> 8) as f32 / 16_777_216.0
}

/// A desk coordinate snapped to 1/64 key, so every LED gets a stable identity from its position.
fn quant(v: f32) -> i32 {
    (v * 64.0 + 0.5).floor() as i32
}

/// Smooth 3-D value noise in 0..1.
fn vnoise(x: f32, y: f32, z: f32) -> f32 {
    let (xf, yf, zf) = (x.floor(), y.floor(), z.floor());
    let (i, j, k) = (xf as i32, yf as i32, zf as i32);
    let s = |f: f32| f * f * (3.0 - 2.0 * f);
    let (u, v, w) = (s(x - xf), s(y - yf), s(z - zf));
    let c = |di: i32, dj: i32, dk: i32| unit(hash3(i + di, j + dj, k + dk));
    let lerp = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let x00 = lerp(c(0, 0, 0), c(1, 0, 0), u);
    let x10 = lerp(c(0, 1, 0), c(1, 1, 0), u);
    let x01 = lerp(c(0, 0, 1), c(1, 0, 1), u);
    let x11 = lerp(c(0, 1, 1), c(1, 1, 1), u);
    lerp(lerp(x00, x10, v), lerp(x01, x11, v), w)
}

/// Black -> dark red -> orange -> yellow.
fn fire_palette(heat: f32) -> Rgb {
    const STOPS: [(f32, [f32; 3]); 5] = [
        (0.0, [0.0, 0.0, 0.0]),
        (0.3, [110.0, 0.0, 0.0]),
        (0.55, [210.0, 40.0, 0.0]),
        (0.8, [255.0, 130.0, 0.0]),
        (1.0, [255.0, 210.0, 60.0]),
    ];
    let mut i = 1;
    while i < STOPS.len() - 1 && heat > STOPS[i].0 {
        i += 1;
    }
    let (t0, a) = STOPS[i - 1];
    let (t1, b) = STOPS[i];
    let f = ((heat - t0) / (t1 - t0)).clamp(0.0, 1.0);
    let ch = |k: usize| (a[k] + (b[k] - a[k]) * f + 0.5) as u8;
    Rgb(ch(0), ch(1), ch(2))
}

/// Green at the left, yellow at 60 %, red at the right.
fn meter_color(u: f32) -> Rgb {
    if u < 0.6 {
        Rgb((255.0 * u / 0.6 + 0.5) as u8, 255, 0)
    } else {
        Rgb(255, (255.0 * (1.0 - (u - 0.6) / 0.4) + 0.5) as u8, 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RED: Rgb = Rgb(255, 0, 0);
    const BLUE: Rgb = Rgb(0, 0, 255);

    fn lum(c: Rgb) -> u32 {
        c.0 as u32 + c.1 as u32 + c.2 as u32
    }

    fn layer(effect: Effect, mask: Mask) -> StudioLayer {
        StudioLayer { name: String::new(), enabled: true, opacity: 1.0, effect, mask }
    }

    #[test]
    fn wave_moves_over_time() {
        let e = Effect::default();
        assert_ne!(e.at(0.0, 1.0, 1.0).color_at(3.0, 2.0), e.at(2.0, 1.0, 1.0).color_at(3.0, 2.0));
    }

    #[test]
    fn wave_repeats_each_period() {
        let e = Effect::Wave { angle_deg: 35.0, period_s: 10.0, wavelength: 20.0, reverse: false };
        assert_eq!(e.at(1.0, 1.0, 1.0).color_at(5.0, 1.0), e.at(11.0, 1.0, 1.0).color_at(5.0, 1.0));
    }

    #[test]
    fn config_roundtrip() {
        let e: Effect = serde_json::from_str(r#"{"kind":"wave","angle_deg":20}"#).unwrap();
        assert!(matches!(e, Effect::Wave { angle_deg, period_s, .. } if angle_deg == 20.0 && period_s == 14.0));
    }

    #[test]
    fn color_led_matches_color_at_for_plain_effects() {
        let e = Effect::default();
        let f = e.at(3.0, 1.0, 0.8);
        assert_eq!(f.color_led("kb", "W", 2.0, 2.5), f.color_at(2.0, 2.5));
    }

    // ---- breathing ----

    #[test]
    fn breathing_fades_out_and_in_once_per_period() {
        let e = Effect::Breathing { colors: vec![RED], period_s: 4.0 };
        assert_eq!(e.at(0.0, 1.0, 1.0).color_at(0.0, 0.0), Rgb::BLACK);
        assert_eq!(e.at(2.0, 1.0, 1.0).color_at(0.0, 0.0), RED);
        assert_eq!(e.at(1.0, 1.0, 1.0).color_at(0.0, 0.0), e.at(5.0, 1.0, 1.0).color_at(0.0, 0.0));
        let a = lum(e.at(1.0, 1.0, 1.0).color_at(0.0, 0.0));
        assert!(a > 0 && a < 255, "half way up should be dimmer than the peak: {a}");
    }

    #[test]
    fn breathing_two_colours_alternate_and_none_cycles_hues() {
        let e = Effect::Breathing { colors: vec![RED, BLUE], period_s: 2.0 };
        assert_eq!(e.at(1.0, 1.0, 1.0).color_at(0.0, 0.0), RED);
        assert_eq!(e.at(3.0, 1.0, 1.0).color_at(0.0, 0.0), BLUE);
        assert_eq!(e.at(5.0, 1.0, 1.0).color_at(0.0, 0.0), RED);
        let r = Effect::Breathing { colors: vec![], period_s: 2.0 };
        assert_ne!(r.at(1.0, 1.0, 1.0).color_at(0.0, 0.0), r.at(3.0, 1.0, 1.0).color_at(0.0, 0.0));
    }

    // ---- starlight ----

    fn grid() -> impl Iterator<Item = (f32, f32)> {
        (0..17).flat_map(|x| (0..6).map(move |y| (x as f32 + 0.5, y as f32 + 0.5)))
    }

    #[test]
    fn starlight_is_deterministic_and_changes_over_time() {
        let e = Effect::Starlight { colors: vec![], density: 0.4, twinkle_s: 1.0 };
        let a: Vec<Rgb> = grid().map(|(x, y)| e.at(7.3, 1.0, 1.0).color_at(x, y)).collect();
        let b: Vec<Rgb> = grid().map(|(x, y)| e.at(7.3, 1.0, 1.0).color_at(x, y)).collect();
        let c: Vec<Rgb> = grid().map(|(x, y)| e.at(9.8, 1.0, 1.0).color_at(x, y)).collect();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn starlight_density_sets_the_lit_share() {
        let lit = |density: f32| {
            let e = Effect::Starlight { colors: vec![RED], density, twinkle_s: 1.0 };
            let mut n = 0;
            let mut total = 0;
            for step in 0..40 {
                let f = e.at(step as f32 * 0.37, 1.0, 1.0);
                for (x, y) in grid() {
                    total += 1;
                    n += (f.color_at(x, y) != Rgb::BLACK) as u32;
                }
            }
            n as f32 / total as f32
        };
        assert_eq!(lit(0.0), 0.0);
        let (lo, hi) = (lit(0.1), lit(0.6));
        assert!((0.04..0.18).contains(&lo), "density 0.1 lit {lo}");
        assert!((0.45..0.75).contains(&hi), "density 0.6 lit {hi}");
    }

    #[test]
    fn starlight_twinkles_fade_smoothly() {
        // sample one LED finely: no jump bigger than a smooth fade allows
        let e = Effect::Starlight { colors: vec![Rgb(255, 255, 255)], density: 1.0, twinkle_s: 1.0 };
        let mut prev = e.at(0.0, 1.0, 1.0).color_at(3.5, 2.5);
        for i in 1..400 {
            let c = e.at(i as f32 * 0.01, 1.0, 1.0).color_at(3.5, 2.5);
            assert!((c.0 as i32 - prev.0 as i32).abs() <= 10, "jump at {i}: {prev:?} -> {c:?}");
            prev = c;
        }
    }

    #[test]
    fn starlight_alpha_is_its_intensity() {
        let e = Effect::Studio {
            layers: vec![
                layer(Effect::Static { color: BLUE }, Mask::All),
                layer(Effect::Starlight { colors: vec![RED], density: 0.5, twinkle_s: 1.0 }, Mask::All),
            ],
        };
        let f = e.at(4.2, 1.0, 1.0);
        // unlit stars show the blue layer beneath, lit ones blend red over it
        let colours: Vec<Rgb> = grid().map(|(x, y)| f.color_at(x, y)).collect();
        assert!(colours.contains(&BLUE));
        assert!(colours.iter().any(|c| c.0 > 0));
        assert!(colours.iter().all(|c| c.1 == 0 && c.0 as u32 + c.2 as u32 >= 254));
    }

    // ---- fire ----

    #[test]
    fn fire_burns_at_the_front_and_is_dark_at_the_back() {
        let e = Effect::Fire { speed: 1.0, height: 0.4 };
        let b = Bounds { min_x: 0.0, min_y: 0.0, max_x: 20.0, max_y: 10.0 };
        let inp = Inputs { bounds: Some(b), ..Default::default() };
        let mut front = 0;
        let mut back = 0;
        for step in 0..20 {
            let f = e.at_with(step as f32 * 0.21, 1.0, 1.0, &inp);
            for x in 0..20 {
                front += lum(f.color_at(x as f32, 9.9));
                back += lum(f.color_at(x as f32, 0.2));
            }
        }
        assert_eq!(back, 0, "the back of the desk stays dark");
        assert!(front > 20 * 20 * 200, "the front edge burns bright: {front}");
    }

    #[test]
    fn fire_is_deterministic_moves_and_uses_the_palette() {
        let e = Effect::Fire { speed: 1.0, height: 0.8 };
        let at = |t: f32| (0..20).map(|x| e.at(t, 1.0, 1.0).color_at(x as f32 * 0.8, 6.0)).collect::<Vec<_>>();
        assert_eq!(at(3.0), at(3.0));
        assert_ne!(at(3.0), at(3.5));
        for c in at(3.0) {
            assert!(c.0 >= c.1 && c.1 >= c.2, "fire colours run red >= green >= blue: {c:?}");
        }
    }

    // ---- wheel ----

    #[test]
    fn wheel_turns_around_its_centre() {
        let e = Effect::Wheel { period_s: 4.0, reverse: false, center: Some([5.0, 5.0]) };
        let f = e.at(0.0, 1.0, 1.0);
        // same angle, different radius: same colour; opposite sides: different colours
        assert_eq!(f.color_at(6.0, 5.0), f.color_at(9.0, 5.0));
        assert_ne!(f.color_at(6.0, 5.0), f.color_at(4.0, 5.0));
        assert_eq!(e.at(1.0, 1.0, 1.0).color_at(7.0, 3.0), e.at(5.0, 1.0, 1.0).color_at(7.0, 3.0));
        assert_ne!(e.at(1.0, 1.0, 1.0).color_at(7.0, 3.0), e.at(2.0, 1.0, 1.0).color_at(7.0, 3.0));
    }

    #[test]
    fn wheel_centre_defaults_to_the_keyboard() {
        let e = Effect::Wheel { period_s: 4.0, reverse: false, center: None };
        let pinned = Effect::Wheel { period_s: 4.0, reverse: false, center: Some([8.1, 3.1]) };
        assert_eq!(e.at(1.0, 1.0, 1.0).color_at(2.0, 1.0), pinned.at(1.0, 1.0, 1.0).color_at(2.0, 1.0));
        let inp = Inputs { keyboard_center: Some((20.0, 3.0)), ..Default::default() };
        let moved = Effect::Wheel { period_s: 4.0, reverse: false, center: Some([20.0, 3.0]) };
        assert_eq!(e.at_with(1.0, 1.0, 1.0, &inp).color_at(2.0, 1.0), moved.at(1.0, 1.0, 1.0).color_at(2.0, 1.0));
    }

    // ---- reactive / ripple ----

    #[test]
    fn reactive_lights_the_pressed_key_and_fades() {
        let e = Effect::Reactive { color: Some(RED), fade_s: 1.0 };
        let presses = [Press { x: 3.0, y: 2.0, t: 10.0 }];
        let inp = Inputs { presses: &presses, ..Default::default() };
        assert_eq!(e.at_with(10.0, 1.0, 1.0, &inp).color_at(3.0, 2.0), RED);
        assert_eq!(e.at_with(10.0, 1.0, 1.0, &inp).color_at(4.0, 2.0), Rgb::BLACK, "the neighbour stays dark");
        let half = e.at_with(10.5, 1.0, 1.0, &inp).color_at(3.0, 2.0);
        assert!(half.0 > 100 && half.0 < 155, "half faded: {half:?}");
        assert_eq!(e.at_with(11.0, 1.0, 1.0, &inp).color_at(3.0, 2.0), Rgb::BLACK);
        assert_eq!(e.at_with(9.9, 1.0, 1.0, &inp).color_at(3.0, 2.0), Rgb::BLACK, "future presses are ignored");
        assert_eq!(e.at(10.0, 1.0, 1.0).color_at(3.0, 2.0), Rgb::BLACK, "no presses, nothing lit");
    }

    #[test]
    fn reactive_rainbow_hue_is_stable_per_press_and_differs_between_presses() {
        let e = Effect::Reactive { color: None, fade_s: 2.0 };
        let presses = [Press { x: 1.0, y: 1.0, t: 4.0 }, Press { x: 5.0, y: 1.0, t: 4.7 }];
        let inp = Inputs { presses: &presses, ..Default::default() };
        let f1 = e.at_with(4.8, 1.0, 1.0, &inp);
        let f2 = e.at_with(4.8, 1.0, 1.0, &inp);
        assert_eq!(f1.color_at(1.0, 1.0), f2.color_at(1.0, 1.0));
        let (a, b) = (f1.color_at(1.0, 1.0), f1.color_at(5.0, 1.0));
        assert!(lum(a) > 0 && lum(b) > 0);
        assert_ne!(a, b, "two presses should get different hues");
    }

    #[test]
    fn ripple_ring_expands_from_the_press() {
        let e = Effect::Ripple { color: Some(BLUE), speed: 10.0, width: 1.0, fade_s: 2.0 };
        let presses = [Press { x: 0.0, y: 0.0, t: 1.0 }];
        let inp = Inputs { presses: &presses, ..Default::default() };
        let f = e.at_with(1.5, 1.0, 1.0, &inp); // radius 5
        assert!(f.color_at(5.0, 0.0).2 > 150, "on the ring");
        assert_eq!(f.color_at(0.0, 0.0), Rgb::BLACK, "inside the ring");
        assert_eq!(f.color_at(9.0, 0.0), Rgb::BLACK, "outside the ring");
        let later = e.at_with(2.0, 1.0, 1.0, &inp); // radius 10, half faded
        assert!(later.color_at(0.0, 10.0).2 > 0 && later.color_at(0.0, 10.0).2 < f.color_at(5.0, 0.0).2);
        assert_eq!(e.at_with(3.1, 1.0, 1.0, &inp).color_at(0.0, 21.0), Rgb::BLACK, "gone after fade_s");
    }

    // ---- audio meter ----

    #[test]
    fn audio_meter_fills_left_to_right() {
        let e = Effect::AudioMeter { sensitivity: 1.0 };
        let b = Bounds { min_x: 0.0, min_y: 0.0, max_x: 10.0, max_y: 5.0 };
        let at = |audio: f32, x: f32| {
            e.at_with(0.0, 1.0, 1.0, &Inputs { audio, bounds: Some(b), ..Default::default() }).color_at(x, 1.0)
        };
        assert_eq!(at(0.0, 0.0), Rgb::BLACK);
        assert_eq!(at(0.5, 1.0), Rgb(43, 255, 0));
        assert_eq!(at(0.5, 6.0), Rgb::BLACK);
        assert_eq!(at(1.0, 10.0), Rgb(255, 0, 0));
        assert_eq!(at(1.0, 6.0), Rgb(255, 255, 0));
        let loud = Effect::AudioMeter { sensitivity: 4.0 };
        let c = loud.at_with(0.0, 1.0, 1.0, &Inputs { audio: 0.25, bounds: Some(b), ..Default::default() });
        assert_ne!(c.color_at(9.9, 1.0), Rgb::BLACK, "sensitivity scales the level");
    }

    #[test]
    fn audio_meter_is_transparent_where_unlit() {
        let e = Effect::Studio {
            layers: vec![
                layer(Effect::Static { color: BLUE }, Mask::All),
                layer(Effect::AudioMeter { sensitivity: 1.0 }, Mask::All),
            ],
        };
        let b = Bounds { min_x: 0.0, min_y: 0.0, max_x: 10.0, max_y: 5.0 };
        let f = e.at_with(0.0, 1.0, 1.0, &Inputs { audio: 0.3, bounds: Some(b), ..Default::default() });
        assert_eq!(f.color_at(9.0, 1.0), BLUE);
        assert_eq!(f.color_at(1.0, 1.0).1, 255);
    }

    // ---- studio ----

    #[test]
    fn studio_layers_blend_bottom_to_top() {
        let stack = |a: Effect, b: Effect| Effect::Studio { layers: vec![layer(a, Mask::All), layer(b, Mask::All)] };
        let red_over_blue = stack(Effect::Static { color: BLUE }, Effect::Static { color: RED });
        assert_eq!(red_over_blue.at(0.0, 1.0, 1.0).color_at(1.0, 1.0), RED, "the top layer wins");
        let blue_over_red = stack(Effect::Static { color: RED }, Effect::Static { color: BLUE });
        assert_eq!(blue_over_red.at(0.0, 1.0, 1.0).color_at(1.0, 1.0), BLUE);

        let mut half = red_over_blue.clone();
        if let Effect::Studio { layers } = &mut half {
            layers[1].opacity = 0.5;
        }
        assert_eq!(half.at(0.0, 1.0, 1.0).color_at(1.0, 1.0), Rgb(128, 0, 128), "opacity mixes");

        let mut off = red_over_blue.clone();
        if let Effect::Studio { layers } = &mut off {
            layers[1].enabled = false;
        }
        assert_eq!(off.at(0.0, 1.0, 1.0).color_at(1.0, 1.0), BLUE, "disabled layers are skipped");

        let empty = Effect::Studio { layers: vec![] };
        assert_eq!(empty.at(0.0, 1.0, 1.0).color_at(1.0, 1.0), Rgb::BLACK);
    }

    #[test]
    fn studio_masks_pick_devices_and_keys() {
        let e = Effect::Studio {
            layers: vec![
                layer(Effect::Static { color: BLUE }, Mask::All),
                layer(Effect::Static { color: RED }, Mask::Devices { ids: vec!["mouse".into()] }),
                layer(
                    Effect::Static { color: Rgb(0, 255, 0) },
                    Mask::Keys { device: "kb".into(), shapes: vec!["W".into(), "Left Shift".into()] },
                ),
            ],
        };
        let f = e.at(0.0, 1.0, 1.0);
        assert_eq!(f.color_led("kb", "Q", 1.0, 1.0), BLUE);
        assert_eq!(f.color_led("kb", "W", 1.0, 1.0), Rgb(0, 255, 0));
        assert_eq!(f.color_led("kb", "Left Shift", 1.0, 1.0), Rgb(0, 255, 0));
        assert_eq!(f.color_led("mouse", "W", 1.0, 1.0), RED, "a key mask is per device");
        assert_eq!(f.color_led("mouse", "Logo", 1.0, 1.0), RED);
        assert_eq!(f.color_led("mat", "Edge", 1.0, 1.0), BLUE);
        assert_eq!(f.color_at(1.0, 1.0), BLUE, "color_at only applies whole-desk layers");
    }

    #[test]
    fn studio_off_layer_is_opaque_and_reactive_over_a_base() {
        let e = Effect::Studio {
            layers: vec![
                layer(Effect::Static { color: BLUE }, Mask::All),
                layer(Effect::Reactive { color: Some(RED), fade_s: 1.0 }, Mask::All),
            ],
        };
        let presses = [Press { x: 2.0, y: 2.0, t: 0.0 }];
        let inp = Inputs { presses: &presses, ..Default::default() };
        let f = e.at_with(0.5, 1.0, 1.0, &inp);
        assert_eq!(f.color_at(8.0, 2.0), BLUE, "reactive is transparent away from presses");
        assert_eq!(f.color_at(2.0, 2.0), Rgb(128, 0, 128), "half-faded press blends half over the base");

        let blackout = Effect::Studio {
            layers: vec![layer(Effect::Static { color: BLUE }, Mask::All), layer(Effect::Off, Mask::All)],
        };
        assert_eq!(blackout.at(0.0, 1.0, 1.0).color_at(1.0, 1.0), Rgb::BLACK);
    }

    #[test]
    fn needs_keys_and_audio() {
        assert!(!Effect::default().uses_keys());
        assert!(Effect::Reactive { color: None, fade_s: 1.0 }.uses_keys());
        assert!(Effect::Ripple { color: None, speed: 1.0, width: 1.0, fade_s: 1.0 }.uses_keys());
        assert!(Effect::AudioMeter { sensitivity: 1.0 }.uses_audio());
        let mut studio = Effect::Studio {
            layers: vec![
                layer(Effect::default(), Mask::All),
                layer(Effect::Ripple { color: None, speed: 1.0, width: 1.0, fade_s: 1.0 }, Mask::All),
            ],
        };
        assert!(studio.uses_keys() && !studio.uses_audio());
        if let Effect::Studio { layers } = &mut studio {
            layers[1].enabled = false;
        }
        assert!(!studio.uses_keys(), "a disabled layer doesn't need input");
    }

    // ---- JSON contract (apps/uncoil/src/lib/types.ts) ----

    fn roundtrip(json: &str) -> Effect {
        let e: Effect = serde_json::from_str(json).unwrap_or_else(|err| panic!("{json}: {err}"));
        let back: serde_json::Value = serde_json::from_str(&serde_json::to_string(&e).unwrap()).unwrap();
        let want: serde_json::Value = serde_json::from_str(json).unwrap();
        assert_eq!(back, want, "{json} must serialise back unchanged");
        e
    }

    #[test]
    fn json_matches_types_ts() {
        roundtrip(r#"{"kind":"wave","angle_deg":35.0,"period_s":14.0,"wavelength":26.0,"reverse":false}"#);
        roundtrip(r#"{"kind":"spectrum","period_s":14.0}"#);
        roundtrip(r#"{"kind":"static","color":[255,128,0]}"#);
        roundtrip(r#"{"kind":"off"}"#);
        roundtrip(r#"{"kind":"breathing","colors":[[255,0,0],[0,0,255]],"period_s":4.0}"#);
        roundtrip(r#"{"kind":"breathing","colors":[],"period_s":4.0}"#);
        roundtrip(r#"{"kind":"starlight","colors":[],"density":0.25,"twinkle_s":1.5}"#);
        roundtrip(r#"{"kind":"fire","speed":1.5,"height":0.5}"#);
        roundtrip(r#"{"kind":"wheel","period_s":6.0,"reverse":true,"center":null}"#);
        roundtrip(r#"{"kind":"wheel","period_s":6.0,"reverse":false,"center":[8.5,3.0]}"#);
        roundtrip(r#"{"kind":"reactive","color":null,"fade_s":1.0}"#);
        roundtrip(r#"{"kind":"reactive","color":[0,255,0],"fade_s":0.5}"#);
        roundtrip(r#"{"kind":"ripple","color":null,"speed":12.0,"width":1.5,"fade_s":1.2}"#);
        roundtrip(r#"{"kind":"audio_meter","sensitivity":2.0}"#);
        let studio = roundtrip(
            r#"{"kind":"studio","layers":[
                {"name":"Base","enabled":true,"opacity":1.0,
                 "effect":{"kind":"wave","angle_deg":35.0,"period_s":14.0,"wavelength":26.0,"reverse":false},
                 "mask":{"kind":"all"}},
                {"name":"Mouse","enabled":false,"opacity":0.5,"effect":{"kind":"static","color":[1,2,3]},
                 "mask":{"kind":"devices","ids":["razer-basilisk-v3-pro"]}},
                {"name":"WASD","enabled":true,"opacity":0.75,
                 "effect":{"kind":"reactive","color":null,"fade_s":1.0},
                 "mask":{"kind":"keys","device":"razer-blackwidow-v4-pro-75","shapes":["W","A","S","D","Left Shift"]}}
            ]}"#,
        );
        assert!(matches!(&studio, Effect::Studio { layers } if layers.len() == 3 && !layers[1].enabled));
    }

    #[test]
    fn json_fills_defaults_for_missing_fields() {
        let e: Effect = serde_json::from_str(r#"{"kind":"ripple"}"#).unwrap();
        assert_eq!(e, Effect::Ripple { color: None, speed: 12.0, width: 1.5, fade_s: 1.0 });
        let l: StudioLayer = serde_json::from_str(r#"{"effect":{"kind":"off"}}"#).unwrap();
        assert!(l.enabled && l.opacity == 1.0 && l.mask == Mask::All);
    }

    /// `apps/uncoil/src/lib/mock/fixtures.json`: what the engine answers, for the TypeScript mirrors
    /// (`effect.ts`, `effects.ts`, `keys.ts`, the mock's default config) to be checked against by
    /// `apps/uncoil/scripts/check-mirror.mjs` (part of `pnpm check`). Regenerate with
    /// `UNCOIL_UPDATE_MOCK=1 cargo test -p uncoil-core ts_mirror`.
    #[test]
    fn ts_mirror_fixtures() {
        use crate::config::Config;
        use serde_json::{json, Value};
        let defs = crate::device::builtin();
        let desk = crate::layout::Desk::new(&defs, &Default::default(), |_| false);
        // a spread of LEDs over the three devices: corners, middle, underglow, mouse, mat
        let wanted = [
            ("razer-blackwidow-v4-pro-75", ["Escape", "W", "Left Shift", "Space", "Delete", "LU1", "RU5"].as_slice()),
            ("razer-basilisk-v3-pro", &["Logo", "Wheel"]),
            ("razer-goliathus-chroma-extended", &["Edge"]),
        ];
        let mut points: Vec<(String, String, f32, f32)> = vec![];
        for (id, shapes) in wanted {
            let dev = &desk.devices.iter().find(|(_, d)| d.id == id).unwrap().1;
            for s in dev.shapes.iter().filter(|s| shapes.contains(&s.name.as_str())) {
                points.push((id.into(), s.name.clone(), s.x, s.y));
            }
        }
        // and some points between and around the devices (no LED: device and shape are empty)
        for (x, y) in [(-1.75, -1.25), (3.3, 2.7), (12.0, 7.9), (23.9, 0.1), (18.6, 5.55)] {
            points.push((String::new(), String::new(), x, y));
        }
        let w = desk.devices[0].1.shape_position("W").unwrap();
        let presses =
            [Press { x: w.0, y: w.1, t: 0.5 }, Press { x: 14.0, y: 4.0, t: 2.9 }, Press { x: 20.9, y: 3.3, t: 3.0 }];
        let studio = r#"{"kind":"studio","layers":[
            {"name":"base","enabled":true,"opacity":1,"effect":{"kind":"wave","angle_deg":20,"period_s":9,"wavelength":18,"reverse":false},"mask":{"kind":"all"}},
            {"name":"mouse","enabled":true,"opacity":0.6,"effect":{"kind":"static","color":[255,40,0]},"mask":{"kind":"devices","ids":["razer-basilisk-v3-pro"]}},
            {"name":"off","enabled":false,"opacity":1,"effect":{"kind":"static","color":[0,255,0]},"mask":{"kind":"all"}},
            {"name":"keys","enabled":true,"opacity":0.75,"effect":{"kind":"reactive","color":null,"fade_s":1.5},"mask":{"kind":"keys","device":"razer-blackwidow-v4-pro-75","shapes":["W","Escape"]}},
            {"name":"stars","enabled":true,"opacity":0.5,"effect":{"kind":"starlight","colors":[],"density":0.4,"twinkle_s":1.1},"mask":{"kind":"all"}},
            {"name":"rings","enabled":true,"opacity":1,"effect":{"kind":"ripple","color":[0,120,255],"speed":9,"width":2,"fade_s":2},"mask":{"kind":"all"}}
        ]}"#;
        let effects: Vec<(&str, &str)> = vec![
            ("wave", r#"{"kind":"wave","angle_deg":35,"period_s":14,"wavelength":26,"reverse":false}"#),
            ("wave reversed", r#"{"kind":"wave","angle_deg":120,"period_s":3,"wavelength":7.5,"reverse":true}"#),
            ("spectrum", r#"{"kind":"spectrum","period_s":5}"#),
            ("static", r#"{"kind":"static","color":[224,163,62]}"#),
            ("off", r#"{"kind":"off"}"#),
            ("breathing rainbow", r#"{"kind":"breathing","colors":[],"period_s":4}"#),
            ("breathing two", r#"{"kind":"breathing","colors":[[255,0,0],[0,0,255]],"period_s":2.5}"#),
            ("starlight", r#"{"kind":"starlight","colors":[],"density":0.5,"twinkle_s":1.5}"#),
            (
                "starlight gels",
                r#"{"kind":"starlight","colors":[[255,217,168],[38,198,218]],"density":0.3,"twinkle_s":0.7}"#,
            ),
            ("fire", r#"{"kind":"fire","speed":1,"height":0.6}"#),
            ("fire fast", r#"{"kind":"fire","speed":2.5,"height":0.3}"#),
            ("wheel", r#"{"kind":"wheel","period_s":6,"reverse":false,"center":null}"#),
            ("wheel centred", r#"{"kind":"wheel","period_s":4,"reverse":true,"center":[20,3]}"#),
            ("reactive", r#"{"kind":"reactive","color":null,"fade_s":1}"#),
            ("reactive gel", r#"{"kind":"reactive","color":[106,168,79],"fade_s":3}"#),
            ("ripple", r#"{"kind":"ripple","color":null,"speed":14,"width":2,"fade_s":1.5}"#),
            ("audio meter", r#"{"kind":"audio_meter","sensitivity":1.5}"#),
            ("studio", studio),
        ];
        let times = [0.0f32, 0.7, 3.3, 12.9];
        let (sat, val, audio) = (0.85f32, 0.9f32, 0.45f32);
        let inputs = desk.inputs(&presses, audio);
        let samples: Vec<Value> = effects
            .iter()
            .map(|(name, src)| {
                let effect: Effect = serde_json::from_str(src).unwrap_or_else(|e| panic!("{name}: {e}"));
                let colors: Vec<Vec<[u8; 3]>> = times
                    .iter()
                    .map(|&t| {
                        let f = effect.at_with(t, sat, val, &inputs);
                        points.iter().map(|(d, s, x, y)| f.color_led(d, s, *x, *y).bytes()).collect()
                    })
                    .collect();
                json!({"name": name, "effect": effect, "colors": colors})
            })
            .collect();
        let kinds = [
            "wave",
            "spectrum",
            "static",
            "off",
            "breathing",
            "starlight",
            "fire",
            "wheel",
            "reactive",
            "ripple",
            "audio_meter",
        ];
        let mut defaults = serde_json::Map::new();
        for k in kinds {
            let src = if k == "static" { json!({"kind": k, "color": [0, 0, 0]}) } else { json!({"kind": k}) };
            let e: Effect = serde_json::from_value(src).unwrap();
            defaults.insert(k.into(), serde_json::to_value(&e).unwrap());
        }
        assert!(effects.iter().all(|(_, src)| kinds.iter().any(|k| src.contains(&format!("\"kind\":\"{k}\"")))));
        let usage_names: Vec<String> = (0..=255u8).filter_map(crate::features::keymap::usage_name).collect();
        let fixtures = json!({
            "about": "Written by `ts_mirror_fixtures` in crates/uncoil-core/src/effect.rs; checked by scripts/check-mirror.mjs.",
            "config_default": Config::default(),
            "effect_defaults": defaults,
            "usage_names": usage_names,
            "inputs": {"presses": presses, "audio": audio, "bounds": inputs.bounds.map(|b| json!({"minX": b.min_x, "minY": b.min_y, "maxX": b.max_x, "maxY": b.max_y})), "keyboard_center": inputs.keyboard_center},
            "sat": sat,
            "val": val,
            "times": times,
            "points": points,
            "samples": samples,
        });
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/uncoil/src/lib/mock/fixtures.json");
        let real = serde_json::to_string(&fixtures).unwrap() + "\n";
        if std::env::var_os("UNCOIL_UPDATE_MOCK").is_some() {
            std::fs::write(&path, &real).unwrap();
        }
        let file = std::fs::read_to_string(&path).unwrap_or_default().replace("\r\n", "\n");
        assert!(file == real, "{} is stale; rerun with UNCOIL_UPDATE_MOCK=1", path.display());
    }
}
