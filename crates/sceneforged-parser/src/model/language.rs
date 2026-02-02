//! Language enum for audio and subtitle tracks.

use super::ParseError;

/// Language of audio or subtitle tracks.
///
/// Based on ISO 639-1 language codes for common languages found in media releases.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum Language {
    /// English (en)
    English,
    /// French (fr)
    French,
    /// German (de)
    German,
    /// Spanish (es)
    Spanish,
    /// Italian (it)
    Italian,
    /// Portuguese (pt)
    Portuguese,
    /// Russian (ru)
    Russian,
    /// Japanese (ja)
    Japanese,
    /// Korean (ko)
    Korean,
    /// Chinese (zh) - generic
    Chinese,
    /// Mandarin Chinese (cmn)
    Mandarin,
    /// Cantonese Chinese (yue)
    Cantonese,
    /// Arabic (ar)
    Arabic,
    /// Hindi (hi)
    Hindi,
    /// Turkish (tr)
    Turkish,
    /// Polish (pl)
    Polish,
    /// Dutch (nl)
    Dutch,
    /// Swedish (sv)
    Swedish,
    /// Norwegian (no)
    Norwegian,
    /// Danish (da)
    Danish,
    /// Finnish (fi)
    Finnish,
    /// Czech (cs)
    Czech,
    /// Hungarian (hu)
    Hungarian,
    /// Romanian (ro)
    Romanian,
    /// Bulgarian (bg)
    Bulgarian,
    /// Greek (el)
    Greek,
    /// Hebrew (he)
    Hebrew,
    /// Thai (th)
    Thai,
    /// Vietnamese (vi)
    Vietnamese,
    /// Indonesian (id)
    Indonesian,
    /// Malay (ms)
    Malay,
    /// Filipino (fil)
    Filipino,
    /// Ukrainian (uk)
    Ukrainian,
    /// Croatian (hr)
    Croatian,
    /// Serbian (sr)
    Serbian,
    /// Slovenian (sl)
    Slovenian,
    /// Slovak (sk)
    Slovak,
    /// Lithuanian (lt)
    Lithuanian,
    /// Latvian (lv)
    Latvian,
    /// Estonian (et)
    Estonian,
    /// Bengali (bn)
    Bengali,
    /// Tamil (ta)
    Tamil,
    /// Telugu (te)
    Telugu,
    /// Punjabi (pa)
    Punjabi,
    /// Marathi (mr)
    Marathi,
    /// Gujarati (gu)
    Gujarati,
    /// Kannada (kn)
    Kannada,
    /// Malayalam (ml)
    Malayalam,
    /// Persian (fa)
    Persian,
    /// Urdu (ur)
    Urdu,
    /// Swahili (sw)
    Swahili,
    /// Latin (la)
    Latin,
    /// Multi-language release
    Multi,
}

impl std::fmt::Display for Language {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Language::English => write!(f, "English"),
            Language::French => write!(f, "French"),
            Language::German => write!(f, "German"),
            Language::Spanish => write!(f, "Spanish"),
            Language::Italian => write!(f, "Italian"),
            Language::Portuguese => write!(f, "Portuguese"),
            Language::Russian => write!(f, "Russian"),
            Language::Japanese => write!(f, "Japanese"),
            Language::Korean => write!(f, "Korean"),
            Language::Chinese => write!(f, "Chinese"),
            Language::Mandarin => write!(f, "Mandarin"),
            Language::Cantonese => write!(f, "Cantonese"),
            Language::Arabic => write!(f, "Arabic"),
            Language::Hindi => write!(f, "Hindi"),
            Language::Turkish => write!(f, "Turkish"),
            Language::Polish => write!(f, "Polish"),
            Language::Dutch => write!(f, "Dutch"),
            Language::Swedish => write!(f, "Swedish"),
            Language::Norwegian => write!(f, "Norwegian"),
            Language::Danish => write!(f, "Danish"),
            Language::Finnish => write!(f, "Finnish"),
            Language::Czech => write!(f, "Czech"),
            Language::Hungarian => write!(f, "Hungarian"),
            Language::Romanian => write!(f, "Romanian"),
            Language::Bulgarian => write!(f, "Bulgarian"),
            Language::Greek => write!(f, "Greek"),
            Language::Hebrew => write!(f, "Hebrew"),
            Language::Thai => write!(f, "Thai"),
            Language::Vietnamese => write!(f, "Vietnamese"),
            Language::Indonesian => write!(f, "Indonesian"),
            Language::Malay => write!(f, "Malay"),
            Language::Filipino => write!(f, "Filipino"),
            Language::Ukrainian => write!(f, "Ukrainian"),
            Language::Croatian => write!(f, "Croatian"),
            Language::Serbian => write!(f, "Serbian"),
            Language::Slovenian => write!(f, "Slovenian"),
            Language::Slovak => write!(f, "Slovak"),
            Language::Lithuanian => write!(f, "Lithuanian"),
            Language::Latvian => write!(f, "Latvian"),
            Language::Estonian => write!(f, "Estonian"),
            Language::Bengali => write!(f, "Bengali"),
            Language::Tamil => write!(f, "Tamil"),
            Language::Telugu => write!(f, "Telugu"),
            Language::Punjabi => write!(f, "Punjabi"),
            Language::Marathi => write!(f, "Marathi"),
            Language::Gujarati => write!(f, "Gujarati"),
            Language::Kannada => write!(f, "Kannada"),
            Language::Malayalam => write!(f, "Malayalam"),
            Language::Persian => write!(f, "Persian"),
            Language::Urdu => write!(f, "Urdu"),
            Language::Swahili => write!(f, "Swahili"),
            Language::Latin => write!(f, "Latin"),
            Language::Multi => write!(f, "Multi"),
        }
    }
}

impl std::str::FromStr for Language {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "english" | "en" | "eng" => Ok(Language::English),
            "french" | "fr" | "fra" | "fre" => Ok(Language::French),
            "german" | "de" | "deu" | "ger" => Ok(Language::German),
            "spanish" | "es" | "spa" | "esp" => Ok(Language::Spanish),
            "italian" | "it" | "ita" => Ok(Language::Italian),
            "portuguese" | "pt" | "por" => Ok(Language::Portuguese),
            "russian" | "ru" | "rus" => Ok(Language::Russian),
            "japanese" | "ja" | "jpn" | "jap" => Ok(Language::Japanese),
            "korean" | "ko" | "kor" => Ok(Language::Korean),
            "chinese" | "zh" | "zho" | "chi" => Ok(Language::Chinese),
            "mandarin" | "cmn" => Ok(Language::Mandarin),
            "cantonese" | "yue" => Ok(Language::Cantonese),
            "arabic" | "ar" | "ara" => Ok(Language::Arabic),
            "hindi" | "hi" | "hin" => Ok(Language::Hindi),
            "turkish" | "tr" | "tur" => Ok(Language::Turkish),
            "polish" | "pl" | "pol" => Ok(Language::Polish),
            "dutch" | "nl" | "nld" | "dut" => Ok(Language::Dutch),
            "swedish" | "sv" | "swe" => Ok(Language::Swedish),
            "norwegian" | "no" | "nor" => Ok(Language::Norwegian),
            "danish" | "da" | "dan" => Ok(Language::Danish),
            "finnish" | "fi" | "fin" => Ok(Language::Finnish),
            "czech" | "cs" | "ces" | "cze" => Ok(Language::Czech),
            "hungarian" | "hu" | "hun" => Ok(Language::Hungarian),
            "romanian" | "ro" | "ron" | "rum" => Ok(Language::Romanian),
            "bulgarian" | "bg" | "bul" => Ok(Language::Bulgarian),
            "greek" | "el" | "ell" | "gre" => Ok(Language::Greek),
            "hebrew" | "he" | "heb" => Ok(Language::Hebrew),
            "thai" | "th" | "tha" => Ok(Language::Thai),
            "vietnamese" | "vi" | "vie" => Ok(Language::Vietnamese),
            "indonesian" | "id" | "ind" => Ok(Language::Indonesian),
            "malay" | "ms" | "msa" | "may" => Ok(Language::Malay),
            "filipino" | "fil" | "tl" | "tgl" => Ok(Language::Filipino),
            "ukrainian" | "uk" | "ukr" => Ok(Language::Ukrainian),
            "croatian" | "hr" | "hrv" => Ok(Language::Croatian),
            "serbian" | "sr" | "srp" => Ok(Language::Serbian),
            "slovenian" | "sl" | "slv" => Ok(Language::Slovenian),
            "slovak" | "sk" | "slk" | "slo" => Ok(Language::Slovak),
            "lithuanian" | "lt" | "lit" => Ok(Language::Lithuanian),
            "latvian" | "lv" | "lav" => Ok(Language::Latvian),
            "estonian" | "et" | "est" => Ok(Language::Estonian),
            "bengali" | "bn" | "ben" => Ok(Language::Bengali),
            "tamil" | "ta" | "tam" => Ok(Language::Tamil),
            "telugu" | "te" | "tel" => Ok(Language::Telugu),
            "punjabi" | "pa" | "pan" => Ok(Language::Punjabi),
            "marathi" | "mr" | "mar" => Ok(Language::Marathi),
            "gujarati" | "gu" | "guj" => Ok(Language::Gujarati),
            "kannada" | "kn" | "kan" => Ok(Language::Kannada),
            "malayalam" | "ml" | "mal" => Ok(Language::Malayalam),
            "persian" | "fa" | "fas" | "per" | "farsi" => Ok(Language::Persian),
            "urdu" | "ur" | "urd" => Ok(Language::Urdu),
            "swahili" | "sw" | "swa" => Ok(Language::Swahili),
            "latin" | "la" | "lat" => Ok(Language::Latin),
            "multi" | "mul" | "multiple" => Ok(Language::Multi),
            _ => Err(ParseError(format!("invalid language: {}", s))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn language_display_fromstr_roundtrip() {
        let variants = [
            Language::English,
            Language::French,
            Language::German,
            Language::Spanish,
            Language::Italian,
            Language::Portuguese,
            Language::Russian,
            Language::Japanese,
            Language::Korean,
            Language::Chinese,
            Language::Mandarin,
            Language::Cantonese,
            Language::Arabic,
            Language::Hindi,
            Language::Turkish,
            Language::Polish,
            Language::Dutch,
            Language::Swedish,
            Language::Norwegian,
            Language::Danish,
            Language::Finnish,
            Language::Czech,
            Language::Hungarian,
            Language::Romanian,
            Language::Bulgarian,
            Language::Greek,
            Language::Hebrew,
            Language::Thai,
            Language::Vietnamese,
            Language::Indonesian,
            Language::Malay,
            Language::Filipino,
            Language::Ukrainian,
            Language::Croatian,
            Language::Serbian,
            Language::Slovenian,
            Language::Slovak,
            Language::Lithuanian,
            Language::Latvian,
            Language::Estonian,
            Language::Bengali,
            Language::Tamil,
            Language::Telugu,
            Language::Punjabi,
            Language::Marathi,
            Language::Gujarati,
            Language::Kannada,
            Language::Malayalam,
            Language::Persian,
            Language::Urdu,
            Language::Swahili,
            Language::Latin,
            Language::Multi,
        ];
        for variant in variants {
            let s = variant.to_string();
            let parsed: Language = s.parse().expect("should parse");
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn language_iso_codes() {
        // Test common ISO 639-1 codes
        assert_eq!("en".parse::<Language>().ok(), Some(Language::English));
        assert_eq!("fr".parse::<Language>().ok(), Some(Language::French));
        assert_eq!("de".parse::<Language>().ok(), Some(Language::German));
        assert_eq!("ja".parse::<Language>().ok(), Some(Language::Japanese));
        assert_eq!("ko".parse::<Language>().ok(), Some(Language::Korean));
        assert_eq!("zh".parse::<Language>().ok(), Some(Language::Chinese));
    }
}
