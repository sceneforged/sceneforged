//! Probe backends that shell out to external tools.
//!
//! Both [`FfprobeProber`] and [`MediaInfoProber`] implement the
//! [`sf_probe::Prober`] trait, allowing them to be used interchangeably
//! (or combined via [`sf_probe::CompositeProber`]).

pub mod ffprobe;
pub mod mediainfo;

pub use self::ffprobe::FfprobeProber;
pub use self::mediainfo::MediaInfoProber;
