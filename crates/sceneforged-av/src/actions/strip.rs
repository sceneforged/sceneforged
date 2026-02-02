//! Track stripping operations.

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

/// Configuration for track stripping.
#[derive(Debug, Clone, Default)]
pub struct StripConfig {
    /// Strip all audio tracks.
    pub strip_audio: bool,
    /// Strip all subtitle tracks.
    pub strip_subtitles: bool,
    /// Languages to strip (e.g., ["spa", "fre"]).
    pub strip_languages: Vec<String>,
    /// Languages to keep (inverted logic - strip everything else).
    pub keep_languages: Vec<String>,
}

impl StripConfig {
    /// Create a new empty strip configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Strip all audio tracks.
    pub fn strip_audio(mut self) -> Self {
        self.strip_audio = true;
        self
    }

    /// Strip all subtitle tracks.
    pub fn strip_subtitles(mut self) -> Self {
        self.strip_subtitles = true;
        self
    }

    /// Strip specific languages.
    pub fn strip_langs(mut self, languages: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.strip_languages = languages.into_iter().map(Into::into).collect();
        self
    }

    /// Keep only specific languages.
    pub fn keep_langs(mut self, languages: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.keep_languages = languages.into_iter().map(Into::into).collect();
        self
    }
}

/// Strip unwanted tracks from a media file.
///
/// Uses mkvmerge for MKV files (more precise control) and ffmpeg for others.
pub fn strip_tracks(workspace: &Workspace, config: &StripConfig) -> Result<()> {
    let input = workspace.input();

    #[cfg(feature = "tracing")]
    tracing::info!("Stripping tracks from {:?}", input);

    if is_mkv(input) {
        strip_tracks_mkvmerge(workspace, config)
    } else {
        strip_tracks_ffmpeg(workspace, config)
    }
}

fn strip_tracks_mkvmerge(workspace: &Workspace, config: &StripConfig) -> Result<()> {
    let input = workspace.input();
    let output = workspace.output();

    let mut cmd = Command::new("mkvmerge");
    cmd.arg("-o").arg(output);

    // Strip audio tracks
    if config.strip_audio {
        cmd.arg("--no-audio");
    }

    // Strip subtitle tracks
    if config.strip_subtitles {
        cmd.arg("--no-subtitles");
    }

    // Handle language filtering
    if !config.keep_languages.is_empty() {
        let langs = config.keep_languages.join(",");
        cmd.args(["--audio-tracks", &langs]);
        cmd.args(["--subtitle-tracks", &langs]);
    } else if !config.strip_languages.is_empty() {
        // Keep tracks NOT in the specified languages
        let langs = format!("!{}", config.strip_languages.join(",!"));
        cmd.args(["--audio-tracks", &langs]);
        cmd.args(["--subtitle-tracks", &langs]);
    }

    cmd.arg(input);

    #[cfg(feature = "tracing")]
    tracing::debug!("Running mkvmerge for track stripping");

    let result = cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Error::tool_not_found("mkvmerge")
        } else {
            Error::Io(e)
        }
    })?;

    // mkvmerge returns 0 for success, 1 for warnings
    if !result.status.success() && result.status.code() != Some(1) {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(Error::tool_failed("mkvmerge", stderr.to_string()));
    }

    #[cfg(feature = "tracing")]
    tracing::info!("Track stripping complete");

    Ok(())
}

/// Strip tracks from non-MKV files using native FFmpeg bindings.
///
/// This uses ffmpeg-the-third to selectively copy streams without re-encoding.
#[cfg(feature = "native-ffmpeg")]
fn strip_tracks_native(workspace: &Workspace, config: &StripConfig) -> Result<()> {
    init_ffmpeg();

    let input_path = workspace.input();
    let output_path = workspace.output();

    #[cfg(feature = "tracing")]
    tracing::debug!("Opening input file with native FFmpeg: {:?}", input_path);

    // Open input file
    let mut input_ctx = ffmpeg::format::input(input_path).map_err(|e| {
        if e.to_string().contains("No such file") {
            Error::file_not_found(input_path)
        } else {
            Error::tool_failed("ffmpeg", format!("Failed to open input: {}", e))
        }
    })?;

    // Create output context
    let mut output_ctx = ffmpeg::format::output(output_path)
        .map_err(|e| Error::tool_failed("ffmpeg", format!("Failed to create output: {}", e)))?;

    // Build stream mapping: (input_stream_index -> output_stream_index)
    let mut stream_mapping: Vec<Option<usize>> = vec![None; input_ctx.nb_streams() as usize];
    let mut output_stream_count = 0usize;

    // Iterate over input streams and decide which to keep
    for (input_idx, input_stream) in input_ctx.streams().enumerate() {
        let codec_params = input_stream.parameters();
        let codec_ctx =
            ffmpeg::codec::context::Context::from_parameters(codec_params).map_err(|e| {
                Error::tool_failed("ffmpeg", format!("Failed to get codec context: {}", e))
            })?;

        let medium = codec_ctx.medium();
        let should_keep = match medium {
            ffmpeg::media::Type::Video => {
                // Always keep video streams
                true
            }
            ffmpeg::media::Type::Audio => {
                if config.strip_audio {
                    false
                } else {
                    should_keep_track_by_language(
                        input_stream.metadata().get("language"),
                        &config.keep_languages,
                        &config.strip_languages,
                    )
                }
            }
            ffmpeg::media::Type::Subtitle => {
                if config.strip_subtitles {
                    false
                } else {
                    should_keep_track_by_language(
                        input_stream.metadata().get("language"),
                        &config.keep_languages,
                        &config.strip_languages,
                    )
                }
            }
            _ => {
                // Keep other streams (attachments, data, etc.)
                true
            }
        };

        if should_keep {
            // Add output stream
            let mut output_stream = output_ctx
                .add_stream(ffmpeg::encoder::find(ffmpeg::codec::Id::None))
                .map_err(|e| {
                    Error::tool_failed("ffmpeg", format!("Failed to add output stream: {}", e))
                })?;
            output_stream.set_parameters(input_stream.parameters());

            // Disable encoding - we're copying
            unsafe {
                (*(*output_stream.as_mut_ptr()).codecpar).codec_tag = 0;
            }

            stream_mapping[input_idx] = Some(output_stream_count);
            output_stream_count += 1;

            #[cfg(feature = "tracing")]
            tracing::trace!(
                "Keeping stream {} ({:?}), mapped to output stream {}",
                input_idx,
                medium,
                output_stream_count - 1
            );
        } else {
            #[cfg(feature = "tracing")]
            tracing::trace!("Stripping stream {} ({:?})", input_idx, medium);
        }
    }

    // Write output header
    output_ctx.write_header().map_err(|e| {
        Error::tool_failed("ffmpeg", format!("Failed to write output header: {}", e))
    })?;

    // Copy packets from input to output
    for result in input_ctx.packets() {
        let (stream, packet) = result?;
        let input_idx = stream.index();

        if let Some(output_idx) = stream_mapping.get(input_idx).and_then(|&x| x) {
            let mut packet = packet;

            // Get time bases for rescaling
            let input_time_base = stream.time_base();
            let output_stream = output_ctx.stream(output_idx).ok_or_else(|| {
                Error::tool_failed("ffmpeg", "Output stream not found".to_string())
            })?;
            let output_time_base = output_stream.time_base();

            // Rescale timestamps
            packet.rescale_ts(input_time_base, output_time_base);
            packet.set_stream(output_idx);
            packet.set_position(-1);

            // Write packet
            packet.write_interleaved(&mut output_ctx).map_err(|e| {
                Error::tool_failed("ffmpeg", format!("Failed to write packet: {}", e))
            })?;
        }
    }

    // Write trailer
    output_ctx.write_trailer().map_err(|e| {
        Error::tool_failed("ffmpeg", format!("Failed to write output trailer: {}", e))
    })?;

    #[cfg(feature = "tracing")]
    tracing::info!("Native FFmpeg track stripping complete");

    Ok(())
}

/// Determine if a track should be kept based on language filters.
#[cfg(feature = "native-ffmpeg")]
fn should_keep_track_by_language(
    track_language: Option<&str>,
    keep_languages: &[String],
    strip_languages: &[String],
) -> bool {
    // If keep_languages is specified, only keep tracks with matching languages
    if !keep_languages.is_empty() {
        return track_language
            .map(|lang| keep_languages.iter().any(|k| k.eq_ignore_ascii_case(lang)))
            .unwrap_or(false); // Strip tracks without language tag when filtering by keep_languages
    }

    // If strip_languages is specified, strip tracks with matching languages
    if !strip_languages.is_empty() {
        if let Some(lang) = track_language {
            return !strip_languages.iter().any(|s| s.eq_ignore_ascii_case(lang));
        }
    }

    // By default, keep the track
    true
}

fn strip_tracks_ffmpeg_cli(workspace: &Workspace, config: &StripConfig) -> Result<()> {
    let input = workspace.input();
    let output = workspace.output();

    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-y", "-i"])
        .arg(input)
        // Always copy video
        .args(["-map", "0:v", "-c:v", "copy"]);

    // Add audio if not stripping
    if !config.strip_audio {
        cmd.args(["-map", "0:a", "-c:a", "copy"]);
    }

    // Add subtitles if not stripping
    if !config.strip_subtitles {
        cmd.args(["-map", "0:s?", "-c:s", "copy"]);
    }

    cmd.arg(output);

    #[cfg(feature = "tracing")]
    tracing::debug!("Running ffmpeg CLI for track stripping");

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

/// Strip tracks from non-MKV files.
///
/// Tries native FFmpeg bindings first (if enabled), falls back to CLI.
#[cfg(feature = "native-ffmpeg")]
fn strip_tracks_ffmpeg(workspace: &Workspace, config: &StripConfig) -> Result<()> {
    match strip_tracks_native(workspace, config) {
        Ok(()) => Ok(()),
        Err(e) => {
            #[cfg(feature = "tracing")]
            tracing::warn!(
                "Native FFmpeg track stripping failed, falling back to CLI: {}",
                e
            );
            let _ = e; // Suppress unused warning when tracing is disabled
            strip_tracks_ffmpeg_cli(workspace, config)
        }
    }
}

/// Strip tracks from non-MKV files using ffmpeg CLI.
#[cfg(not(feature = "native-ffmpeg"))]
fn strip_tracks_ffmpeg(workspace: &Workspace, config: &StripConfig) -> Result<()> {
    strip_tracks_ffmpeg_cli(workspace, config)
}

/// Check if the input is an MKV file.
fn is_mkv(path: &Path) -> bool {
    path.extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("mkv"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_is_mkv() {
        assert!(is_mkv(&PathBuf::from("movie.mkv")));
        assert!(is_mkv(&PathBuf::from("movie.MKV")));
        assert!(!is_mkv(&PathBuf::from("movie.mp4")));
        assert!(!is_mkv(&PathBuf::from("movie")));
    }

    #[test]
    fn test_strip_config_builder() {
        let config = StripConfig::new()
            .strip_audio()
            .strip_subtitles()
            .keep_langs(["eng", "jpn"]);

        assert!(config.strip_audio);
        assert!(config.strip_subtitles);
        assert_eq!(config.keep_languages, vec!["eng", "jpn"]);
    }
}
