//! Effects are pure functions of (desk position, time). Every device samples the same field, so a wave
//! flows continuously from the keyboard onto the mouse and mat.

use crate::color::{rainbow, Rgb};
use serde::{Deserialize, Serialize};

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

impl Default for Effect {
    fn default() -> Self {
        Effect::Wave { angle_deg: d_angle(), period_s: d_period(), wavelength: d_wavelength(), reverse: false }
    }
}

/// An effect frozen at one instant, cheap to evaluate per LED.
pub struct Frame {
    kind: FrameKind,
    sat: f32,
    val: f32,
}

enum FrameKind {
    Wave { ux: f32, uy: f32, inv_wl: f32, phase: f32 },
    Uniform(f32),
    Static(Rgb),
    Off,
}

impl Effect {
    /// Prepare the effect for time `t` (seconds) at brightness `val` and saturation `sat`.
    pub fn at(&self, t: f32, sat: f32, val: f32) -> Frame {
        let kind = match *self {
            Effect::Wave { angle_deg, period_s, wavelength, reverse } => {
                let a = angle_deg.to_radians();
                let dir = if reverse { -1.0 } else { 1.0 };
                FrameKind::Wave {
                    ux: a.cos(),
                    uy: a.sin(),
                    inv_wl: 1.0 / wavelength.max(1.0),
                    phase: dir * t / period_s.max(0.5),
                }
            }
            Effect::Spectrum { period_s } => FrameKind::Uniform(t / period_s.max(0.5)),
            Effect::Static { color } => FrameKind::Static(color),
            Effect::Off => FrameKind::Off,
        };
        Frame { kind, sat, val }
    }
}

impl Frame {
    pub fn color_at(&self, x: f32, y: f32) -> Rgb {
        match self.kind {
            FrameKind::Wave { ux, uy, inv_wl, phase } => {
                rainbow((x * ux + y * uy) * inv_wl - phase, self.sat, self.val)
            }
            FrameKind::Uniform(h) => rainbow(h, self.sat, self.val),
            FrameKind::Static(c) => c.scale(self.val),
            FrameKind::Off => Rgb::BLACK,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
