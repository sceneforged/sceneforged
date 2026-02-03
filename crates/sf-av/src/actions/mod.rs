//! Media processing actions: remux, DV conversion, audio, track stripping,
//! and arbitrary command execution.

mod remux;
mod dovi;
mod audio;
mod strip;
mod exec;

pub use remux::remux;
pub use dovi::convert_dv_profile;
pub use audio::add_compat_audio;
pub use strip::strip_tracks;
pub use exec::exec_command;
