//! Benchmarks for probe output parsing
//!
//! Tests JSON deserialization performance for ffprobe and mediainfo output.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use sceneforged::probe::{
    AudioTrack, DolbyVisionInfo, HdrFormat, MediaInfo, SubtitleTrack, VideoTrack,
};
use std::path::PathBuf;
use std::time::Duration;

/// Sample ffprobe JSON output for a simple file
const FFPROBE_SIMPLE: &str = r#"{
    "format": {
        "filename": "/movies/movie.mkv",
        "format_name": "matroska,webm",
        "duration": "7200.000000",
        "size": "15000000000"
    },
    "streams": [
        {
            "index": 0,
            "codec_type": "video",
            "codec_name": "hevc",
            "width": 3840,
            "height": 2160,
            "r_frame_rate": "24000/1001",
            "disposition": {"default": 1, "forced": 0},
            "tags": {},
            "side_data_list": []
        },
        {
            "index": 1,
            "codec_type": "audio",
            "codec_name": "truehd",
            "channels": 8,
            "sample_rate": "48000",
            "disposition": {"default": 1, "forced": 0},
            "tags": {"language": "eng", "title": "TrueHD 7.1"}
        }
    ]
}"#;

/// Sample ffprobe JSON output for a complex multi-track file
const FFPROBE_COMPLEX: &str = r#"{
    "format": {
        "filename": "/movies/complex_movie.mkv",
        "format_name": "matroska,webm",
        "duration": "9000.000000",
        "size": "45000000000"
    },
    "streams": [
        {
            "index": 0,
            "codec_type": "video",
            "codec_name": "hevc",
            "width": 3840,
            "height": 2160,
            "r_frame_rate": "24000/1001",
            "disposition": {"default": 1, "forced": 0},
            "tags": {},
            "side_data_list": [{"side_data_type": "DOVI configuration record"}]
        },
        {
            "index": 1,
            "codec_type": "audio",
            "codec_name": "truehd",
            "channels": 8,
            "sample_rate": "48000",
            "disposition": {"default": 1, "forced": 0},
            "tags": {"language": "eng", "title": "English - Atmos"}
        },
        {
            "index": 2,
            "codec_type": "audio",
            "codec_name": "ac3",
            "channels": 6,
            "sample_rate": "48000",
            "disposition": {"default": 0, "forced": 0},
            "tags": {"language": "eng", "title": "English - Compatibility"}
        },
        {
            "index": 3,
            "codec_type": "audio",
            "codec_name": "dts",
            "channels": 6,
            "sample_rate": "48000",
            "disposition": {"default": 0, "forced": 0},
            "tags": {"language": "spa", "title": "Spanish"}
        },
        {
            "index": 4,
            "codec_type": "audio",
            "codec_name": "aac",
            "channels": 2,
            "sample_rate": "48000",
            "disposition": {"default": 0, "forced": 0},
            "tags": {"language": "jpn", "title": "Japanese"}
        },
        {
            "index": 5,
            "codec_type": "subtitle",
            "codec_name": "subrip",
            "disposition": {"default": 1, "forced": 0},
            "tags": {"language": "eng", "title": "English"}
        },
        {
            "index": 6,
            "codec_type": "subtitle",
            "codec_name": "subrip",
            "disposition": {"default": 0, "forced": 1},
            "tags": {"language": "eng", "title": "English (Forced)"}
        },
        {
            "index": 7,
            "codec_type": "subtitle",
            "codec_name": "subrip",
            "disposition": {"default": 0, "forced": 0},
            "tags": {"language": "spa", "title": "Spanish"}
        },
        {
            "index": 8,
            "codec_type": "subtitle",
            "codec_name": "hdmv_pgs_subtitle",
            "disposition": {"default": 0, "forced": 0},
            "tags": {"language": "jpn", "title": "Japanese"}
        }
    ]
}"#;

/// Intermediate struct matching ffprobe output for benchmarking JSON parsing
#[derive(serde::Deserialize)]
struct FfprobeOutput {
    format: FfprobeFormat,
    streams: Vec<FfprobeStream>,
}

#[derive(serde::Deserialize)]
struct FfprobeFormat {
    #[allow(dead_code)]
    filename: String,
    format_name: String,
    duration: Option<String>,
    size: Option<String>,
}

#[derive(serde::Deserialize)]
struct FfprobeStream {
    #[allow(dead_code)]
    index: u32,
    codec_type: String,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    r_frame_rate: Option<String>,
    channels: Option<u32>,
    sample_rate: Option<String>,
    #[serde(default)]
    disposition: FfprobeDisposition,
    #[serde(default)]
    tags: FfprobeTags,
    #[serde(default)]
    side_data_list: Vec<FfprobeSideData>,
}

#[derive(Default, serde::Deserialize)]
struct FfprobeDisposition {
    #[serde(default)]
    default: u8,
    #[serde(default)]
    forced: u8,
}

#[derive(Default, serde::Deserialize)]
struct FfprobeTags {
    language: Option<String>,
    title: Option<String>,
}

#[derive(serde::Deserialize)]
struct FfprobeSideData {
    side_data_type: Option<String>,
}

/// Parse ffprobe output into MediaInfo
fn parse_ffprobe_to_mediainfo(json: &str) -> MediaInfo {
    let output: FfprobeOutput = serde_json::from_str(json).unwrap();

    let mut info = MediaInfo {
        file_path: PathBuf::from("/test/file.mkv"),
        file_size: output.format.size.and_then(|s| s.parse().ok()).unwrap_or(0),
        container: output.format.format_name,
        duration: output
            .format
            .duration
            .and_then(|s| s.parse::<f64>().ok())
            .map(Duration::from_secs_f64),
        video_tracks: Vec::new(),
        audio_tracks: Vec::new(),
        subtitle_tracks: Vec::new(),
    };

    let mut video_index = 0u32;
    let mut audio_index = 0u32;
    let mut subtitle_index = 0u32;

    for stream in output.streams {
        match stream.codec_type.as_str() {
            "video" => {
                let has_dovi = stream
                    .side_data_list
                    .iter()
                    .any(|sd| sd.side_data_type.as_deref() == Some("DOVI configuration record"));

                info.video_tracks.push(VideoTrack {
                    index: video_index,
                    codec: stream.codec_name.unwrap_or_default(),
                    width: stream.width.unwrap_or(0),
                    height: stream.height.unwrap_or(0),
                    frame_rate: stream.r_frame_rate.and_then(|s| parse_frame_rate(&s)),
                    bit_depth: None,
                    hdr_format: if has_dovi {
                        Some(HdrFormat::DolbyVision)
                    } else {
                        None
                    },
                    dolby_vision: if has_dovi {
                        Some(DolbyVisionInfo {
                            profile: 0,
                            level: None,
                            rpu_present: true,
                            el_present: false,
                            bl_present: true,
                            bl_compatibility_id: None,
                        })
                    } else {
                        None
                    },
                });
                video_index += 1;
            }
            "audio" => {
                info.audio_tracks.push(AudioTrack {
                    index: audio_index,
                    codec: stream.codec_name.unwrap_or_default(),
                    channels: stream.channels.unwrap_or(2),
                    sample_rate: stream.sample_rate.and_then(|s| s.parse().ok()),
                    language: stream.tags.language,
                    title: stream.tags.title,
                    default: stream.disposition.default == 1,
                    atmos: false,
                });
                audio_index += 1;
            }
            "subtitle" => {
                info.subtitle_tracks.push(SubtitleTrack {
                    index: subtitle_index,
                    codec: stream.codec_name.unwrap_or_default(),
                    language: stream.tags.language,
                    title: stream.tags.title,
                    default: stream.disposition.default == 1,
                    forced: stream.disposition.forced == 1,
                });
                subtitle_index += 1;
            }
            _ => {}
        }
    }

    info
}

fn parse_frame_rate(rate_str: &str) -> Option<f64> {
    let parts: Vec<&str> = rate_str.split('/').collect();
    if parts.len() == 2 {
        let num: f64 = parts[0].parse().ok()?;
        let den: f64 = parts[1].parse().ok()?;
        if den != 0.0 {
            return Some(num / den);
        }
    }
    rate_str.parse().ok()
}

fn bench_json_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_parsing");

    // Benchmark simple file parsing
    group.throughput(Throughput::Bytes(FFPROBE_SIMPLE.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("ffprobe", "simple"),
        &FFPROBE_SIMPLE,
        |b, json| {
            b.iter(|| {
                let _: FfprobeOutput = serde_json::from_str(black_box(json)).unwrap();
            });
        },
    );

    // Benchmark complex file parsing
    group.throughput(Throughput::Bytes(FFPROBE_COMPLEX.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("ffprobe", "complex"),
        &FFPROBE_COMPLEX,
        |b, json| {
            b.iter(|| {
                let _: FfprobeOutput = serde_json::from_str(black_box(json)).unwrap();
            });
        },
    );

    group.finish();
}

fn bench_full_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("full_parsing");

    // Benchmark full parse to MediaInfo (simple)
    group.bench_with_input(
        BenchmarkId::new("to_mediainfo", "simple"),
        &FFPROBE_SIMPLE,
        |b, json| {
            b.iter(|| parse_ffprobe_to_mediainfo(black_box(json)));
        },
    );

    // Benchmark full parse to MediaInfo (complex)
    group.bench_with_input(
        BenchmarkId::new("to_mediainfo", "complex"),
        &FFPROBE_COMPLEX,
        |b, json| {
            b.iter(|| parse_ffprobe_to_mediainfo(black_box(json)));
        },
    );

    group.finish();
}

fn bench_mediainfo_helpers(c: &mut Criterion) {
    let mut group = c.benchmark_group("mediainfo_helpers");

    let simple_info = parse_ffprobe_to_mediainfo(FFPROBE_SIMPLE);
    let complex_info = parse_ffprobe_to_mediainfo(FFPROBE_COMPLEX);

    // Benchmark helper methods
    group.bench_function("has_dolby_vision/simple", |b| {
        b.iter(|| black_box(&simple_info).has_dolby_vision());
    });

    group.bench_function("has_dolby_vision/complex", |b| {
        b.iter(|| black_box(&complex_info).has_dolby_vision());
    });

    group.bench_function("resolution_name/4k", |b| {
        b.iter(|| black_box(&simple_info).resolution_name());
    });

    group.bench_function("primary_video", |b| {
        b.iter(|| black_box(&simple_info).primary_video());
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_json_parsing,
    bench_full_parsing,
    bench_mediainfo_helpers
);
criterion_main!(benches);
