//! Token types for the Logos-based lexer.

use logos::Logos;

/// Token types recognized by the lexer.
///
/// Each variant represents a specific pattern in release names, ordered by priority
/// where needed. The lexer automatically handles tokenization and classification.
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t]+")]
pub enum Token<'src> {
    /// Season and episode identifier (e.g., S01E05, S1E1, S01E01E02, S01E12v2, S1936E18, S14E3533)
    /// Supports seasons up to 4 digits, episodes up to 4 digits
    /// Also supports "EP" variant (S01EP01)
    /// Range patterns (S01E01-E05) are handled in the parser by combining tokens
    #[regex(
        r"(?i)S[0-9]{1,4}E[Pp]?[0-9]{1,4}(?:E[Pp]?[0-9]{1,4})*(?:v[0-9]+)?",
        priority = 10
    )]
    SeasonEpisode(&'src str),

    /// Season x episode format (e.g., 1x05, 01x05, 19x06, 2009x09)
    /// Lower priority than Resolution to avoid matching "1920x1080"
    #[regex(r"[0-9]{1,4}x[0-9]{1,3}", priority = 9)]
    SeasonEpisodeX(&'src str),

    /// Season-only identifier (e.g., S01, S1) - for full season releases
    /// Note: SeasonEpisode has higher priority, so this only matches when no E follows
    #[regex(r"(?i)S\d{1,4}", priority = 8)]
    SeasonOnly(&'src str),

    /// Part episode format (e.g., Part01, Part1, Part02)
    #[regex(r"(?i)Part\d{1,2}", priority = 9)]
    PartEpisode(&'src str),

    /// Spelled-out "Season" keyword
    #[regex(r"(?i)Season", priority = 7)]
    SeasonWord(&'src str),

    /// Spelled-out "Episode" keyword
    #[regex(r"(?i)Episode", priority = 7)]
    EpisodeWord(&'src str),

    /// Abbreviated "Ep" episode format (e.g., Ep06, Ep1)
    #[regex(r"(?i)Ep[0-9]{1,3}", priority = 9)]
    EpNumber(&'src str),

    /// X of Y format (e.g., "5of9", "1of6")
    #[regex(r"\d{1,2}of\d{1,2}", priority = 9)]
    EpisodeOfTotal(&'src str),

    /// Video resolution (e.g., 2160p, 1080p, 720p, 1920x1080)
    #[regex(
        r"(?i)((2160|1080|720|480|576|360)[pi]?|1920x1080|3840x2160|1280x720)",
        priority = 10
    )]
    Resolution(&'src str),

    /// H.265/HEVC codec variants
    #[regex(r"(?i)(x265|H\.?265|HEVC)", priority = 8)]
    CodecH265(&'src str),

    /// H.264/AVC codec variants
    #[regex(r"(?i)(x264|H\.?264|AVC)", priority = 8)]
    CodecH264(&'src str),

    /// AV1 codec
    #[regex(r"(?i)AV1", priority = 8)]
    CodecAv1(&'src str),

    /// MPEG-4 Part 2 codecs (XviD, DivX)
    #[regex(r"(?i)(Xvi[Dd]|DivX)", priority = 8)]
    CodecMpeg4(&'src str),

    /// BDRip source variant (lower quality BluRay rip)
    #[regex(r"(?i)BDRip", priority = 8)]
    SourceBdRip(&'src str),

    /// BluRay source variants (including BRRip)
    #[regex(r"(?i)(BluRay|BRRip|Blu-Ray)", priority = 7)]
    SourceBluray(&'src str),

    /// Web download source
    #[regex(r"(?i)(WEB-?DL|WEBDL)", priority = 7)]
    SourceWebDL(&'src str),

    /// Web rip source
    #[regex(r"(?i)(WEB-?Rip|WEBRIP)", priority = 7)]
    SourceWebRip(&'src str),

    /// HD source (HDRip, HDTV)
    #[regex(r"(?i)(HDRip|HDTV)", priority = 7)]
    SourceHD(&'src str),

    /// DVD source
    #[regex(r"(?i)(DVDRip|DVD-?R)", priority = 7)]
    SourceDVD(&'src str),

    /// CAM source (theater recording)
    #[regex(r"(?i)(CAM|CAMRIP|HDCAM)", priority = 7)]
    SourceCam(&'src str),

    /// Telesync source
    #[regex(r"(?i)(TS|TELESYNC|HDTS)", priority = 7)]
    SourceTelesync(&'src str),

    /// Telecine source
    #[regex(r"(?i)(TC|TELECINE)", priority = 7)]
    SourceTelecine(&'src str),

    /// Screener source
    #[regex(r"(?i)(SCR|SCREENER|DVDSCR|BDSCR|R5)", priority = 7)]
    SourceScreener(&'src str),

    /// PPV source
    #[regex(r"(?i)PPV", priority = 7)]
    SourcePPV(&'src str),

    /// PDTV/SDTV sources
    #[regex(r"(?i)(PDTV|SDTV|DSR|DSRIP|SATRIP)", priority = 7)]
    SourceTV(&'src str),

    /// Bare WEB source (without -DL/-Rip suffix)
    /// Note: SourceWebDL and SourceWebRip have higher priority, so this only matches bare WEB
    #[regex(r"(?i)WEB", priority = 5)]
    SourceWeb(&'src str),

    /// BD source (bare BluRay marker)
    /// Note: Other BD patterns like BDRip have higher priority
    #[regex(r"(?i)BD", priority = 5)]
    SourceBD(&'src str),

    /// REMUX quality modifier
    #[regex(r"(?i)REMUX", priority = 7)]
    QualityRemux(&'src str),

    /// DTS-HD audio codec
    #[regex(r"(?i)(DTS-?HD|DTS:X|DTS-?X)", priority = 9)]
    AudioDTSHD(&'src str),

    /// Standard DTS audio
    #[regex(r"(?i)DTS", priority = 6)]
    AudioDTS(&'src str),

    /// TrueHD/Atmos audio
    #[regex(r"(?i)(TrueHD|Atmos)", priority = 9)]
    AudioTrueHD(&'src str),

    /// Dolby Digital Plus (E-AC-3) - may include channel config
    #[regex(r"(?i)(DD\+|DDP|E-?AC-?3|EAC3)(\d\.\d)?", priority = 8)]
    AudioDDPlus(&'src str),

    /// Dolby Digital (AC-3) - may include channel config
    #[regex(r"(?i)(DD|AC-?3|AC3)(\d\.\d)?", priority = 7)]
    AudioDD(&'src str),

    /// Other audio formats
    #[regex(r"(?i)(AAC|FLAC|MP3|LPCM|PCM)", priority = 6)]
    AudioFormat(&'src str),

    /// Audio channel configuration (e.g., 5.1, 7.1)
    #[regex(r"(?i)(7\.1|5\.1|2\.0|2\.1|1\.0)", priority = 8)]
    AudioChannels(&'src str),

    /// Dolby Vision HDR
    #[regex(r"(?i)(Dolby.?Vision|DoVi|DV)", priority = 9)]
    HdrDolbyVision(&'src str),

    /// HDR10+ format
    #[regex(r"(?i)(HDR10\+|HDR10Plus)", priority = 9)]
    HdrHDR10Plus(&'src str),

    /// HDR10 format
    #[regex(r"(?i)HDR10", priority = 8)]
    HdrHDR10(&'src str),

    /// Generic HDR formats (HDR, HLG, PQ)
    #[regex(r"(?i)(HDR|HLG|PQ)", priority = 7)]
    HdrGeneric(&'src str),

    /// 10-bit color depth
    #[regex(r"(?i)(10bit|10-?bit)", priority = 7)]
    BitDepth10(&'src str),

    /// 8-bit color depth
    #[regex(r"(?i)(8bit|8-?bit)", priority = 7)]
    BitDepth8(&'src str),

    /// Release modifiers (REPACK, PROPER, etc.)
    #[regex(r"(?i)(REPACK|PROPER|REAL|RERIP)", priority = 6)]
    ReleaseModifier(&'src str),

    /// Edition identifiers (EXTENDED, UNCUT, etc.)
    #[regex(
        r"(?i)(EXTENDED|UNCUT|UNRATED|DC|DIRECTORS.?CUT|THEATRICAL)",
        priority = 6
    )]
    Edition(&'src str),

    /// Streaming service identifiers
    #[regex(
        r"(?i)(AMZN|NF|DSNP|HMAX|ATVP|PMTP|STAN|CRAV|PCOK|MA|HULU|iP)",
        priority = 7
    )]
    StreamingService(&'src str),

    /// Year (1900-2099)
    #[regex(r"(19|20)\d{2}", priority = 5)]
    Year(&'src str),

    /// Dot delimiter
    #[token(".")]
    Dot,

    /// Hyphen delimiter
    #[token("-")]
    Hyphen,

    /// Underscore delimiter
    #[token("_")]
    Underscore,

    /// Ampersand character (preserved in titles)
    #[token("&")]
    Ampersand,

    /// Opening square bracket
    #[token("[")]
    BracketOpen,

    /// Closing square bracket
    #[token("]")]
    BracketClose,

    /// Opening parenthesis
    #[token("(")]
    ParenOpen,

    /// Closing parenthesis
    #[token(")")]
    ParenClose,

    /// Generic word token (lower priority than specific patterns)
    #[regex(r"[a-zA-Z][a-zA-Z0-9'&]*", priority = 1)]
    Word(&'src str),

    /// Numeric token
    #[regex(r"\d+", priority = 2)]
    Number(&'src str),
}
