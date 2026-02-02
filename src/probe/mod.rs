// Re-export all probe functionality from sceneforged-av
pub use sceneforged_av::probe::*;
pub use sceneforged_av::{
    check_tool, check_tools, require_tool, AudioTrack, DolbyVisionInfo, HdrFormat, MediaInfo,
    ProbeBackend, SubtitleTrack, ToolInfo, VideoTrack,
};

use anyhow::Result;
use std::path::Path;

/// Probe a media file, preferring mediainfo but falling back to ffprobe
pub fn probe_file(path: &Path) -> Result<MediaInfo> {
    sceneforged_av::probe(path).map_err(|e| anyhow::anyhow!("{}", e))
}
