use crate::config::{HdrFormatMatch, MatchConditions, NormalizedMatchConditions, Resolution, Rule};
use crate::probe::{HdrFormat, MediaInfo};

/// Check if a media file matches a rule's conditions
pub fn matches_rule(info: &MediaInfo, rule: &Rule) -> bool {
    if !rule.enabled {
        return false;
    }

    match &rule.normalized {
        Some(normalized) => matches_normalized(info, normalized),
        None => matches_conditions(info, &rule.match_conditions),
    }
}

/// Check if media info matches the given conditions
pub fn matches_conditions(info: &MediaInfo, conditions: &MatchConditions) -> bool {
    // All specified conditions must match (AND logic)
    // Empty conditions always match

    if !matches_codecs(info, &conditions.codecs) {
        return false;
    }

    if !matches_containers(info, &conditions.containers) {
        return false;
    }

    if !matches_hdr_formats(info, &conditions.hdr_formats) {
        return false;
    }

    if !matches_dv_profiles(info, &conditions.dolby_vision_profiles) {
        return false;
    }

    if !matches_resolution(
        info,
        conditions.min_resolution.as_ref(),
        conditions.max_resolution.as_ref(),
    ) {
        return false;
    }

    if !matches_audio_codecs(info, &conditions.audio_codecs) {
        return false;
    }

    true
}

fn matches_normalized(info: &MediaInfo, cond: &NormalizedMatchConditions) -> bool {
    matches_codecs_normalized(info, &cond.codecs)
        && matches_containers_normalized(info, &cond.containers)
        && matches_hdr_formats_normalized(info, &cond.hdr_formats)
        && matches_dv_profiles(info, &cond.dolby_vision_profiles)
        && matches_resolution(
            info,
            cond.min_resolution.as_ref(),
            cond.max_resolution.as_ref(),
        )
        && matches_audio_codecs_normalized(info, &cond.audio_codecs)
}

fn matches_codecs_normalized(info: &MediaInfo, codecs: &[String]) -> bool {
    if codecs.is_empty() {
        return true;
    }

    info.video_tracks.iter().any(|track| {
        let track_codec = track.codec.to_lowercase();
        codecs.contains(&track_codec)
    })
}

fn matches_containers_normalized(info: &MediaInfo, containers: &[String]) -> bool {
    if containers.is_empty() {
        return true;
    }

    let file_container = info.container.to_lowercase();
    let file_ext = info
        .file_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    containers
        .iter()
        .any(|c| file_container.contains(c) || file_ext.as_ref() == Some(c))
}

fn matches_hdr_formats_normalized(info: &MediaInfo, formats: &[HdrFormatMatch]) -> bool {
    if formats.is_empty() {
        return true;
    }

    info.video_tracks
        .iter()
        .any(|track| match &track.hdr_format {
            None => formats.contains(&HdrFormatMatch::Sdr),
            Some(hdr) => formats.iter().any(|f| hdr_matches(hdr, f)),
        })
}

fn hdr_matches(hdr: &HdrFormat, target: &HdrFormatMatch) -> bool {
    matches!(
        (hdr, target),
        (HdrFormat::Sdr, HdrFormatMatch::Sdr)
            | (HdrFormat::Hdr10, HdrFormatMatch::Hdr10)
            | (HdrFormat::Hdr10Plus, HdrFormatMatch::Hdr10Plus)
            | (HdrFormat::DolbyVision, HdrFormatMatch::DolbyVision)
            | (HdrFormat::Hlg, HdrFormatMatch::Hlg)
    )
}

fn matches_audio_codecs_normalized(info: &MediaInfo, codecs: &[String]) -> bool {
    if codecs.is_empty() {
        return true;
    }

    info.audio_tracks.iter().any(|track| {
        let track_codec = track.codec.to_lowercase();
        codecs.contains(&track_codec)
    })
}

fn matches_codecs(info: &MediaInfo, codecs: &[String]) -> bool {
    if codecs.is_empty() {
        return true;
    }

    info.video_tracks.iter().any(|track| {
        codecs
            .iter()
            .any(|c| track.codec.to_lowercase().contains(&c.to_lowercase()))
    })
}

fn matches_containers(info: &MediaInfo, containers: &[String]) -> bool {
    if containers.is_empty() {
        return true;
    }

    let file_container = info.container.to_lowercase();
    let file_path_str = info.file_path.display().to_string().to_lowercase();
    containers.iter().any(|c| {
        let c_lower = c.to_lowercase();
        file_container.contains(&c_lower) || file_path_str.ends_with(&format!(".{}", c_lower))
    })
}

fn matches_hdr_formats(info: &MediaInfo, formats: &[String]) -> bool {
    if formats.is_empty() {
        return true;
    }

    info.video_tracks.iter().any(|track| {
        if let Some(ref hdr) = track.hdr_format {
            let hdr_str = format!("{:?}", hdr).to_lowercase();
            formats.iter().any(|f| {
                let f_lower = f.to_lowercase();
                hdr_str.contains(&f_lower)
                    || match f_lower.as_str() {
                        "hdr10" => hdr_str == "hdr10",
                        "hdr10+" | "hdr10plus" => hdr_str == "hdr10plus",
                        "dolbyvision" | "dolby_vision" | "dv" => hdr_str == "dolbyvision",
                        "hlg" => hdr_str == "hlg",
                        _ => false,
                    }
            })
        } else {
            // Check if looking for SDR
            formats.iter().any(|f| f.to_lowercase() == "sdr")
        }
    })
}

fn matches_dv_profiles(info: &MediaInfo, profiles: &[u8]) -> bool {
    if profiles.is_empty() {
        return true;
    }

    info.video_tracks.iter().any(|track| {
        if let Some(ref dv) = track.dolby_vision {
            profiles.contains(&dv.profile)
        } else {
            false
        }
    })
}

fn matches_resolution(
    info: &MediaInfo,
    min_res: Option<&Resolution>,
    max_res: Option<&Resolution>,
) -> bool {
    let video = match info.primary_video() {
        Some(v) => v,
        None => return true, // No video track, skip resolution check
    };

    if let Some(min) = min_res {
        if video.width < min.width || video.height < min.height {
            return false;
        }
    }

    if let Some(max) = max_res {
        if video.width > max.width || video.height > max.height {
            return false;
        }
    }

    true
}

fn matches_audio_codecs(info: &MediaInfo, codecs: &[String]) -> bool {
    if codecs.is_empty() {
        return true;
    }

    // Check if ANY of the specified audio codecs is present
    info.audio_tracks.iter().any(|track| {
        codecs
            .iter()
            .any(|c| track.codec.to_lowercase().contains(&c.to_lowercase()))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::test_fixtures::make_test_info;

    #[test]
    fn test_matches_dv_profile() {
        let info = make_test_info();

        assert!(matches_dv_profiles(&info, &[7]));
        assert!(matches_dv_profiles(&info, &[7, 8]));
        assert!(!matches_dv_profiles(&info, &[8]));
        assert!(matches_dv_profiles(&info, &[])); // Empty matches all
    }

    #[test]
    fn test_matches_codec() {
        let info = make_test_info();

        assert!(matches_codecs(&info, &["hevc".to_string()]));
        assert!(matches_codecs(&info, &["HEVC".to_string()]));
        assert!(!matches_codecs(&info, &["h264".to_string()]));
    }

    #[test]
    fn test_matches_container() {
        let info = make_test_info();

        assert!(matches_containers(&info, &["mkv".to_string()]));
        assert!(matches_containers(&info, &["matroska".to_string()]));
        assert!(!matches_containers(&info, &["mp4".to_string()]));
    }
}
