//! uncoil-hid: talking to real hardware.
//!
//! * [`transport`] — open Razer devices by their definition's USB endpoint and send/ack reports,
//!   honouring each device's quirks.
//! * [`display`]   — Windows display power notifications (on / off / dimmed).
//! * [`keys`]      — key press notifications (Raw Input) for reactive effects; positions only, never logged.
//! * [`audio`]     — the default playback device's peak level (WASAPI meter) for the audio meter effect.

pub mod audio;
pub mod display;
pub mod keys;
pub mod transport;

pub use transport::{discover, LiveDevice};
