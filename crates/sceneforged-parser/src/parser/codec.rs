//! Codec metadata parser.
//!
//! Extracts video encoder, video standard, audio codec, audio channels,
//! and HDR format from the token stream.

use crate::lexer::{Lexer, Token};
use crate::model::{
    AudioChannels, AudioCodec, Confidence, HdrFormat, ParsedField, ParsedRelease, VideoEncoder,
    VideoStandard,
};

/// Extract codec information from the token stream.
///
/// This parser looks for video and audio codec tokens and populates the
/// corresponding fields in the ParsedRelease.
pub fn extract(lexer: &Lexer, release: &mut ParsedRelease) {
    let tokens = lexer.tokens();
    let mut has_truehd = false;
    let mut has_atmos = false;
    let mut has_dd_plus = false;

    for (token, span) in tokens {
        match token {
            // Video codecs
            Token::CodecH265(text) => {
                if release.video_encoder.is_none() {
                    release.video_encoder = Some(ParsedField::new(
                        VideoEncoder::X265,
                        Confidence::CERTAIN,
                        (span.start, span.end),
                        *text,
                    ));
                }
                if release.video_standard.is_none() {
                    release.video_standard = Some(ParsedField::new(
                        VideoStandard::H265,
                        Confidence::CERTAIN,
                        (span.start, span.end),
                        *text,
                    ));
                }
            }
            Token::CodecH264(text) => {
                if release.video_encoder.is_none() {
                    release.video_encoder = Some(ParsedField::new(
                        VideoEncoder::X264,
                        Confidence::CERTAIN,
                        (span.start, span.end),
                        *text,
                    ));
                }
                if release.video_standard.is_none() {
                    release.video_standard = Some(ParsedField::new(
                        VideoStandard::H264,
                        Confidence::CERTAIN,
                        (span.start, span.end),
                        *text,
                    ));
                }
            }
            Token::CodecAv1(text) => {
                // AV1 doesn't have a specific encoder in VideoEncoder, use SvtAv1 as default
                if release.video_encoder.is_none() {
                    release.video_encoder = Some(ParsedField::new(
                        VideoEncoder::SvtAv1,
                        Confidence::HIGH, // Less certain which AV1 encoder
                        (span.start, span.end),
                        *text,
                    ));
                }
                if release.video_standard.is_none() {
                    release.video_standard = Some(ParsedField::new(
                        VideoStandard::Av1,
                        Confidence::CERTAIN,
                        (span.start, span.end),
                        *text,
                    ));
                }
            }
            Token::CodecMpeg4(text) => {
                // XviD/DivX MPEG-4 Part 2 codecs
                if release.video_encoder.is_none() {
                    let encoder = if text.to_uppercase().contains("DIVX") {
                        VideoEncoder::DivX
                    } else {
                        VideoEncoder::Xvid
                    };
                    release.video_encoder = Some(ParsedField::new(
                        encoder,
                        Confidence::CERTAIN,
                        (span.start, span.end),
                        *text,
                    ));
                }
                if release.video_standard.is_none() {
                    release.video_standard = Some(ParsedField::new(
                        VideoStandard::Mpeg4Part2,
                        Confidence::CERTAIN,
                        (span.start, span.end),
                        *text,
                    ));
                }
            }

            // Audio codecs
            Token::AudioDTSHD(text) => {
                if release.audio_codec.is_none() {
                    let codec = if text.to_uppercase().contains("X") {
                        AudioCodec::DtsX
                    } else {
                        AudioCodec::DtsHdMa
                    };
                    release.audio_codec = Some(ParsedField::new(
                        codec,
                        Confidence::CERTAIN,
                        (span.start, span.end),
                        *text,
                    ));
                }
            }
            Token::AudioDTS(text) => {
                if release.audio_codec.is_none() {
                    release.audio_codec = Some(ParsedField::new(
                        AudioCodec::Dts,
                        Confidence::CERTAIN,
                        (span.start, span.end),
                        *text,
                    ));
                }
            }
            Token::AudioTrueHD(text) => {
                let upper = text.to_uppercase();
                if upper.contains("TRUEHD") {
                    has_truehd = true;
                }
                if upper.contains("ATMOS") {
                    has_atmos = true;
                }
            }
            Token::AudioDDPlus(text) => {
                has_dd_plus = true;
                // Check if channel info is embedded (e.g., "DDP5.1")
                extract_embedded_channels(text, release, span);
                // Will be set later based on Atmos presence
            }
            Token::AudioDD(text) => {
                if release.audio_codec.is_none() && !has_dd_plus {
                    release.audio_codec = Some(ParsedField::new(
                        AudioCodec::Ac3,
                        Confidence::CERTAIN,
                        (span.start, span.end),
                        *text,
                    ));
                }
                // Check if channel info is embedded (e.g., "DD5.1")
                extract_embedded_channels(text, release, span);
            }
            Token::AudioFormat(text) => {
                if release.audio_codec.is_none() {
                    if let Some(codec) = parse_audio_format(text) {
                        release.audio_codec = Some(ParsedField::new(
                            codec,
                            Confidence::CERTAIN,
                            (span.start, span.end),
                            *text,
                        ));
                    }
                }
            }

            // Audio channels
            Token::AudioChannels(text) => {
                if release.audio_channels.is_none() {
                    if let Some(channels) = parse_audio_channels(text) {
                        release.audio_channels = Some(ParsedField::new(
                            channels,
                            Confidence::CERTAIN,
                            (span.start, span.end),
                            *text,
                        ));
                    }
                }
            }

            // HDR formats
            Token::HdrDolbyVision(text) => {
                if release.hdr_format.is_none() {
                    release.hdr_format = Some(ParsedField::new(
                        HdrFormat::DolbyVision,
                        Confidence::CERTAIN,
                        (span.start, span.end),
                        *text,
                    ));
                }
            }
            Token::HdrHDR10Plus(text) => {
                if release.hdr_format.is_none() {
                    release.hdr_format = Some(ParsedField::new(
                        HdrFormat::Hdr10Plus,
                        Confidence::CERTAIN,
                        (span.start, span.end),
                        *text,
                    ));
                }
            }
            Token::HdrHDR10(text) => {
                if release.hdr_format.is_none() {
                    release.hdr_format = Some(ParsedField::new(
                        HdrFormat::Hdr10,
                        Confidence::CERTAIN,
                        (span.start, span.end),
                        *text,
                    ));
                }
            }
            Token::HdrGeneric(text) => {
                if release.hdr_format.is_none() {
                    let upper = text.to_uppercase();
                    let format = if upper == "HLG" {
                        HdrFormat::Hlg
                    } else {
                        // Generic "HDR" or "PQ" defaults to HDR10
                        HdrFormat::Hdr10
                    };
                    release.hdr_format = Some(ParsedField::new(
                        format,
                        Confidence::HIGH,
                        (span.start, span.end),
                        *text,
                    ));
                }
            }

            _ => {}
        }
    }

    // Post-process audio codec based on TrueHD/Atmos/DD+ combinations
    if has_truehd && has_atmos && release.audio_codec.is_none() {
        release.audio_codec = Some(ParsedField::new(
            AudioCodec::TrueHdAtmos,
            Confidence::CERTAIN,
            (0, 0),
            "TrueHD.Atmos",
        ));
    } else if has_truehd && release.audio_codec.is_none() {
        release.audio_codec = Some(ParsedField::new(
            AudioCodec::TrueHd,
            Confidence::CERTAIN,
            (0, 0),
            "TrueHD",
        ));
    } else if has_dd_plus && has_atmos && release.audio_codec.is_none() {
        release.audio_codec = Some(ParsedField::new(
            AudioCodec::Eac3Atmos,
            Confidence::CERTAIN,
            (0, 0),
            "DD+.Atmos",
        ));
    } else if has_dd_plus && release.audio_codec.is_none() {
        release.audio_codec = Some(ParsedField::new(
            AudioCodec::Eac3,
            Confidence::CERTAIN,
            (0, 0),
            "DD+",
        ));
    }
}

/// Parse an audio format token into an AudioCodec enum.
fn parse_audio_format(text: &str) -> Option<AudioCodec> {
    let upper = text.to_uppercase();
    match upper.as_str() {
        "AAC" => Some(AudioCodec::Aac),
        "FLAC" => Some(AudioCodec::Flac),
        "MP3" => Some(AudioCodec::Mp3),
        "LPCM" | "PCM" => Some(AudioCodec::Pcm),
        _ => None,
    }
}

/// Parse an audio channels string into an AudioChannels enum.
fn parse_audio_channels(text: &str) -> Option<AudioChannels> {
    match text {
        "7.1" => Some(AudioChannels::_7_1),
        "5.1" => Some(AudioChannels::_5_1),
        "2.0" => Some(AudioChannels::_2_0),
        "1.0" => Some(AudioChannels::_1_0),
        _ => None,
    }
}

/// Extract channel configuration from combined audio codec strings like "DD5.1" or "DDP7.1".
fn extract_embedded_channels(
    text: &str,
    release: &mut ParsedRelease,
    span: &std::ops::Range<usize>,
) {
    if release.audio_channels.is_some() {
        return;
    }

    // Look for channel patterns: 7.1, 5.1, 2.0, 2.1, 1.0
    let patterns = [
        ("7.1", AudioChannels::_7_1),
        ("5.1", AudioChannels::_5_1),
        ("2.1", AudioChannels::_2_0), // Map 2.1 to 2.0 as it's not in the enum
        ("2.0", AudioChannels::_2_0),
        ("1.0", AudioChannels::_1_0),
    ];

    for (pattern, channels) in &patterns {
        if text.contains(pattern) {
            release.audio_channels = Some(ParsedField::new(
                *channels,
                Confidence::CERTAIN,
                (span.start, span.end),
                text,
            ));
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_video_codec_h264() {
        let input = "Movie.2020.1080p.BluRay.x264-GROUP";
        let lexer = Lexer::new(input);
        let mut release = ParsedRelease::new(input);

        extract(&lexer, &mut release);

        assert!(release.video_encoder.is_some());
        assert_eq!(
            **release.video_encoder.as_ref().unwrap(),
            VideoEncoder::X264
        );
        assert!(release.video_standard.is_some());
        assert_eq!(
            **release.video_standard.as_ref().unwrap(),
            VideoStandard::H264
        );
    }

    #[test]
    fn test_extract_video_codec_h265() {
        let input = "Movie.2020.2160p.BluRay.x265-GROUP";
        let lexer = Lexer::new(input);
        let mut release = ParsedRelease::new(input);

        extract(&lexer, &mut release);

        assert!(release.video_encoder.is_some());
        assert_eq!(
            **release.video_encoder.as_ref().unwrap(),
            VideoEncoder::X265
        );
        assert!(release.video_standard.is_some());
        assert_eq!(
            **release.video_standard.as_ref().unwrap(),
            VideoStandard::H265
        );
    }

    #[test]
    fn test_extract_audio_channels() {
        let input = "Movie.2020.1080p.BluRay.DTS.5.1.x264-GROUP";
        let lexer = Lexer::new(input);
        let mut release = ParsedRelease::new(input);

        extract(&lexer, &mut release);

        assert!(release.audio_channels.is_some());
        assert_eq!(
            **release.audio_channels.as_ref().unwrap(),
            AudioChannels::_5_1
        );
    }

    #[test]
    fn test_extract_hdr_format() {
        let input = "Movie.2020.2160p.BluRay.HDR10.x265-GROUP";
        let lexer = Lexer::new(input);
        let mut release = ParsedRelease::new(input);

        extract(&lexer, &mut release);

        assert!(release.hdr_format.is_some());
        assert_eq!(**release.hdr_format.as_ref().unwrap(), HdrFormat::Hdr10);
    }
}
