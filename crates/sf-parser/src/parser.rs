//! Core parsing logic for media release filenames.
//!
//! The parser operates in three phases:
//! 1. Tokenize the input using the Logos lexer.
//! 2. Scan tokens to identify the release group and all known metadata.
//! 3. Extract the title from the remaining leading text.

use crate::tokenizer::{tokenize, SpannedToken, Token};
use crate::types::ParsedRelease;

/// Parse a release name string into a [`ParsedRelease`].
///
/// This is the main entry point for the parsing logic. It tokenizes the
/// input, then extracts metadata fields in a single left-to-right pass.
pub fn parse(input: &str) -> ParsedRelease {
    let tokens = tokenize(input);

    if tokens.is_empty() {
        return ParsedRelease::new(clean_title(input));
    }

    let mut release = ParsedRelease::new(String::new());

    // Phase 1: Extract release group (text after the last hyphen that
    // doesn't match a known keyword).
    extract_group(&tokens, &mut release);

    // Phase 2: Scan tokens left-to-right and populate metadata fields.
    extract_metadata(&tokens, &mut release);

    // Phase 3: Build the title from tokens before the first recognized
    // keyword or year.
    extract_title(&tokens, input, &mut release);

    release
}

// -------------------------------------------------------------------------
// Release group extraction
// -------------------------------------------------------------------------

/// Find the release group -- typically the word after the last hyphen.
///
/// A trailing word is considered the group only if it is not itself a
/// recognized keyword token.
fn extract_group(tokens: &[SpannedToken<'_>], release: &mut ParsedRelease) {
    // Walk backwards to find the last Hyphen followed by a Word/Number.
    let mut i = tokens.len();
    while i > 0 {
        i -= 1;
        if matches!(tokens[i].token, Token::Hyphen) {
            // Collect everything after this hyphen that looks like the group name.
            if i + 1 < tokens.len() {
                // The group is the next word if it is not a known keyword.
                let candidate = &tokens[i + 1];
                if is_plain_word(&candidate.token) && !is_keyword(&candidate.token) {
                    // Make sure nothing significant follows (only dots/containers).
                    let remaining_significant = tokens[i + 2..]
                        .iter()
                        .any(|t| !is_ignorable_after_group(&t.token));

                    if !remaining_significant {
                        if let Some(name) = token_text(&candidate.token) {
                            release.group = Some(name.to_string());
                            return;
                        }
                    }
                }
            }
        }
    }
}

// -------------------------------------------------------------------------
// Metadata extraction
// -------------------------------------------------------------------------

/// Scan all tokens and populate the metadata fields of `release`.
fn extract_metadata(tokens: &[SpannedToken<'_>], release: &mut ParsedRelease) {
    let mut has_truehd = false;
    let mut has_atmos = false;
    let mut has_eac3 = false;

    for st in tokens {
        match &st.token {
            // Year
            Token::Year(text) => {
                if release.year.is_none() {
                    if let Ok(y) = text.parse::<u32>() {
                        release.year = Some(y);
                    }
                }
            }

            // Season / Episode
            Token::SeasonEpisode(text) => {
                if release.season.is_none() {
                    parse_season_episode(text, release);
                }
            }

            // Resolution
            Token::Resolution(text) => {
                if release.resolution.is_none() {
                    release.resolution = Some(normalize_resolution(text));
                }
            }

            // Source
            Token::SourceBluRay(_) => set_if_none(&mut release.source, "BluRay"),
            Token::SourceWebDL(_) => set_if_none(&mut release.source, "WEB-DL"),
            Token::SourceWebRip(_) => set_if_none(&mut release.source, "WEB"),
            Token::SourceWeb(_) => set_if_none(&mut release.source, "WEB"),
            Token::SourceHDTV(_) => set_if_none(&mut release.source, "HDTV"),
            Token::SourceDVDRip(_) => set_if_none(&mut release.source, "DVDRip"),
            Token::SourceRemux(_) => set_if_none(&mut release.source, "Remux"),

            // Video codec
            Token::CodecX264(_) => set_if_none(&mut release.video_codec, "x264"),
            Token::CodecX265(_) => set_if_none(&mut release.video_codec, "x265"),
            Token::CodecH264(_) => set_if_none(&mut release.video_codec, "H.264"),
            Token::CodecH265(_) => set_if_none(&mut release.video_codec, "H.265"),
            Token::CodecAV1(_) => set_if_none(&mut release.video_codec, "AV1"),
            Token::CodecVP9(_) => set_if_none(&mut release.video_codec, "VP9"),
            Token::CodecMPEG2(_) => set_if_none(&mut release.video_codec, "MPEG2"),
            Token::CodecXviD(_) => set_if_none(&mut release.video_codec, "XviD"),
            Token::CodecDivX(_) => set_if_none(&mut release.video_codec, "DivX"),

            // Audio codec -- compound codecs (TrueHD + Atmos, EAC3 + Atmos)
            // are resolved in a post-pass after the loop.
            Token::AudioDTSHD(text) => {
                if release.audio_codec.is_none() {
                    let upper = text.to_uppercase();
                    if upper.contains("MA") {
                        release.audio_codec = Some("DTS-HD".to_string());
                    } else {
                        release.audio_codec = Some("DTS-HD".to_string());
                    }
                }
            }
            Token::AudioTrueHD(_) => {
                has_truehd = true;
            }
            Token::AudioAtmos(_) => {
                has_atmos = true;
            }
            Token::AudioEAC3(_) => {
                has_eac3 = true;
            }
            Token::AudioAC3(_) | Token::AudioDD51(_) => {
                set_if_none(&mut release.audio_codec, "AC3");
            }
            Token::AudioDTS(_) => set_if_none(&mut release.audio_codec, "DTS"),
            Token::AudioAAC(_) => set_if_none(&mut release.audio_codec, "AAC"),
            Token::AudioFLAC(_) => set_if_none(&mut release.audio_codec, "FLAC"),
            Token::AudioOpus(_) => set_if_none(&mut release.audio_codec, "Opus"),

            // HDR
            Token::HdrHDR10Plus(_) => set_if_none(&mut release.hdr, "HDR10+"),
            Token::HdrHDR10(_) => set_if_none(&mut release.hdr, "HDR10"),
            Token::HdrGeneric(_) => set_if_none(&mut release.hdr, "HDR"),
            Token::HdrDolbyVision(text) => {
                let upper = text.to_uppercase();
                if upper == "DV" || upper == "DOVI" {
                    set_if_none(&mut release.hdr, "DV");
                } else {
                    set_if_none(&mut release.hdr, "DoVi");
                }
            }
            Token::HdrHLG(_) => set_if_none(&mut release.hdr, "HLG"),

            // Edition
            Token::EditionDirectorsCut(_) => {
                set_if_none(&mut release.edition, "Director's Cut");
            }
            Token::EditionExtended(_) => set_if_none(&mut release.edition, "Extended"),
            Token::EditionUnrated(_) => set_if_none(&mut release.edition, "Unrated"),
            Token::EditionRemastered(_) => set_if_none(&mut release.edition, "Remastered"),
            Token::EditionIMAX(_) => set_if_none(&mut release.edition, "IMAX"),
            Token::EditionTheatrical(_) => set_if_none(&mut release.edition, "Theatrical"),
            Token::EditionSpecial(_) => {
                set_if_none(&mut release.edition, "Special Edition");
            }

            // Revision
            Token::Proper(_) => {
                if release.revision.is_none() {
                    release.revision = Some(1);
                }
            }
            Token::Repack(_) => {
                if release.revision.is_none() {
                    release.revision = Some(1);
                }
            }
            Token::Version(text) => {
                // Extract digit from "v2", "v3", etc.
                let digit = text
                    .chars()
                    .find(|c| c.is_ascii_digit())
                    .and_then(|c| c.to_digit(10))
                    .map(|d| d as u8);
                if let Some(v) = digit {
                    release.revision = Some(v);
                }
            }

            _ => {}
        }
    }

    // Post-pass: resolve compound audio codecs.
    if release.audio_codec.is_none() {
        if has_truehd && has_atmos {
            release.audio_codec = Some("TrueHD Atmos".to_string());
        } else if has_truehd {
            release.audio_codec = Some("TrueHD".to_string());
        } else if has_eac3 && has_atmos {
            release.audio_codec = Some("EAC3 Atmos".to_string());
        } else if has_eac3 {
            release.audio_codec = Some("EAC3".to_string());
        } else if has_atmos {
            release.audio_codec = Some("Atmos".to_string());
        }
    }
}

// -------------------------------------------------------------------------
// Title extraction
// -------------------------------------------------------------------------

/// Build the title from tokens that precede the first recognized keyword
/// or year. Dots, underscores, and hyphens are normalized to spaces.
fn extract_title(
    tokens: &[SpannedToken<'_>],
    input: &str,
    release: &mut ParsedRelease,
) {
    // Find the index of the first "stop" token -- a keyword, year, or
    // resolution that signals the end of the title portion.
    let stop_idx = tokens.iter().position(|st| is_title_stop(&st.token));

    let end = stop_idx.unwrap_or(tokens.len());

    // If the group occupies tokens at the tail, don't include them in the
    // title either (this is already handled by stop tokens for most cases).

    let mut parts: Vec<&str> = Vec::new();

    for st in tokens.iter().take(end) {
        match &st.token {
            // Delimiters become word separators (we just push a marker).
            Token::Dot | Token::Underscore => {
                // Only add space if we already have content.
                // Actual joining happens below.
            }
            Token::Hyphen => {
                // Hyphens between title words become spaces.
            }
            Token::Word(w) => parts.push(w),
            Token::Number(n) => parts.push(n),
            _ => {
                // Any recognized keyword inside the title range shouldn't
                // normally happen, but skip gracefully.
            }
        }
    }

    if parts.is_empty() {
        // Fallback: use the raw input up to the first keyword as the title.
        let fallback = if let Some(idx) = stop_idx {
            let byte_end = tokens[idx].span.start;
            &input[..byte_end]
        } else {
            input
        };
        release.title = clean_title(fallback);
    } else {
        release.title = parts.join(" ");
    }

    // If the title is still empty, use the whole input as a last resort.
    if release.title.is_empty() {
        release.title = clean_title(input);
    }
}

// -------------------------------------------------------------------------
// Helpers
// -------------------------------------------------------------------------

/// Parse a SeasonEpisode token like "S01E01" or "S02E03E04" into
/// season, episode, and optional episode_end fields.
fn parse_season_episode(text: &str, release: &mut ParsedRelease) {
    let upper = text.to_uppercase();
    // Split on 'S' to get the season part, then split on 'E' for episodes.
    // Format: S<season>E<ep1>[E<ep2>]
    let after_s = &upper[1..]; // skip the leading 'S'
    if let Some(e_pos) = after_s.find('E') {
        if let Ok(season) = after_s[..e_pos].parse::<u32>() {
            release.season = Some(season);
        }
        // Parse all E## segments
        let ep_part = &after_s[e_pos..]; // "E01" or "E01E02"
        let episodes: Vec<u32> = ep_part
            .split('E')
            .filter(|s| !s.is_empty())
            .filter_map(|s| s.parse().ok())
            .collect();
        if let Some(&first) = episodes.first() {
            release.episode = Some(first);
        }
        if episodes.len() > 1 {
            if let Some(&last) = episodes.last() {
                release.episode_end = Some(last);
            }
        }
    }
}

/// Replace dots, underscores with spaces and trim.
fn clean_title(raw: &str) -> String {
    raw.replace('.', " ")
        .replace('_', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Normalize a resolution token to its canonical form (e.g. "1080P" -> "1080p").
fn normalize_resolution(text: &str) -> String {
    text.to_lowercase()
}

/// Set an `Option<String>` only if it is currently `None`.
fn set_if_none(field: &mut Option<String>, value: &str) {
    if field.is_none() {
        *field = Some(value.to_string());
    }
}

/// Whether this token is a plain word or number (not a keyword).
fn is_plain_word(token: &Token) -> bool {
    matches!(token, Token::Word(_) | Token::Number(_))
}

/// Whether this token is a recognized keyword (anything other than
/// Word, Number, Dot, Hyphen, Underscore).
fn is_keyword(token: &Token) -> bool {
    !matches!(
        token,
        Token::Word(_)
            | Token::Number(_)
            | Token::Dot
            | Token::Hyphen
            | Token::Underscore
            | Token::Year(_)
            | Token::SeasonEpisode(_)
    )
}

/// Tokens that may legally follow the release group at the end.
fn is_ignorable_after_group(token: &Token) -> bool {
    matches!(
        token,
        Token::Dot | Token::Underscore | Token::Hyphen | Token::Number(_)
    ) || is_container_word(token)
}

/// Whether the token is a word that looks like a file extension.
fn is_container_word(token: &Token) -> bool {
    if let Token::Word(w) = token {
        matches!(
            w.to_lowercase().as_str(),
            "mkv" | "mp4" | "avi" | "m4v" | "ts" | "m2ts"
        )
    } else {
        false
    }
}

/// Whether a token should stop the title scan.
fn is_title_stop(token: &Token) -> bool {
    matches!(
        token,
        Token::Year(_)
            | Token::SeasonEpisode(_)
            | Token::Resolution(_)
            | Token::SourceBluRay(_)
            | Token::SourceWebDL(_)
            | Token::SourceWebRip(_)
            | Token::SourceWeb(_)
            | Token::SourceHDTV(_)
            | Token::SourceDVDRip(_)
            | Token::SourceRemux(_)
            | Token::CodecX264(_)
            | Token::CodecX265(_)
            | Token::CodecH264(_)
            | Token::CodecH265(_)
            | Token::CodecAV1(_)
            | Token::CodecVP9(_)
            | Token::CodecMPEG2(_)
            | Token::CodecXviD(_)
            | Token::CodecDivX(_)
            | Token::AudioDTSHD(_)
            | Token::AudioTrueHD(_)
            | Token::AudioAtmos(_)
            | Token::AudioEAC3(_)
            | Token::AudioAC3(_)
            | Token::AudioDD51(_)
            | Token::AudioDTS(_)
            | Token::AudioAAC(_)
            | Token::AudioFLAC(_)
            | Token::AudioOpus(_)
            | Token::HdrHDR10Plus(_)
            | Token::HdrHDR10(_)
            | Token::HdrGeneric(_)
            | Token::HdrDolbyVision(_)
            | Token::HdrHLG(_)
            | Token::EditionDirectorsCut(_)
            | Token::EditionExtended(_)
            | Token::EditionUnrated(_)
            | Token::EditionRemastered(_)
            | Token::EditionIMAX(_)
            | Token::EditionTheatrical(_)
            | Token::EditionSpecial(_)
            | Token::Proper(_)
            | Token::Repack(_)
            | Token::Version(_)
    )
}

/// Extract the text from a token that carries payload.
fn token_text<'a>(token: &Token<'a>) -> Option<&'a str> {
    match token {
        Token::Word(s)
        | Token::Number(s)
        | Token::Year(s)
        | Token::SeasonEpisode(s)
        | Token::Resolution(s)
        | Token::SourceBluRay(s)
        | Token::SourceWebDL(s)
        | Token::SourceWebRip(s)
        | Token::SourceWeb(s)
        | Token::SourceHDTV(s)
        | Token::SourceDVDRip(s)
        | Token::SourceRemux(s)
        | Token::CodecX264(s)
        | Token::CodecX265(s)
        | Token::CodecH264(s)
        | Token::CodecH265(s)
        | Token::CodecAV1(s)
        | Token::CodecVP9(s)
        | Token::CodecMPEG2(s)
        | Token::CodecXviD(s)
        | Token::CodecDivX(s)
        | Token::AudioDTSHD(s)
        | Token::AudioTrueHD(s)
        | Token::AudioAtmos(s)
        | Token::AudioEAC3(s)
        | Token::AudioAC3(s)
        | Token::AudioDD51(s)
        | Token::AudioDTS(s)
        | Token::AudioAAC(s)
        | Token::AudioFLAC(s)
        | Token::AudioOpus(s)
        | Token::HdrHDR10Plus(s)
        | Token::HdrHDR10(s)
        | Token::HdrGeneric(s)
        | Token::HdrDolbyVision(s)
        | Token::HdrHLG(s)
        | Token::EditionDirectorsCut(s)
        | Token::EditionExtended(s)
        | Token::EditionUnrated(s)
        | Token::EditionRemastered(s)
        | Token::EditionIMAX(s)
        | Token::EditionTheatrical(s)
        | Token::EditionSpecial(s)
        | Token::Proper(s)
        | Token::Repack(s)
        | Token::Version(s) => Some(s),
        Token::Dot | Token::Hyphen | Token::Underscore => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_the_matrix() {
        let r = parse("The.Matrix.1999.1080p.BluRay.x264-GROUP");
        assert_eq!(r.title, "The Matrix");
        assert_eq!(r.year, Some(1999));
        assert_eq!(r.resolution.as_deref(), Some("1080p"));
        assert_eq!(r.source.as_deref(), Some("BluRay"));
        assert_eq!(r.video_codec.as_deref(), Some("x264"));
        assert_eq!(r.group.as_deref(), Some("GROUP"));
    }

    #[test]
    fn test_breaking_bad() {
        let r = parse("Breaking.Bad.S01E01.720p.WEB-DL.DD5.1.H.264-DEMAND");
        assert_eq!(r.title, "Breaking Bad");
        assert_eq!(r.season, Some(1));
        assert_eq!(r.episode, Some(1));
        assert_eq!(r.episode_end, None);
        assert_eq!(r.resolution.as_deref(), Some("720p"));
        assert_eq!(r.source.as_deref(), Some("WEB-DL"));
    }

    #[test]
    fn test_4k_movie() {
        let r = parse("Movie.2023.2160p.UHD.BluRay.Remux.HDR.DV.TrueHD.7.1.Atmos.HEVC-FraMeSToR");
        assert_eq!(r.title, "Movie");
        assert_eq!(r.year, Some(2023));
        assert_eq!(r.resolution.as_deref(), Some("2160p"));
        assert!(r.hdr.is_some());
        assert!(r.audio_codec.is_some());
        assert_eq!(r.group.as_deref(), Some("FraMeSToR"));
    }

    #[test]
    fn test_directors_cut() {
        let r = parse("Some.Movie.2020.Directors.Cut.1080p.BluRay.x265-GROUP");
        assert_eq!(r.title, "Some Movie");
        assert_eq!(r.year, Some(2020));
        assert_eq!(r.edition.as_deref(), Some("Director's Cut"));
        assert_eq!(r.resolution.as_deref(), Some("1080p"));
        assert_eq!(r.source.as_deref(), Some("BluRay"));
        assert_eq!(r.video_codec.as_deref(), Some("x265"));
        assert_eq!(r.group.as_deref(), Some("GROUP"));
    }

    #[test]
    fn test_simple_title() {
        let r = parse("My Movie");
        assert_eq!(r.title, "My Movie");
        assert_eq!(r.year, None);
        assert_eq!(r.resolution, None);
    }

    #[test]
    fn test_proper_revision() {
        let r = parse("Movie.2020.1080p.BluRay.PROPER.x264-GROUP");
        assert_eq!(r.revision, Some(1));
    }

    #[test]
    fn test_version_revision() {
        let r = parse("Movie.2020.1080p.BluRay.x264.v2-GROUP");
        assert_eq!(r.revision, Some(2));
    }

    #[test]
    fn test_extended_edition() {
        let r = parse("Movie.2020.Extended.1080p.BluRay-GROUP");
        assert_eq!(r.edition.as_deref(), Some("Extended"));
    }

    #[test]
    fn test_truehd_atmos() {
        let r = parse("Movie.2023.2160p.BluRay.TrueHD.Atmos.x265-GROUP");
        assert_eq!(r.audio_codec.as_deref(), Some("TrueHD Atmos"));
    }

    #[test]
    fn test_eac3() {
        let r = parse("Movie.2023.1080p.WEB-DL.DDP.x264-GROUP");
        assert_eq!(r.audio_codec.as_deref(), Some("EAC3"));
    }

    #[test]
    fn test_dts_hd() {
        let r = parse("Movie.2023.1080p.BluRay.DTS-HD.MA.x264-GROUP");
        assert_eq!(r.audio_codec.as_deref(), Some("DTS-HD"));
    }

    #[test]
    fn test_hdr10_detection() {
        let r = parse("Movie.2023.2160p.BluRay.HDR10.x265-GROUP");
        assert_eq!(r.hdr.as_deref(), Some("HDR10"));
    }

    #[test]
    fn test_dolby_vision() {
        let r = parse("Movie.2023.2160p.BluRay.DV.x265-GROUP");
        assert_eq!(r.hdr.as_deref(), Some("DV"));
    }

    #[test]
    fn test_hdr10_plus() {
        let r = parse("Movie.2023.2160p.BluRay.HDR10+.x265-GROUP");
        assert_eq!(r.hdr.as_deref(), Some("HDR10+"));
    }

    #[test]
    fn test_multi_episode() {
        let r = parse("Show.Name.S02E03E04.1080p.WEB-DL.x264-GROUP");
        assert_eq!(r.title, "Show Name");
        assert_eq!(r.season, Some(2));
        assert_eq!(r.episode, Some(3));
        assert_eq!(r.episode_end, Some(4));
    }

    #[test]
    fn test_tv_show_with_year() {
        let r = parse("The.Mandalorian.2019.S01E01.1080p.WEB-DL.x264-GROUP");
        assert_eq!(r.title, "The Mandalorian");
        assert_eq!(r.year, Some(2019));
        assert_eq!(r.season, Some(1));
        assert_eq!(r.episode, Some(1));
    }

    #[test]
    fn test_movie_has_no_season() {
        let r = parse("The.Matrix.1999.1080p.BluRay.x264-GROUP");
        assert_eq!(r.season, None);
        assert_eq!(r.episode, None);
    }
}
