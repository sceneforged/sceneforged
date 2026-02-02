//! Data model types for parsed media release information.
//!
//! This module contains all the types used to represent parsed metadata
//! from media release names, including quality, codecs, languages, and more.

mod codec;
mod edition;
mod episode;
mod field;
mod frame_rate;
mod hdr;
mod language;
mod media_type;
mod quality;
mod release;
mod streaming;

pub use codec::{AudioChannels, AudioCodec, VideoEncoder, VideoStandard};
pub use edition::Edition;
pub use episode::Revision;
pub use field::{Confidence, OptionalField, ParsedField};
pub use frame_rate::FrameRate;
pub use hdr::HdrFormat;
pub use language::Language;
pub use media_type::MediaType;
pub use quality::{QualityModifier, Resolution, Source};
pub use release::ParsedRelease;
pub use streaming::StreamingService;

/// Error type for parsing enum values from strings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError(pub String);

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "parse error: {}", self.0)
    }
}

impl std::error::Error for ParseError {}
