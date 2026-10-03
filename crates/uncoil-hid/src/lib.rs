//! uncoil-hid: talking to real hardware.
//!
//! * [`transport`] — open Razer devices by their definition's USB endpoint and send/ack reports,
//!   honouring each device's quirks.
//! * [`display`]   — Windows display power notifications (on / off / dimmed).

pub mod display;
pub mod transport;

pub use transport::{discover, LiveDevice};
