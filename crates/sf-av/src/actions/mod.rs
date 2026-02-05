//! Media processing actions: remux, DV conversion, audio, track stripping,
//! arbitrary command execution, Profile B encoding, and HLS segmentation.

mod remux;
mod dovi;
mod audio;
mod strip;
mod exec;
mod profile_b;
mod hls_segment;

pub use remux::remux;
pub use dovi::convert_dv_profile;
pub use audio::add_compat_audio;
pub use strip::strip_tracks;
pub use exec::exec_command;
pub use profile_b::{adaptive_crf, convert_to_profile_b};
pub use hls_segment::generate_hls_segments;
