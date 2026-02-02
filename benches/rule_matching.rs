//! Benchmarks for rule matching
//!
//! Tests performance of matching media files against rules.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use sceneforged::config::{Action, MatchConditions, Resolution, Rule};
use sceneforged::probe::{AudioTrack, DolbyVisionInfo, HdrFormat, MediaInfo, VideoTrack};
use sceneforged::rules::{find_all_matching_rules, find_matching_rule, matches_rule};
use std::path::PathBuf;
use std::time::Duration;

/// Create a simple SDR media file
fn simple_sdr_media() -> MediaInfo {
    MediaInfo {
        file_path: PathBuf::from("/movies/movie.mkv"),
        file_size: 4 * 1024 * 1024 * 1024,
        container: "Matroska".to_string(),
        duration: Some(Duration::from_millis(7200000)),
        video_tracks: vec![VideoTrack {
            index: 0,
            codec: "AVC".to_string(),
            width: 1920,
            height: 1080,
            frame_rate: Some(23.976),
            bit_depth: Some(8),
            hdr_format: None,
            dolby_vision: None,
        }],
        audio_tracks: vec![AudioTrack {
            index: 1,
            codec: "AAC".to_string(),
            channels: 2,
            sample_rate: Some(48000),
            language: Some("eng".to_string()),
            title: None,
            default: true,
            atmos: false,
        }],
        subtitle_tracks: vec![],
    }
}

/// Create a complex 4K HDR media file with Dolby Vision
fn complex_dv_media() -> MediaInfo {
    MediaInfo {
        file_path: PathBuf::from("/movies/dv_movie.mkv"),
        file_size: 50 * 1024 * 1024 * 1024,
        container: "Matroska".to_string(),
        duration: Some(Duration::from_millis(9000000)),
        video_tracks: vec![VideoTrack {
            index: 0,
            codec: "HEVC".to_string(),
            width: 3840,
            height: 2160,
            frame_rate: Some(23.976),
            bit_depth: Some(10),
            hdr_format: Some(HdrFormat::DolbyVision),
            dolby_vision: Some(DolbyVisionInfo {
                profile: 7,
                level: Some(6),
                rpu_present: true,
                el_present: true,
                bl_present: true,
                bl_compatibility_id: Some(1),
            }),
        }],
        audio_tracks: vec![
            AudioTrack {
                index: 1,
                codec: "TrueHD".to_string(),
                channels: 8,
                sample_rate: Some(48000),
                language: Some("eng".to_string()),
                title: Some("English - Atmos".to_string()),
                default: true,
                atmos: true,
            },
            AudioTrack {
                index: 2,
                codec: "AC3".to_string(),
                channels: 6,
                sample_rate: Some(48000),
                language: Some("eng".to_string()),
                title: Some("English - Compatibility".to_string()),
                default: false,
                atmos: false,
            },
        ],
        subtitle_tracks: vec![],
    }
}

/// Create a rule that matches HEVC
fn hevc_rule() -> Rule {
    Rule {
        name: "HEVC Remux".to_string(),
        enabled: true,
        priority: 100,
        match_conditions: MatchConditions {
            codecs: vec!["hevc".to_string(), "h265".to_string()],
            ..Default::default()
        },
        actions: vec![Action::Remux {
            container: "mkv".to_string(),
            keep_original: false,
        }],
        normalized: None,
    }
}

/// Create a rule that matches Dolby Vision profile 7
fn dv_profile7_rule() -> Rule {
    Rule {
        name: "DV P7 Convert".to_string(),
        enabled: true,
        priority: 200,
        match_conditions: MatchConditions {
            dolby_vision_profiles: vec![7],
            ..Default::default()
        },
        actions: vec![Action::DvConvert { target_profile: 8 }],
        normalized: None,
    }
}

/// Create a rule that matches 4K content
fn resolution_4k_rule() -> Rule {
    Rule {
        name: "4K Processing".to_string(),
        enabled: true,
        priority: 50,
        match_conditions: MatchConditions {
            min_resolution: Some(Resolution {
                width: 3840,
                height: 2160,
            }),
            ..Default::default()
        },
        actions: vec![Action::Remux {
            container: "mkv".to_string(),
            keep_original: true,
        }],
        normalized: None,
    }
}

/// Create a complex rule with multiple conditions
fn complex_rule() -> Rule {
    Rule {
        name: "Complex Rule".to_string(),
        enabled: true,
        priority: 150,
        match_conditions: MatchConditions {
            codecs: vec!["hevc".to_string()],
            containers: vec!["matroska".to_string(), "mkv".to_string()],
            hdr_formats: vec!["dolby_vision".to_string()],
            dolby_vision_profiles: vec![7, 8],
            min_resolution: Some(Resolution {
                width: 1920,
                height: 1080,
            }),
            audio_codecs: vec!["truehd".to_string(), "dts-hd ma".to_string()],
            ..Default::default()
        },
        actions: vec![
            Action::DvConvert { target_profile: 8 },
            Action::AddCompatAudio {
                source_codec: "truehd".to_string(),
                target_codec: "ac3".to_string(),
            },
        ],
        normalized: None,
    }
}

/// Create a rule that won't match anything
fn non_matching_rule() -> Rule {
    Rule {
        name: "VP9 Only".to_string(),
        enabled: true,
        priority: 10,
        match_conditions: MatchConditions {
            codecs: vec!["vp9".to_string()],
            ..Default::default()
        },
        actions: vec![Action::Remux {
            container: "webm".to_string(),
            keep_original: false,
        }],
        normalized: None,
    }
}

fn bench_single_rule_matching(c: &mut Criterion) {
    let mut group = c.benchmark_group("single_rule_matching");

    let sdr_media = simple_sdr_media();
    let dv_media = complex_dv_media();
    let hevc_rule = hevc_rule();
    let dv_rule = dv_profile7_rule();
    let complex_rule = complex_rule();

    // Simple match (codec only)
    group.bench_function("codec_match/hit", |b| {
        b.iter(|| matches_rule(black_box(&dv_media), black_box(&hevc_rule)));
    });

    group.bench_function("codec_match/miss", |b| {
        b.iter(|| matches_rule(black_box(&sdr_media), black_box(&hevc_rule)));
    });

    // DV profile match
    group.bench_function("dv_profile_match/hit", |b| {
        b.iter(|| matches_rule(black_box(&dv_media), black_box(&dv_rule)));
    });

    group.bench_function("dv_profile_match/miss", |b| {
        b.iter(|| matches_rule(black_box(&sdr_media), black_box(&dv_rule)));
    });

    // Complex rule with many conditions
    group.bench_function("complex_rule/hit", |b| {
        b.iter(|| matches_rule(black_box(&dv_media), black_box(&complex_rule)));
    });

    group.bench_function("complex_rule/miss", |b| {
        b.iter(|| matches_rule(black_box(&sdr_media), black_box(&complex_rule)));
    });

    group.finish();
}

fn bench_find_matching_rule(c: &mut Criterion) {
    let mut group = c.benchmark_group("find_matching_rule");

    let dv_media = complex_dv_media();
    let sdr_media = simple_sdr_media();

    // Create rule sets of various sizes
    let small_rules: Vec<Rule> = vec![hevc_rule(), dv_profile7_rule(), resolution_4k_rule()];

    let medium_rules: Vec<Rule> = (0..10)
        .map(|i| Rule {
            name: format!("Rule {}", i),
            enabled: true,
            priority: i * 10,
            match_conditions: MatchConditions {
                codecs: vec![format!("codec{}", i)],
                ..Default::default()
            },
            actions: vec![Action::Remux {
                container: "mkv".to_string(),
                keep_original: false,
            }],
            normalized: None,
        })
        .chain(vec![hevc_rule(), dv_profile7_rule()])
        .collect();

    let large_rules: Vec<Rule> = (0..50)
        .map(|i| Rule {
            name: format!("Rule {}", i),
            enabled: true,
            priority: i * 10,
            match_conditions: MatchConditions {
                codecs: vec![format!("codec{}", i)],
                ..Default::default()
            },
            actions: vec![Action::Remux {
                container: "mkv".to_string(),
                keep_original: false,
            }],
            normalized: None,
        })
        .chain(vec![hevc_rule(), dv_profile7_rule()])
        .collect();

    // Benchmark finding first match
    group.bench_with_input(
        BenchmarkId::new("first_match", "3_rules"),
        &(&dv_media, &small_rules),
        |b, (media, rules)| {
            b.iter(|| find_matching_rule(black_box(media), black_box(rules)));
        },
    );

    group.bench_with_input(
        BenchmarkId::new("first_match", "12_rules"),
        &(&dv_media, &medium_rules),
        |b, (media, rules)| {
            b.iter(|| find_matching_rule(black_box(media), black_box(rules)));
        },
    );

    group.bench_with_input(
        BenchmarkId::new("first_match", "52_rules"),
        &(&dv_media, &large_rules),
        |b, (media, rules)| {
            b.iter(|| find_matching_rule(black_box(media), black_box(rules)));
        },
    );

    // Benchmark no match (worst case - checks all rules)
    let no_match_rules: Vec<Rule> = (0..50).map(|_| non_matching_rule()).collect();

    group.bench_with_input(
        BenchmarkId::new("no_match", "50_rules"),
        &(&sdr_media, &no_match_rules),
        |b, (media, rules)| {
            b.iter(|| find_matching_rule(black_box(media), black_box(rules)));
        },
    );

    group.finish();
}

fn bench_find_all_matching_rules(c: &mut Criterion) {
    let mut group = c.benchmark_group("find_all_matching_rules");

    let dv_media = complex_dv_media();

    // Rules where multiple match
    let multi_match_rules: Vec<Rule> = vec![
        hevc_rule(),
        dv_profile7_rule(),
        resolution_4k_rule(),
        complex_rule(),
        non_matching_rule(),
    ];

    group.bench_function("multiple_matches/5_rules", |b| {
        b.iter(|| find_all_matching_rules(black_box(&dv_media), black_box(&multi_match_rules)));
    });

    // Larger set with multiple matches
    let large_multi_match: Vec<Rule> = (0..20)
        .map(|i| {
            if i % 5 == 0 {
                hevc_rule()
            } else {
                non_matching_rule()
            }
        })
        .collect();

    group.bench_function("multiple_matches/20_rules", |b| {
        b.iter(|| find_all_matching_rules(black_box(&dv_media), black_box(&large_multi_match)));
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_single_rule_matching,
    bench_find_matching_rule,
    bench_find_all_matching_rules
);
criterion_main!(benches);
