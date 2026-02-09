//! Benchmark sf_parser::parse() throughput across release name complexity.

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_parser(c: &mut Criterion) {
    let inputs = [
        ("simple_movie", "The.Matrix.1999.1080p.BluRay.x264-GROUP"),
        (
            "4k_hdr",
            "Movie.2023.2160p.UHD.BluRay.Remux.HDR.DV.TrueHD.7.1.Atmos.HEVC-FraMeSToR",
        ),
        (
            "tv_episode",
            "Breaking.Bad.S01E01.720p.WEB-DL.DD5.1.H.264-DEMAND",
        ),
        (
            "multi_episode",
            "Show.S01E01E02.1080p.WEB-DL.x265-GROUP",
        ),
        (
            "long_complex",
            "The.Lord.of.the.Rings.The.Return.of.the.King.2003.EXTENDED.2160p.UHD.BluRay.x265.HDR10.DTS-HD.MA.6.1-SWTYBLZ",
        ),
    ];

    let mut group = c.benchmark_group("parser");
    for (name, input) in &inputs {
        group.bench_function(*name, |b| {
            b.iter(|| sf_parser::parse(black_box(input)));
        });
    }
    group.finish();
}

criterion_group!(benches, bench_parser);
criterion_main!(benches);
