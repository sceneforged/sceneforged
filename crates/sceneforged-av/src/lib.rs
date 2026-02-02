//! # sceneforged-av
//!
//! Media probing and processing library for video files.
//!
//! This crate provides functionality for:
//! - Probing media files to extract metadata (codecs, HDR info, Dolby Vision, etc.)
//! - Remuxing between container formats (MKV, MP4, etc.)
//! - Dolby Vision profile conversion
//! - Audio track manipulation
//!
//! ## Features
//!
//! - `probe` (default) - Core probing functionality using ffprobe/mediainfo
//! - `remux` - Container remuxing using ffmpeg/mkvmerge
//! - `dovi` - Dolby Vision processing using native dolby_vision crate
//! - `audio` - Audio transcoding using ffmpeg
//! - `all` - Enable all features
//! - `async` - Async subprocess execution
//! - `tracing` - Enable tracing support
//!
//! ## Example
//!
//! ```no_run
//! use sceneforged_av::probe;
//!
//! let info = probe("/path/to/video.mkv")?;
//! println!("Container: {}", info.container);
//! if info.has_dolby_vision() {
//!     println!("Dolby Vision Profile: {:?}", info.dolby_vision_profile());
//! }
//! # Ok::<(), sceneforged_av::Error>(())
//! ```

mod error;
pub mod probe;
pub mod template;
pub mod tools;
pub mod workspace;

#[cfg(feature = "remux")]
pub mod actions;

// Re-exports
pub use error::{Error, Result};
pub use probe::{AudioTrack, DolbyVisionInfo, HdrFormat, MediaInfo, SubtitleTrack, VideoTrack};
pub use template::TemplateContext;
pub use tools::{check_tool, check_tools, require_tool, ToolInfo};
pub use workspace::Workspace;

/// Probe a media file and return its metadata.
///
/// This is the main entry point for probing files. It will try mediainfo first
/// (better HDR/DV detection) and fall back to ffprobe.
///
/// # Example
///
/// ```no_run
/// use sceneforged_av::probe;
///
/// let info = probe("/path/to/video.mkv")?;
/// println!("Video codec: {}", info.video_tracks[0].codec);
/// # Ok::<(), sceneforged_av::Error>(())
/// ```
pub fn probe<P: AsRef<std::path::Path>>(path: P) -> Result<MediaInfo> {
    probe::probe(path.as_ref())
}

/// Probe a media file using a specific backend.
pub fn probe_with<P: AsRef<std::path::Path>>(path: P, backend: ProbeBackend) -> Result<MediaInfo> {
    probe::probe_with(path.as_ref(), backend)
}

/// Backend to use for probing media files.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProbeBackend {
    /// Try the best available backend (pure-rust > native-ffmpeg > mediainfo > ffprobe)
    #[default]
    Auto,
    /// Use ffprobe CLI (parses JSON output)
    Ffprobe,
    /// Use mediainfo CLI (parses JSON output, better DV/HDR detection)
    MediaInfo,
    /// Use native FFmpeg bindings (no subprocess, requires `native-ffmpeg` feature)
    #[cfg(feature = "native-ffmpeg")]
    NativeFfmpeg,
    /// Use pure Rust probing (no external tools, requires `pure-rust-probe` feature)
    #[cfg(feature = "pure-rust-probe")]
    PureRust,
}
