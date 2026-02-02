//! Container remuxing operations.

use crate::{Error, Result, Workspace};
use std::path::Path;
use std::process::Command;

#[cfg(feature = "native-ffmpeg")]
use ffmpeg_the_third as ffmpeg;
#[cfg(feature = "native-ffmpeg")]
use std::sync::Once;

#[cfg(feature = "native-ffmpeg")]
static FFMPEG_INIT: Once = Once::new();

#[cfg(feature = "native-ffmpeg")]
fn init_ffmpeg() {
    FFMPEG_INIT.call_once(|| {
        ffmpeg::init().expect("Failed to initialize FFmpeg");
    });
}

/// Supported container formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Container {
    /// Matroska container
    Mkv,
    /// MPEG-4 Part 14 container
    Mp4,
    /// MPEG transport stream
    Ts,
    /// QuickTime container
    Mov,
    /// WebM container
    Webm,
    /// AVI container
    Avi,
    /// M2TS (Blu-ray) container
    M2ts,
}

impl Container {
    /// Get the file extension for this container.
    pub fn extension(&self) -> &'static str {
        match self {
            Container::Mkv => "mkv",
            Container::Mp4 => "mp4",
            Container::Ts => "ts",
            Container::Mov => "mov",
            Container::Webm => "webm",
            Container::Avi => "avi",
            Container::M2ts => "m2ts",
        }
    }
}

impl std::str::FromStr for Container {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "mkv" | "matroska" => Ok(Container::Mkv),
            "mp4" | "m4v" => Ok(Container::Mp4),
            "ts" | "mpegts" => Ok(Container::Ts),
            "mov" | "quicktime" => Ok(Container::Mov),
            "webm" => Ok(Container::Webm),
            "avi" => Ok(Container::Avi),
            "m2ts" => Ok(Container::M2ts),
            _ => Err(format!("Unknown container format: {}", s)),
        }
    }
}

impl Container {
    /// Get the FFmpeg muxer format name for this container.
    ///
    /// This is used with `ffmpeg::format::output_as()` to specify the output format.
    #[cfg(feature = "native-ffmpeg")]
    pub fn ffmpeg_format_name(&self) -> &'static str {
        match self {
            Container::Mkv => "matroska",
            Container::Mp4 => "mp4",
            Container::Ts => "mpegts",
            Container::Mov => "mov",
            Container::Webm => "webm",
            Container::Avi => "avi",
            Container::M2ts => "mpegts", // M2TS uses the same muxer as TS
        }
    }
}

/// Remux a media file to a different container format.
///
/// Uses mkvmerge for MKV output (better metadata handling) and falls back to
/// ffmpeg for other containers.
pub fn remux(workspace: &Workspace, target: Container) -> Result<()> {
    let input = workspace.input();
    let target_ext = target.extension();

    // Determine output path with new extension
    let output = workspace.temp_file(&format!(
        "{}.{}",
        input.file_stem().unwrap().to_string_lossy(),
        target_ext
    ));

    #[cfg(feature = "tracing")]
    tracing::info!("Remuxing {:?} to {}", input, target_ext);

    // Try mkvmerge first for MKV output (better metadata handling)
    if target == Container::Mkv {
        match remux_with_mkvmerge(input, &output) {
            Ok(()) => {
                // Copy to workspace output
                std::fs::rename(&output, workspace.output())?;
                return Ok(());
            }
            Err(e) => {
                #[cfg(feature = "tracing")]
                tracing::warn!("mkvmerge failed, falling back to ffmpeg: {}", e);
                let _ = e; // Suppress unused warning when tracing is disabled
            }
        }
    }

    // Use native FFmpeg bindings for non-MKV containers, with CLI fallback
    remux_non_mkv(input, &output, target)?;

    // Move to workspace output location
    std::fs::rename(&output, workspace.output())?;

    #[cfg(feature = "tracing")]
    tracing::info!("Remux complete: {:?}", workspace.output());

    Ok(())
}

fn remux_with_mkvmerge(input: &Path, output: &Path) -> Result<()> {
    #[cfg(feature = "tracing")]
    tracing::debug!("Remuxing with mkvmerge: {:?} -> {:?}", input, output);

    let result = Command::new("mkvmerge")
        .arg("-o")
        .arg(output)
        .arg(input)
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                Error::tool_not_found("mkvmerge")
            } else {
                Error::Io(e)
            }
        })?;

    // mkvmerge returns 0 for success, 1 for warnings (still OK), 2 for errors
    if !result.status.success() && result.status.code() != Some(1) {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(Error::tool_failed("mkvmerge", stderr.to_string()));
    }

    Ok(())
}

/// Remux non-MKV containers.
///
/// Tries native FFmpeg bindings first (if enabled), falls back to CLI.
#[cfg(feature = "native-ffmpeg")]
fn remux_non_mkv(input: &Path, output: &Path, container: Container) -> Result<()> {
    match remux_native(input, output, container) {
        Ok(()) => Ok(()),
        Err(e) => {
            #[cfg(feature = "tracing")]
            tracing::warn!("Native FFmpeg remuxing failed, falling back to CLI: {}", e);
            let _ = e; // Suppress unused warning when tracing is disabled
            remux_with_ffmpeg(input, output, container)
        }
    }
}

/// Remux non-MKV containers using ffmpeg CLI.
#[cfg(not(feature = "native-ffmpeg"))]
fn remux_non_mkv(input: &Path, output: &Path, container: Container) -> Result<()> {
    remux_with_ffmpeg(input, output, container)
}

/// Remux a file using native FFmpeg bindings.
///
/// This uses ffmpeg-the-third to copy all streams without re-encoding,
/// supporting container-specific options for MP4, TS, MOV, WebM, and AVI.
#[cfg(feature = "native-ffmpeg")]
fn remux_native(input: &Path, output: &Path, container: Container) -> Result<()> {
    init_ffmpeg();

    #[cfg(feature = "tracing")]
    tracing::debug!(
        "Remuxing with native FFmpeg: {:?} -> {:?} ({:?})",
        input,
        output,
        container
    );

    // Open input file
    let mut input_ctx = ffmpeg::format::input(input).map_err(|e| {
        if e.to_string().contains("No such file") {
            Error::file_not_found(input)
        } else {
            Error::tool_failed("ffmpeg", format!("Failed to open input: {}", e))
        }
    })?;

    // Create output context with specific format
    let format_name = container.ffmpeg_format_name();
    let mut output_ctx = ffmpeg::format::output_as(output, format_name)
        .map_err(|e| Error::tool_failed("ffmpeg", format!("Failed to create output: {}", e)))?;

    // Store time bases for timestamp rescaling
    let mut input_time_bases: Vec<ffmpeg::Rational> = Vec::new();

    // Add all streams to output (copy mode)
    for input_stream in input_ctx.streams() {
        let codec_params = input_stream.parameters();

        let mut output_stream = output_ctx
            .add_stream(ffmpeg::encoder::find(ffmpeg::codec::Id::None))
            .map_err(|e| {
                Error::tool_failed("ffmpeg", format!("Failed to add output stream: {}", e))
            })?;

        output_stream.set_parameters(codec_params);

        // Reset codec_tag to avoid incompatible codec tag issues when muxing
        // to a different container format
        unsafe {
            (*(*output_stream.as_mut_ptr()).codecpar).codec_tag = 0;
        }

        input_time_bases.push(input_stream.time_base());

        #[cfg(feature = "tracing")]
        tracing::trace!("Added stream {} to output", input_stream.index());
    }

    // Copy metadata from input
    output_ctx.set_metadata(input_ctx.metadata().to_owned());

    // Write header (this also sets up container-specific options)
    output_ctx.write_header().map_err(|e| {
        Error::tool_failed("ffmpeg", format!("Failed to write output header: {}", e))
    })?;

    // Copy packets from input to output
    for result in input_ctx.packets() {
        let (stream, packet) = result?;
        let input_idx = stream.index();
        let mut packet = packet;

        // Get time bases for rescaling
        let input_time_base = input_time_bases
            .get(input_idx)
            .copied()
            .unwrap_or(stream.time_base());
        let output_stream = output_ctx
            .stream(input_idx)
            .ok_or_else(|| Error::tool_failed("ffmpeg", "Output stream not found".to_string()))?;
        let output_time_base = output_stream.time_base();

        // Rescale timestamps
        packet.rescale_ts(input_time_base, output_time_base);
        packet.set_stream(input_idx);
        packet.set_position(-1);

        // Write packet
        packet
            .write_interleaved(&mut output_ctx)
            .map_err(|e| Error::tool_failed("ffmpeg", format!("Failed to write packet: {}", e)))?;
    }

    // Write trailer
    output_ctx.write_trailer().map_err(|e| {
        Error::tool_failed("ffmpeg", format!("Failed to write output trailer: {}", e))
    })?;

    // Handle MP4-specific post-processing (faststart)
    if container == Container::Mp4 {
        #[cfg(feature = "tracing")]
        tracing::debug!("Applying faststart optimization for MP4");
        apply_mp4_faststart(output)?;
    }

    #[cfg(feature = "tracing")]
    tracing::info!("Native FFmpeg remuxing complete: {:?}", output);

    Ok(())
}

/// Apply MP4 faststart optimization by moving the moov atom to the beginning.
///
/// This uses qt-faststart logic to relocate metadata for faster streaming starts.
/// If the native approach fails, the file is left as-is (still valid, just not optimized).
#[cfg(feature = "native-ffmpeg")]
fn apply_mp4_faststart(path: &Path) -> Result<()> {
    // The ffmpeg-the-third library doesn't have built-in faststart support,
    // so we'll use a simple approach: re-read and re-write with the faststart muxer option
    // by running a quick ffmpeg CLI pass, or leave as-is if unavailable.
    //
    // For a truly native approach, we would need to implement the moov atom relocation
    // ourselves by parsing the MP4 box structure. For now, we fall back to CLI for this step.
    let temp_output = path.with_extension("faststart.mp4");

    let result = std::process::Command::new("ffmpeg")
        .args(["-y", "-i"])
        .arg(path)
        .args(["-c", "copy", "-movflags", "+faststart"])
        .arg(&temp_output)
        .output();

    match result {
        Ok(output) if output.status.success() => {
            // Replace original with faststart version
            std::fs::rename(&temp_output, path)?;
            Ok(())
        }
        Ok(_) | Err(_) => {
            // Clean up temp file if it exists
            let _ = std::fs::remove_file(&temp_output);
            // Non-fatal: file is still valid, just not optimized for streaming
            #[cfg(feature = "tracing")]
            tracing::warn!("Could not apply faststart optimization, file is still valid");
            Ok(())
        }
    }
}

fn remux_with_ffmpeg(input: &Path, output: &Path, container: Container) -> Result<()> {
    #[cfg(feature = "tracing")]
    tracing::debug!("Remuxing with ffmpeg CLI: {:?} -> {:?}", input, output);

    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-y", "-i"]).arg(input).args(["-c", "copy"]); // Copy all streams

    // Add container-specific options
    match container {
        Container::Mp4 => {
            cmd.args(["-movflags", "+faststart"]);
        }
        Container::Ts => {
            cmd.args(["-f", "mpegts"]);
        }
        _ => {}
    }

    cmd.arg(output);

    let result = cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Error::tool_not_found("ffmpeg")
        } else {
            Error::Io(e)
        }
    })?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(Error::tool_failed("ffmpeg", stderr.to_string()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_extension() {
        assert_eq!(Container::Mkv.extension(), "mkv");
        assert_eq!(Container::Mp4.extension(), "mp4");
        assert_eq!(Container::Ts.extension(), "ts");
    }

    #[test]
    fn test_container_from_str() {
        assert_eq!("mkv".parse::<Container>().ok(), Some(Container::Mkv));
        assert_eq!("MKV".parse::<Container>().ok(), Some(Container::Mkv));
        assert_eq!("mp4".parse::<Container>().ok(), Some(Container::Mp4));
        assert_eq!("unknown".parse::<Container>().ok(), None);
    }

    #[test]
    #[cfg(feature = "native-ffmpeg")]
    fn test_container_ffmpeg_format_name() {
        assert_eq!(Container::Mkv.ffmpeg_format_name(), "matroska");
        assert_eq!(Container::Mp4.ffmpeg_format_name(), "mp4");
        assert_eq!(Container::Ts.ffmpeg_format_name(), "mpegts");
        assert_eq!(Container::Mov.ffmpeg_format_name(), "mov");
        assert_eq!(Container::Webm.ffmpeg_format_name(), "webm");
        assert_eq!(Container::Avi.ffmpeg_format_name(), "avi");
        assert_eq!(Container::M2ts.ffmpeg_format_name(), "mpegts");
    }
}
