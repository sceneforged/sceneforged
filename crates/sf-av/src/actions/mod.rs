//! Media processing actions: remux, DV conversion, audio, track stripping,
//! arbitrary command execution, and Profile B encoding.

mod remux;
mod dovi;
mod audio;
mod strip;
mod exec;
mod profile_b;

pub use remux::remux;
pub use dovi::convert_dv_profile;
pub use audio::add_compat_audio;
pub use strip::strip_tracks;
pub use exec::exec_command;
pub use profile_b::{adaptive_crf, convert_to_profile_b, convert_to_profile_b_with_progress, EncodeProgress};
