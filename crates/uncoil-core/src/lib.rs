//! uncoil-core: everything that can be reasoned about without touching hardware.
//!
//! * [`proto`]   — the 90-byte Razer feature report (builder, CRC, known commands)
//! * [`color`]   — LED-tuned colour maths (FastLED's rainbow hue map)
//! * [`effect`]  — effects evaluated at a desk position and time
//! * [`device`]  — device definitions loaded from `devices/*.toml`
//! * [`layout`]  — where every LED physically sits on the desk
//! * [`config`]  — the user config shared by the daemon and the GUI
//! * [`scancode`] — key scan codes to desk layout shape names (reactive effects)
//! * [`features`] — device features beyond lighting frames: firmware effects, key maps, profiles, the
//!   OLED command dial and display (report builders + reply parsers)
//! * [`ipc`]     — the daemon's control-pipe protocol, shared by the CLI and the GUI
//! * [`owners`]  — programs that light PC parts themselves, and which devices uncoil leaves them

pub mod color;
pub mod config;
pub mod device;
pub mod effect;
pub mod features;
pub mod ipc;
pub mod layout;
pub mod owners;
pub mod proto;
pub mod scancode;

pub use color::Rgb;
