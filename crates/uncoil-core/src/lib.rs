//! uncoil-core: everything that can be reasoned about without touching hardware.
//!
//! * [`proto`]   — the 90-byte Razer feature report (builder, CRC, known commands)
//! * [`color`]   — LED-tuned colour maths (FastLED's rainbow hue map)
//! * [`effect`]  — effects evaluated at a desk position and time
//! * [`device`]  — device definitions loaded from `devices/*.toml`
//! * [`layout`]  — where every LED physically sits on the desk
//! * [`config`]  — the user config shared by the daemon and the GUI

pub mod color;
pub mod config;
pub mod device;
pub mod effect;
pub mod layout;
pub mod proto;

pub use color::Rgb;
