//! Logos-based tokenizer for media release filenames.
//!
//! Each variant of [`Token`] corresponds to a recognizable keyword or
//! structural element commonly found in scene/P2P release names.
//! The lexer is case-insensitive for all keyword patterns.

use logos::Logos;

/// Token types emitted by the Logos lexer.
///
/// Variants are ordered by specificity -- more specific patterns receive
/// higher priorities so they win when multiple regexes could match the
/// same input span.
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t]+")]
pub enum Token<'src> {
    // -----------------------------------------------------------------
    // Resolution
    // -----------------------------------------------------------------
    /// Video resolution: 2160p, 1080p, 720p, 480p (case-insensitive).
    #[regex(r"(?i)(2160|1080|720|480)[pi]", priority = 10)]
    Resolution(&'src str),

    // -----------------------------------------------------------------
    // Source
    // -----------------------------------------------------------------
    /// BluRay / Blu-Ray / BRRip / BDRip source.
    #[regex(r"(?i)(Blu-?Ray|BRRip|BDRip)", priority = 8)]
    SourceBluRay(&'src str),

    /// WEB-DL / WEBDL source.
    #[regex(r"(?i)(WEB-?DL|WEBDL)", priority = 8)]
    SourceWebDL(&'src str),

    /// WEBRip source.
    #[regex(r"(?i)WEB-?Rip", priority = 8)]
    SourceWebRip(&'src str),

    /// Bare WEB source (lower priority than WEB-DL / WEBRip).
    #[regex(r"(?i)WEB", priority = 5)]
    SourceWeb(&'src str),

    /// HDTV source.
    #[regex(r"(?i)HDTV", priority = 8)]
    SourceHDTV(&'src str),

    /// DVDRip source.
    #[regex(r"(?i)DVDRip", priority = 8)]
    SourceDVDRip(&'src str),

    /// Remux quality/source indicator.
    #[regex(r"(?i)Remux", priority = 8)]
    SourceRemux(&'src str),

    // -----------------------------------------------------------------
    // Video codecs
    // -----------------------------------------------------------------
    /// x264 encoder.
    #[regex(r"(?i)x264", priority = 9)]
    CodecX264(&'src str),

    /// x265 encoder.
    #[regex(r"(?i)x265", priority = 9)]
    CodecX265(&'src str),

    /// H.264 / H264 / AVC.
    #[regex(r"(?i)(H\.?264|AVC)", priority = 8)]
    CodecH264(&'src str),

    /// H.265 / H265 / HEVC.
    #[regex(r"(?i)(H\.?265|HEVC)", priority = 8)]
    CodecH265(&'src str),

    /// AV1 codec.
    #[regex(r"(?i)AV1", priority = 8)]
    CodecAV1(&'src str),

    /// VP9 codec.
    #[regex(r"(?i)VP9", priority = 8)]
    CodecVP9(&'src str),

    /// MPEG2 codec.
    #[regex(r"(?i)MPEG-?2", priority = 8)]
    CodecMPEG2(&'src str),

    /// XviD codec.
    #[regex(r"(?i)XviD", priority = 8)]
    CodecXviD(&'src str),

    /// DivX codec.
    #[regex(r"(?i)DivX", priority = 8)]
    CodecDivX(&'src str),

    // -----------------------------------------------------------------
    // Audio codecs
    // -----------------------------------------------------------------
    /// DTS-HD.MA / DTS-HD audio (must be higher priority than plain DTS).
    #[regex(r"(?i)DTS-?HD(\.?MA)?", priority = 10)]
    AudioDTSHD(&'src str),

    /// TrueHD audio.
    #[regex(r"(?i)TrueHD", priority = 9)]
    AudioTrueHD(&'src str),

    /// Atmos audio modifier.
    #[regex(r"(?i)Atmos", priority = 9)]
    AudioAtmos(&'src str),

    /// E-AC-3 / EAC3 / DD+ / DDP (Dolby Digital Plus).
    #[regex(r"(?i)(E-?AC-?3|EAC3|DD\+|DDP)", priority = 9)]
    AudioEAC3(&'src str),

    /// AC3 / DD (Dolby Digital) -- lower priority than EAC3/DD+.
    #[regex(r"(?i)(AC-?3|AC3)", priority = 7)]
    AudioAC3(&'src str),

    /// DD5.1 -- explicit Dolby Digital 5.1 token.
    #[regex(r"(?i)DD5\.1", priority = 10)]
    AudioDD51(&'src str),

    /// Plain DTS audio (lower priority than DTS-HD).
    #[regex(r"(?i)DTS", priority = 6)]
    AudioDTS(&'src str),

    /// AAC audio.
    #[regex(r"(?i)AAC", priority = 8)]
    AudioAAC(&'src str),

    /// FLAC audio.
    #[regex(r"(?i)FLAC", priority = 8)]
    AudioFLAC(&'src str),

    /// Opus audio.
    #[regex(r"(?i)OPUS", priority = 8)]
    AudioOpus(&'src str),

    // -----------------------------------------------------------------
    // HDR formats
    // -----------------------------------------------------------------
    /// HDR10+ (must be higher priority than HDR10 and HDR).
    #[regex(r"(?i)HDR10\+", priority = 11)]
    HdrHDR10Plus(&'src str),

    /// HDR10 (higher priority than generic HDR).
    #[regex(r"(?i)HDR10", priority = 10)]
    HdrHDR10(&'src str),

    /// Generic HDR.
    #[regex(r"(?i)HDR", priority = 7)]
    HdrGeneric(&'src str),

    /// Dolby Vision: DV / DoVi / Dolby.Vision.
    #[regex(r"(?i)(Dolby\.?Vision|DoVi|DV)", priority = 9)]
    HdrDolbyVision(&'src str),

    /// HLG (Hybrid Log-Gamma).
    #[regex(r"(?i)HLG", priority = 8)]
    HdrHLG(&'src str),

    // -----------------------------------------------------------------
    // Edition
    // -----------------------------------------------------------------
    /// Director's Cut (handles dots/spaces).
    #[regex(r"(?i)Directors?[.\s'-]*Cut", priority = 8)]
    EditionDirectorsCut(&'src str),

    /// Extended edition.
    #[regex(r"(?i)Extended", priority = 7)]
    EditionExtended(&'src str),

    /// Unrated edition.
    #[regex(r"(?i)Unrated", priority = 7)]
    EditionUnrated(&'src str),

    /// Remastered edition.
    #[regex(r"(?i)Remastered", priority = 7)]
    EditionRemastered(&'src str),

    /// IMAX edition.
    #[regex(r"(?i)IMAX", priority = 7)]
    EditionIMAX(&'src str),

    /// Theatrical edition.
    #[regex(r"(?i)Theatrical", priority = 7)]
    EditionTheatrical(&'src str),

    /// Special Edition (handles dots/spaces).
    #[regex(r"(?i)Special[.\s'-]*Edition", priority = 8)]
    EditionSpecial(&'src str),

    // -----------------------------------------------------------------
    // Revision / proper
    // -----------------------------------------------------------------
    /// PROPER release.
    #[regex(r"(?i)PROPER", priority = 7)]
    Proper(&'src str),

    /// REPACK release.
    #[regex(r"(?i)REPACK", priority = 7)]
    Repack(&'src str),

    /// Version marker: v2, v3, etc.
    #[regex(r"(?i)v[2-9]", priority = 7)]
    Version(&'src str),

    // -----------------------------------------------------------------
    // Season / Episode
    // -----------------------------------------------------------------
    /// Season+episode tag, e.g. S01E01, S01E01E02 (multi-episode).
    #[regex(r"(?i)S\d{1,2}E\d{1,2}(E\d{1,2})*", priority = 12)]
    SeasonEpisode(&'src str),

    // -----------------------------------------------------------------
    // Year
    // -----------------------------------------------------------------
    /// Four-digit year 1900--2099.
    #[regex(r"(19|20)\d{2}", priority = 5)]
    Year(&'src str),

    // -----------------------------------------------------------------
    // Structural / separators
    // -----------------------------------------------------------------
    /// Dot separator.
    #[token(".")]
    Dot,

    /// Hyphen separator.
    #[token("-")]
    Hyphen,

    /// Underscore separator.
    #[token("_")]
    Underscore,

    /// Generic word token (lowest priority -- anything not matched above).
    #[regex(r"[a-zA-Z][a-zA-Z0-9']*", priority = 1)]
    Word(&'src str),

    /// Numeric token.
    #[regex(r"\d+", priority = 2)]
    Number(&'src str),
}

/// A token together with the byte span it occupies in the original input.
#[derive(Debug, Clone)]
pub struct SpannedToken<'src> {
    pub token: Token<'src>,
    pub span: std::ops::Range<usize>,
}

/// Tokenize an input string into a `Vec` of spanned tokens.
pub fn tokenize(input: &str) -> Vec<SpannedToken<'_>> {
    Token::lexer(input)
        .spanned()
        .filter_map(|(result, span)| {
            result.ok().map(|token| SpannedToken { token, span })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokenize_basic_movie() {
        let tokens = tokenize("The.Matrix.1999.1080p.BluRay.x264-GROUP");
        let kinds: Vec<_> = tokens.iter().map(|t| &t.token).collect();
        assert!(kinds.contains(&&Token::Year("1999")));
        assert!(kinds.contains(&&Token::Resolution("1080p")));
        assert!(kinds.contains(&&Token::SourceBluRay("BluRay")));
        assert!(kinds.contains(&&Token::CodecX264("x264")));
    }

    #[test]
    fn tokenize_webdl() {
        let tokens = tokenize("Show.2020.WEB-DL.720p");
        let has_webdl = tokens
            .iter()
            .any(|t| matches!(t.token, Token::SourceWebDL(_)));
        assert!(has_webdl, "Should detect WEB-DL");
    }

    #[test]
    fn tokenize_dts_hd() {
        let tokens = tokenize("Movie.DTS-HD.MA.5.1");
        let has_dtshd = tokens
            .iter()
            .any(|t| matches!(t.token, Token::AudioDTSHD(_)));
        assert!(has_dtshd, "Should detect DTS-HD");
    }

    #[test]
    fn tokenize_hdr10_plus() {
        let tokens = tokenize("Movie.HDR10+.2160p");
        let has_hdr10plus = tokens
            .iter()
            .any(|t| matches!(t.token, Token::HdrHDR10Plus(_)));
        assert!(has_hdr10plus, "Should detect HDR10+");
    }

    #[test]
    fn tokenize_directors_cut() {
        let tokens = tokenize("Movie.Directors.Cut.1080p");
        let has_dc = tokens
            .iter()
            .any(|t| matches!(t.token, Token::EditionDirectorsCut(_)));
        assert!(has_dc, "Should detect Directors.Cut");
    }

    #[test]
    fn tokenize_dd51() {
        let tokens = tokenize("Show.DD5.1.H.264");
        let has_dd51 = tokens
            .iter()
            .any(|t| matches!(t.token, Token::AudioDD51(_)));
        assert!(has_dd51, "Should detect DD5.1");
    }

    #[test]
    fn tokenize_season_episode() {
        let tokens = tokenize("Breaking.Bad.S01E01.720p");
        let has_se = tokens
            .iter()
            .any(|t| matches!(t.token, Token::SeasonEpisode(_)));
        assert!(has_se, "Should detect S01E01");
    }

    #[test]
    fn tokenize_multi_episode() {
        let tokens = tokenize("Show.S02E03E04.1080p");
        let se = tokens
            .iter()
            .find(|t| matches!(t.token, Token::SeasonEpisode(_)))
            .expect("Should detect multi-episode");
        if let Token::SeasonEpisode(text) = se.token {
            assert_eq!(text, "S02E03E04");
        }
    }
}
