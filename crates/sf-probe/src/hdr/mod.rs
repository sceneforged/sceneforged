//! HDR format detection from HEVC bitstream data.

mod detect;
mod dolby_vision;

pub(crate) use detect::detect_hdr_from_hevc;
pub(crate) use dolby_vision::detect_dolby_vision;
