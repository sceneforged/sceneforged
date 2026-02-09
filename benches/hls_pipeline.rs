//! Benchmark the full HLS preparation pipeline against a real MP4 file.
//!
//! Uses the Big Buck Bunny Profile B fixture (24s, H.264 High, AAC stereo,
//! 640×360, keyframes every 2s) to measure real-world performance of:
//! - moov atom parsing
//! - segment map computation + fMP4 init/moof generation
//! - full pipeline (parse_moov + build_prepared_media)

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::io::BufReader;
use std::path::Path;

fn fixture_path() -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    format!("{manifest}/tests/fixtures/bbb_profile_b.mp4")
}

fn bench_hls_pipeline(c: &mut Criterion) {
    let path = fixture_path();
    let path_ref = Path::new(&path);

    // Pre-parse to get metadata for the build_prepared_media-only benchmark.
    let mut reader = BufReader::new(std::fs::File::open(&path).unwrap());
    let metadata = sf_media::parse_moov(&mut reader).unwrap();

    let mut group = c.benchmark_group("hls_pipeline");

    // Benchmark moov parsing only (I/O + atom traversal + sample table resolution).
    group.bench_function("parse_moov", |b| {
        b.iter(|| {
            let mut r = BufReader::new(std::fs::File::open(black_box(&path)).unwrap());
            sf_media::parse_moov(&mut r).unwrap()
        });
    });

    // Benchmark segment map + fMP4 generation only (CPU-bound, no I/O).
    group.bench_function("build_prepared_media", |b| {
        b.iter(|| {
            sf_media::build_prepared_media(black_box(&metadata), black_box(path_ref)).unwrap()
        });
    });

    // Benchmark full pipeline (parse_moov + build_prepared_media).
    group.bench_function("full_pipeline", |b| {
        b.iter(|| {
            let mut r = BufReader::new(std::fs::File::open(black_box(&path)).unwrap());
            let meta = sf_media::parse_moov(&mut r).unwrap();
            sf_media::build_prepared_media(&meta, path_ref).unwrap()
        });
    });

    group.finish();
}

criterion_group!(benches, bench_hls_pipeline);
criterion_main!(benches);
