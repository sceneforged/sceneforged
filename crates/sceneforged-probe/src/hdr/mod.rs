//! HDR format detection

pub mod bitstream;
pub mod dolby_vision;

pub use bitstream::detect_hdr_from_hevc;
pub use dolby_vision::detect_dolby_vision;
