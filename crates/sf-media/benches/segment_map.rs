//! Benchmark compute_segment_map() with varying keyframe counts.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sf_media::segment_map::{compute_segment_map, KeyframeInfo};

fn make_keyframes(count: usize, interval: f64) -> Vec<KeyframeInfo> {
    (0..count)
        .map(|i| KeyframeInfo {
            timestamp: i as f64 * interval,
            byte_offset: i as u64 * 500_000,
        })
        .collect()
}

fn bench_segment_map(c: &mut Criterion) {
    let mut group = c.benchmark_group("segment_map");

    // 5 minutes: ~75 keyframes at 4s intervals.
    let kf_5min = make_keyframes(75, 4.0);
    group.bench_function("5min_75kf", |b| {
        b.iter(|| compute_segment_map(black_box(&kf_5min), 300.0, 6.0));
    });

    // 30 minutes: ~450 keyframes.
    let kf_30min = make_keyframes(450, 4.0);
    group.bench_function("30min_450kf", |b| {
        b.iter(|| compute_segment_map(black_box(&kf_30min), 1800.0, 6.0));
    });

    // 2 hours: ~1800 keyframes.
    let kf_2hr = make_keyframes(1800, 4.0);
    group.bench_function("2hr_1800kf", |b| {
        b.iter(|| compute_segment_map(black_box(&kf_2hr), 7200.0, 6.0));
    });

    group.finish();
}

criterion_group!(benches, bench_segment_map);
criterion_main!(benches);
