//! Media processing actions.
//!
//! This module provides various media processing operations including:
//! - Remuxing between container formats
//! - Dolby Vision profile conversion
//! - Audio track manipulation
//! - Track stripping

#[cfg(feature = "remux")]
mod remux;

#[cfg(feature = "dovi")]
mod dovi;

#[cfg(feature = "audio")]
mod audio;

mod strip;

#[cfg(feature = "remux")]
pub use remux::{remux, Container};

#[cfg(feature = "dovi")]
pub use dovi::{convert_dv_profile, DvProfile};

#[cfg(feature = "audio")]
pub use audio::{add_compat_audio, AudioCodec};

pub use strip::{strip_tracks, StripConfig};
