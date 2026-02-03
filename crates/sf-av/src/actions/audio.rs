//! Add a compatibility audio track via ffmpeg transcoding.

use crate::command::ToolCommand;
use crate::tools::ToolRegistry;
use crate::workspace::Workspace;

/// Add a compatibility audio track by transcoding from an existing track.
///
/// `source_track` is the 0-based index of the audio stream to transcode
/// (e.g. `0` for the first audio track).  `target_codec` selects the
/// output codec (commonly [`sf_core::AudioCodec::Aac`] for universal
/// playback).
///
/// The output file will contain all original streams **plus** the new
/// transcoded audio track appended after the existing audio tracks.
pub async fn add_compat_audio(
    workspace: &Workspace,
    tools: &ToolRegistry,
    source_track: usize,
    target_codec: sf_core::AudioCodec,
) -> sf_core::Result<()> {
    let input = workspace.input();
    let output = workspace.output();
    let ffmpeg = tools.require("ffmpeg")?;

    let (ffmpeg_codec, bitrate, channels): (&str, Option<&str>, Option<&str>) = match target_codec {
        sf_core::AudioCodec::Aac => ("aac", Some("256k"), Some("2")),
        sf_core::AudioCodec::Ac3 => ("ac3", Some("640k"), Some("6")),
        sf_core::AudioCodec::Eac3 => ("eac3", Some("768k"), None),
        sf_core::AudioCodec::Flac => ("flac", None, None),
        sf_core::AudioCodec::Opus => ("libopus", Some("128k"), Some("2")),
        _ => ("aac", Some("256k"), Some("2")),
    };

    tracing::info!(
        "add compat audio ({ffmpeg_codec}) from track {source_track} for {:?}",
        input
    );

    let source_map = format!("0:a:{source_track}");

    let mut cmd = ToolCommand::new(ffmpeg.path.clone());
    cmd.args(["-y", "-i"]);
    cmd.arg(input.to_string_lossy().as_ref());
    // Copy video.
    cmd.args(["-map", "0:v", "-c:v", "copy"]);
    // Copy all existing audio.
    cmd.args(["-map", "0:a", "-c:a", "copy"]);
    // Add transcoded audio track from source.
    cmd.args(["-map", &source_map]);
    // The new audio track index = number of existing audio tracks (last one).
    // Use codec on the *last* audio stream.
    cmd.args(["-c:a:1", ffmpeg_codec]);

    if let Some(br) = bitrate {
        cmd.args(["-b:a:1", br]);
    }
    if let Some(ch) = channels {
        cmd.args(["-ac:a:1", ch]);
    }

    // Copy subtitles.
    cmd.args(["-map", "0:s?", "-c:s", "copy"]);

    cmd.arg(output.to_string_lossy().as_ref());
    cmd.execute().await?;

    Ok(())
}
