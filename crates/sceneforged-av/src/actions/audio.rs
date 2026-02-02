//! Audio track manipulation.

use crate::{Error, Result, Workspace};
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

/// Target audio codecs for compatibility transcoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioCodec {
    /// AAC (Advanced Audio Coding) - widely compatible
    Aac,
    /// AC-3 (Dolby Digital) - good for 5.1 surround
    Ac3,
    /// E-AC-3 (Dolby Digital Plus) - improved AC-3
    Eac3,
    /// FLAC (Free Lossless Audio Codec)
    Flac,
    /// Opus - modern, efficient codec
    Opus,
}

impl AudioCodec {
    /// Get the ffmpeg codec name.
    pub fn ffmpeg_name(&self) -> &'static str {
        match self {
            AudioCodec::Aac => "aac",
            AudioCodec::Ac3 => "ac3",
            AudioCodec::Eac3 => "eac3",
            AudioCodec::Flac => "flac",
            AudioCodec::Opus => "libopus",
        }
    }

    /// Get recommended bitrate for this codec.
    pub fn default_bitrate(&self) -> &'static str {
        match self {
            AudioCodec::Aac => "256k",
            AudioCodec::Ac3 => "640k",
            AudioCodec::Eac3 => "768k",
            AudioCodec::Flac => "", // Lossless, no bitrate
            AudioCodec::Opus => "128k",
        }
    }

    /// Get recommended channel count.
    pub fn default_channels(&self) -> Option<u8> {
        match self {
            AudioCodec::Aac => Some(2),  // Stereo for compatibility
            AudioCodec::Ac3 => Some(6),  // 5.1 surround
            AudioCodec::Eac3 => None,    // Keep original
            AudioCodec::Flac => None,    // Keep original
            AudioCodec::Opus => Some(2), // Stereo for compatibility
        }
    }

    /// Get the FFmpeg codec ID for native transcoding.
    #[cfg(feature = "native-ffmpeg")]
    pub fn codec_id(&self) -> ffmpeg::codec::Id {
        match self {
            AudioCodec::Aac => ffmpeg::codec::Id::AAC,
            AudioCodec::Ac3 => ffmpeg::codec::Id::AC3,
            AudioCodec::Eac3 => ffmpeg::codec::Id::EAC3,
            AudioCodec::Flac => ffmpeg::codec::Id::FLAC,
            AudioCodec::Opus => ffmpeg::codec::Id::OPUS,
        }
    }

    /// Get default bitrate in bits per second for native transcoding.
    #[cfg(feature = "native-ffmpeg")]
    pub fn default_bitrate_bps(&self) -> Option<usize> {
        match self {
            AudioCodec::Aac => Some(256_000),
            AudioCodec::Ac3 => Some(640_000),
            AudioCodec::Eac3 => Some(768_000),
            AudioCodec::Flac => None, // Lossless, no bitrate
            AudioCodec::Opus => Some(128_000),
        }
    }
}

/// Add a compatibility audio track by transcoding from an existing track using native FFmpeg.
///
/// This uses the `ffmpeg-the-third` bindings for direct FFmpeg library access,
/// avoiding subprocess spawning and JSON parsing overhead.
#[cfg(feature = "native-ffmpeg")]
pub fn transcode_audio_native(
    workspace: &Workspace,
    _source_codec: &str,
    target: AudioCodec,
) -> Result<()> {
    use ffmpeg::{codec, encoder, format, frame};

    init_ffmpeg();

    let input_path = workspace.input();
    let output_path = workspace.output();

    #[cfg(feature = "tracing")]
    tracing::info!(
        "Native audio transcode: adding {} track for {:?}",
        target.ffmpeg_name(),
        input_path
    );

    // Open input file
    let mut ictx = format::input(input_path).map_err(|e| {
        if e.to_string().contains("No such file") {
            Error::file_not_found(input_path)
        } else {
            Error::tool_failed("ffmpeg-native", format!("Failed to open input: {}", e))
        }
    })?;

    // Create output context
    let mut octx = format::output(output_path).map_err(|e| {
        Error::tool_failed("ffmpeg-native", format!("Failed to create output: {}", e))
    })?;

    // Track mapping: input stream index -> output stream index
    let mut stream_mapping: Vec<Option<usize>> = vec![None; ictx.nb_streams() as usize];
    let mut output_stream_index = 0usize;

    // First pass: add all existing streams as copy
    for (i, stream) in ictx.streams().enumerate() {
        let params = stream.parameters();

        #[cfg(feature = "tracing")]
        let codec_type = params.medium();

        // Add stream to output
        let mut out_stream = octx
            .add_stream(codec::encoder::find(codec::Id::None))
            .map_err(|e| {
                Error::tool_failed("ffmpeg-native", format!("Failed to add stream: {}", e))
            })?;

        out_stream.set_parameters(params);

        // Fix time base for the output stream
        unsafe {
            (*out_stream.as_mut_ptr()).time_base = (*stream.as_ptr()).time_base;
        }

        stream_mapping[i] = Some(output_stream_index);
        output_stream_index += 1;

        #[cfg(feature = "tracing")]
        tracing::debug!(
            "Mapped input stream {} ({:?}) to output stream {}",
            i,
            codec_type,
            output_stream_index - 1
        );
    }

    // Find the first audio stream for transcoding
    let audio_stream_info = ictx
        .streams()
        .enumerate()
        .find(|(_, s)| s.parameters().medium() == ffmpeg::media::Type::Audio);

    let (source_audio_idx, source_audio_stream) = audio_stream_info
        .ok_or_else(|| Error::InvalidInput("No audio stream found in input file".to_string()))?;

    // Get source audio parameters
    let source_params = source_audio_stream.parameters();
    let source_codec_ctx =
        codec::context::Context::from_parameters(source_params).map_err(|e| {
            Error::tool_failed(
                "ffmpeg-native",
                format!("Failed to get source codec context: {}", e),
            )
        })?;

    let mut decoder = source_codec_ctx.decoder().audio().map_err(|e| {
        Error::tool_failed(
            "ffmpeg-native",
            format!("Failed to create audio decoder: {}", e),
        )
    })?;

    // Find encoder for target codec
    let encoder_codec = encoder::find(target.codec_id()).ok_or_else(|| {
        Error::Unsupported(format!(
            "Encoder not available for codec: {}",
            target.ffmpeg_name()
        ))
    })?;

    // Add new audio stream for transcoded audio
    let mut transcoded_stream = octx.add_stream(encoder_codec).map_err(|e| {
        Error::tool_failed(
            "ffmpeg-native",
            format!("Failed to add transcoded audio stream: {}", e),
        )
    })?;

    let transcoded_stream_index = output_stream_index;

    // Set up encoder context
    let encoder_ctx = codec::context::Context::new_with_codec(encoder_codec);
    let mut audio_encoder = encoder_ctx.encoder().audio().map_err(|e| {
        Error::tool_failed(
            "ffmpeg-native",
            format!("Failed to create audio encoder: {}", e),
        )
    })?;

    // Configure encoder parameters
    let decoder_channels = decoder.ch_layout().channels();
    let target_channels = target.default_channels().unwrap_or(decoder_channels as u8);

    // FFmpeg 7+ uses ch_layout API - use static layouts to avoid lifetime issues
    let target_channel_layout = match target_channels {
        1 => ffmpeg::channel_layout::ChannelLayout::MONO,
        2 => ffmpeg::channel_layout::ChannelLayout::STEREO,
        6 => ffmpeg::channel_layout::ChannelLayout::_5POINT1,
        8 => ffmpeg::channel_layout::ChannelLayout::_7POINT1,
        // For other channel counts, use stereo as fallback
        _ => ffmpeg::channel_layout::ChannelLayout::STEREO,
    };

    audio_encoder.set_ch_layout(target_channel_layout.clone());

    let sample_rate = decoder.rate() as i32;
    audio_encoder.set_rate(sample_rate);

    // Set format - prefer the first supported format by the encoder
    let target_format = if let Some(audio) = encoder_codec.audio() {
        audio
            .formats()
            .and_then(|mut f| f.next())
            .unwrap_or(ffmpeg::format::Sample::F32(
                ffmpeg::format::sample::Type::Packed,
            ))
    } else {
        ffmpeg::format::Sample::F32(ffmpeg::format::sample::Type::Packed)
    };
    audio_encoder.set_format(target_format);

    // Set bitrate if applicable
    if let Some(bitrate) = target.default_bitrate_bps() {
        audio_encoder.set_bit_rate(bitrate);
    }

    // Set time base
    audio_encoder.set_time_base(ffmpeg::Rational::new(1, sample_rate));

    // Open encoder - this consumes audio_encoder and returns an opened encoder
    let mut opened_encoder = audio_encoder.open().map_err(|e| {
        Error::tool_failed("ffmpeg-native", format!("Failed to open encoder: {}", e))
    })?;

    // Set transcoded stream parameters from encoder
    // In v4, we use avcodec_parameters_from_context to transfer encoder settings to stream
    unsafe {
        let stream_params = (*transcoded_stream.as_mut_ptr()).codecpar;
        let ret =
            ffmpeg::ffi::avcodec_parameters_from_context(stream_params, opened_encoder.as_ptr());
        if ret < 0 {
            return Err(Error::tool_failed(
                "ffmpeg-native",
                "Failed to copy codec parameters from encoder to stream",
            ));
        }
    }

    #[cfg(feature = "tracing")]
    tracing::debug!(
        "Added transcoded audio stream {} with codec {}",
        transcoded_stream_index,
        target.ffmpeg_name()
    );

    // Write output header
    octx.write_header().map_err(|e| {
        Error::tool_failed(
            "ffmpeg-native",
            format!("Failed to write output header: {}", e),
        )
    })?;

    // Process packets
    let source_time_base = source_audio_stream.time_base();
    let output_time_base = octx.stream(transcoded_stream_index).unwrap().time_base();

    // Resampler for format conversion if needed
    let mut resampler: Option<ffmpeg::software::resampling::Context> = None;

    // Get encoder rate for resampler setup
    let encoder_rate = opened_encoder.rate();

    for result in ictx.packets() {
        let (stream, mut packet) = result?;
        let stream_index = stream.index();

        if let Some(out_idx) = stream_mapping.get(stream_index).and_then(|&x| x) {
            // Copy non-audio streams or non-source audio streams directly
            if stream_index != source_audio_idx {
                let out_stream = octx.stream(out_idx).unwrap();
                packet.rescale_ts(stream.time_base(), out_stream.time_base());
                packet.set_stream(out_idx);
                packet.set_position(-1);
                packet.write_interleaved(&mut octx).map_err(|e| {
                    Error::tool_failed("ffmpeg-native", format!("Failed to write packet: {}", e))
                })?;
            }
        }

        // Handle source audio stream - copy and transcode
        if stream_index == source_audio_idx {
            // First, copy the original audio
            if let Some(out_idx) = stream_mapping.get(stream_index).and_then(|&x| x) {
                let out_stream = octx.stream(out_idx).unwrap();
                let mut copy_packet = packet.clone();
                copy_packet.rescale_ts(stream.time_base(), out_stream.time_base());
                copy_packet.set_stream(out_idx);
                copy_packet.set_position(-1);
                copy_packet.write_interleaved(&mut octx).map_err(|e| {
                    Error::tool_failed(
                        "ffmpeg-native",
                        format!("Failed to write copied audio packet: {}", e),
                    )
                })?;
            }

            // Then, decode and transcode
            decoder.send_packet(&packet).map_err(|e| {
                Error::tool_failed(
                    "ffmpeg-native",
                    format!("Failed to send packet to decoder: {}", e),
                )
            })?;

            let mut decoded_frame = frame::Audio::empty();
            while decoder.receive_frame(&mut decoded_frame).is_ok() {
                // Set up resampler if needed (lazy initialization)
                let resampled_frame = if decoder.format() != target_format
                    || decoder.rate() != encoder_rate
                    || decoder.ch_layout().channels() != target_channels as u32
                {
                    if resampler.is_none() {
                        // FFmpeg 7+ uses ch_layout API
                        let src_layout = decoder.ch_layout();

                        // In v4, use Context::get2() which takes the same parameters
                        resampler = Some(
                            ffmpeg::software::resampling::Context::get2(
                                decoder.format(),
                                src_layout,
                                decoder.rate(),
                                target_format,
                                target_channel_layout.clone(),
                                encoder_rate,
                            )
                            .map_err(|e| {
                                Error::tool_failed(
                                    "ffmpeg-native",
                                    format!("Failed to create resampler: {}", e),
                                )
                            })?,
                        );
                    }

                    let resampler_ctx = resampler.as_mut().unwrap();
                    let mut resampled = frame::Audio::empty();
                    resampler_ctx
                        .run(&decoded_frame, &mut resampled)
                        .map_err(|e| {
                            Error::tool_failed(
                                "ffmpeg-native",
                                format!("Failed to resample audio: {}", e),
                            )
                        })?;
                    resampled.set_pts(decoded_frame.pts());
                    resampled
                } else {
                    decoded_frame.clone()
                };

                // Encode the frame
                opened_encoder.send_frame(&resampled_frame).map_err(|e| {
                    Error::tool_failed(
                        "ffmpeg-native",
                        format!("Failed to send frame to encoder: {}", e),
                    )
                })?;

                let mut encoded_packet = ffmpeg::Packet::empty();
                while opened_encoder.receive_packet(&mut encoded_packet).is_ok() {
                    encoded_packet.rescale_ts(source_time_base, output_time_base);
                    encoded_packet.set_stream(transcoded_stream_index);
                    encoded_packet.set_position(-1);
                    encoded_packet.write_interleaved(&mut octx).map_err(|e| {
                        Error::tool_failed(
                            "ffmpeg-native",
                            format!("Failed to write encoded packet: {}", e),
                        )
                    })?;
                }
            }
        }
    }

    // Flush decoder
    decoder.send_eof().ok();
    let mut decoded_frame = frame::Audio::empty();
    while decoder.receive_frame(&mut decoded_frame).is_ok() {
        let resampled_frame = if let Some(ref mut resampler_ctx) = resampler {
            let mut resampled = frame::Audio::empty();
            resampler_ctx.run(&decoded_frame, &mut resampled).ok();
            resampled.set_pts(decoded_frame.pts());
            resampled
        } else {
            decoded_frame.clone()
        };

        opened_encoder.send_frame(&resampled_frame).ok();
        let mut encoded_packet = ffmpeg::Packet::empty();
        while opened_encoder.receive_packet(&mut encoded_packet).is_ok() {
            encoded_packet.rescale_ts(source_time_base, output_time_base);
            encoded_packet.set_stream(transcoded_stream_index);
            encoded_packet.set_position(-1);
            encoded_packet.write_interleaved(&mut octx).ok();
        }
    }

    // Flush encoder
    opened_encoder.send_eof().ok();
    let mut encoded_packet = ffmpeg::Packet::empty();
    while opened_encoder.receive_packet(&mut encoded_packet).is_ok() {
        encoded_packet.rescale_ts(source_time_base, output_time_base);
        encoded_packet.set_stream(transcoded_stream_index);
        encoded_packet.set_position(-1);
        encoded_packet.write_interleaved(&mut octx).ok();
    }

    // Write trailer
    octx.write_trailer().map_err(|e| {
        Error::tool_failed(
            "ffmpeg-native",
            format!("Failed to write output trailer: {}", e),
        )
    })?;

    #[cfg(feature = "tracing")]
    tracing::info!("Native audio transcode completed successfully");

    Ok(())
}

/// Add a compatibility audio track by transcoding from an existing track using CLI fallback.
fn add_compat_audio_cli(
    workspace: &Workspace,
    _source_codec: &str,
    target: AudioCodec,
) -> Result<()> {
    let input = workspace.input();
    let output = workspace.output();

    #[cfg(feature = "tracing")]
    tracing::info!(
        "Adding compatibility audio track (CLI): {} for {:?}",
        target.ffmpeg_name(),
        input
    );

    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-y", "-i"])
        .arg(input)
        // Copy video stream
        .args(["-map", "0:v", "-c:v", "copy"])
        // Copy all existing audio streams
        .args(["-map", "0:a", "-c:a", "copy"])
        // Add transcoded audio track from first audio stream
        .args(["-map", "0:a:0"]);

    // Add codec-specific arguments
    cmd.args(["-c:a:1", target.ffmpeg_name()]);

    let bitrate = target.default_bitrate();
    if !bitrate.is_empty() {
        cmd.args(["-b:a:1", bitrate]);
    }

    if let Some(channels) = target.default_channels() {
        cmd.args(["-ac:a:1", &channels.to_string()]);
    }

    // Copy all subtitle streams
    cmd.args(["-map", "0:s?", "-c:s", "copy"]);

    cmd.arg(output);

    #[cfg(feature = "tracing")]
    tracing::debug!("Running ffmpeg CLI for audio transcode");

    let result = cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            Error::tool_not_found("ffmpeg")
        } else {
            Error::Io(e)
        }
    })?;

    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(Error::tool_failed(
            "ffmpeg",
            format!("Failed to add compatibility audio: {}", stderr),
        ));
    }

    #[cfg(feature = "tracing")]
    tracing::info!("Compatibility audio track added successfully (CLI)");

    Ok(())
}

/// Add a compatibility audio track by transcoding from an existing track.
///
/// Common use case: Add AAC stereo track from TrueHD/DTS-HD for device compatibility.
///
/// When the `native-ffmpeg` feature is enabled, this uses direct FFmpeg library bindings
/// for better performance. Falls back to CLI if native transcoding fails.
pub fn add_compat_audio(
    workspace: &Workspace,
    source_codec: &str,
    target: AudioCodec,
) -> Result<()> {
    #[cfg(feature = "native-ffmpeg")]
    {
        #[cfg(feature = "tracing")]
        tracing::debug!("Attempting native FFmpeg audio transcode");

        match transcode_audio_native(workspace, source_codec, target) {
            Ok(()) => return Ok(()),
            Err(e) => {
                #[cfg(feature = "tracing")]
                tracing::warn!("Native audio transcode failed, falling back to CLI: {}", e);
            }
        }
    }

    // Fallback to CLI
    add_compat_audio_cli(workspace, source_codec, target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_codec_ffmpeg_name() {
        assert_eq!(AudioCodec::Aac.ffmpeg_name(), "aac");
        assert_eq!(AudioCodec::Ac3.ffmpeg_name(), "ac3");
        assert_eq!(AudioCodec::Flac.ffmpeg_name(), "flac");
    }

    #[test]
    fn test_audio_codec_default_bitrate() {
        assert_eq!(AudioCodec::Aac.default_bitrate(), "256k");
        assert_eq!(AudioCodec::Ac3.default_bitrate(), "640k");
        assert_eq!(AudioCodec::Flac.default_bitrate(), "");
    }
}
