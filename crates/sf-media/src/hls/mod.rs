//! HLS playlist generation.
//!
//! This module generates M3U8 playlists for HLS streaming, supporting
//! both master playlists (with multiple variants) and media playlists
//! (with segment lists and optional init segment references).

mod generator;
mod types;

pub use generator::{generate_master_playlist, generate_media_playlist};
pub use types::{MasterPlaylist, MediaPlaylist, Segment, Variant};
