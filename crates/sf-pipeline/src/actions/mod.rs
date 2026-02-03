//! Built-in pipeline actions.
//!
//! Each action wraps a corresponding [`sf_av::actions`] function, presenting it
//! behind the unified [`Action`](crate::action::Action) trait.

mod dv_convert;
mod remux;
mod add_compat_audio;
mod strip_tracks;
mod exec;

pub use dv_convert::DvConvertAction;
pub use remux::RemuxAction;
pub use add_compat_audio::AddCompatAudioAction;
pub use strip_tracks::StripTracksAction;
pub use exec::ExecAction;
