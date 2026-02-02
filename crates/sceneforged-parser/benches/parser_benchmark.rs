//! Benchmarks for sceneforged-parser.
//!
//! Run with: cargo bench

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use sceneforged_parser::parse;

/// Sample release names for benchmarking
const MOVIE_SAMPLES: &[&str] = &[
    "The.Matrix.1999.1080p.BluRay.x264-GROUP",
    "Inception.2010.2160p.UHD.BluRay.x265.HDR.DTS-HD.MA.5.1-RELEASE",
    "The.Dark.Knight.2008.PROPER.720p.BluRay.x264.DTS-WiKi",
    "Interstellar.2014.IMAX.2160p.UHD.BluRay.REMUX.HDR.HEVC.TrueHD.7.1.Atmos-FGT",
    "Pulp.Fiction.1994.REMASTERED.1080p.BluRay.x264.DTS-HD.MA.5.1-SWTYBLZ",
];

const TV_SAMPLES: &[&str] = &[
    "Breaking.Bad.S01E01.720p.BluRay.x264-DEMAND",
    "Game.of.Thrones.S08E06.1080p.WEB-DL.DD5.1.H.264-GoT",
    "The.Office.US.S02E01E02.720p.BluRay.x264-DEMAND",
    "Stranger.Things.S04E09.Chapter.Nine.The.Piggyback.2160p.NF.WEB-DL.DDP5.1.Atmos.DV.HDR.H.265-FLUX",
    "House.of.the.Dragon.S01E10.The.Black.Queen.1080p.HMAX.WEB-DL.DDP5.1.Atmos.H.264-CMRG",
];

const ANIME_SAMPLES: &[&str] = &[
    "[SubsPlease] Jujutsu Kaisen - 24 (1080p) [ABCD1234].mkv",
    "[Judas] Chainsaw Man - S01E12 [1080p][HEVC x265 10bit][Dual-Audio].mkv",
    "[Erai-raws] Spy x Family - 25 [1080p][Multiple Subtitle].mkv",
    "[SubGroup] Attack on Titan - The Final Season - 28 [1080p] [ENG].mkv",
    "[HorribleSubs] My Hero Academia - 88 [720p].mkv",
];

const COMPLEX_SAMPLES: &[&str] = &[
    "The.Lord.of.the.Rings.The.Fellowship.of.the.Ring.2001.EXTENDED.2160p.UHD.BluRay.REMUX.HDR.HEVC.TrueHD.7.1.Atmos-FGT",
    "Star.Wars.Episode.IV.A.New.Hope.1977.REMASTERED.2160p.UHD.BluRay.x265.10bit.HDR.TrueHD.7.1.Atmos-SWTYBLZ",
    "[Commie] Steins;Gate - Fuka Ryouiki no Deja vu (BD 1080p AAC) [ABCD1234].mkv",
    "Marvel's.Agents.of.S.H.I.E.L.D.S07E13.What.We're.Fighting.For.1080p.AMZN.WEB-DL.DDP5.1.H.264-T6D",
];

fn bench_parse_single(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_single");

    // Simple movie
    group.bench_function("simple_movie", |b| {
        b.iter(|| parse(black_box("The.Matrix.1999.1080p.BluRay.x264-GROUP")))
    });

    // Complex movie with many attributes
    group.bench_function("complex_movie", |b| {
        b.iter(|| {
            parse(black_box(
                "Inception.2010.2160p.UHD.BluRay.x265.HDR.DTS-HD.MA.5.1-RELEASE",
            ))
        })
    });

    // TV episode
    group.bench_function("tv_episode", |b| {
        b.iter(|| parse(black_box("Breaking.Bad.S01E01.720p.BluRay.x264-DEMAND")))
    });

    // Anime with brackets
    group.bench_function("anime", |b| {
        b.iter(|| {
            parse(black_box(
                "[SubsPlease] Jujutsu Kaisen - 24 (1080p) [ABCD1234].mkv",
            ))
        })
    });

    group.finish();
}

fn bench_parse_batch(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_batch");

    // Movies
    group.throughput(Throughput::Elements(MOVIE_SAMPLES.len() as u64));
    group.bench_function("movies", |b| {
        b.iter(|| {
            for sample in MOVIE_SAMPLES {
                black_box(parse(black_box(sample)));
            }
        })
    });

    // TV shows
    group.throughput(Throughput::Elements(TV_SAMPLES.len() as u64));
    group.bench_function("tv_episodes", |b| {
        b.iter(|| {
            for sample in TV_SAMPLES {
                black_box(parse(black_box(sample)));
            }
        })
    });

    // Anime
    group.throughput(Throughput::Elements(ANIME_SAMPLES.len() as u64));
    group.bench_function("anime", |b| {
        b.iter(|| {
            for sample in ANIME_SAMPLES {
                black_box(parse(black_box(sample)));
            }
        })
    });

    // Complex
    group.throughput(Throughput::Elements(COMPLEX_SAMPLES.len() as u64));
    group.bench_function("complex", |b| {
        b.iter(|| {
            for sample in COMPLEX_SAMPLES {
                black_box(parse(black_box(sample)));
            }
        })
    });

    group.finish();
}

fn bench_input_length(c: &mut Criterion) {
    let mut group = c.benchmark_group("input_length");

    // Various input lengths
    let inputs = [
        ("short", "Movie.2020.720p"),
        ("medium", "The.Matrix.1999.1080p.BluRay.x264-GROUP"),
        (
            "long",
            "The.Lord.of.the.Rings.The.Fellowship.of.the.Ring.2001.EXTENDED.2160p.BluRay.x265-GROUP",
        ),
        (
            "very_long",
            "Marvel's.Agents.of.S.H.I.E.L.D.S07E13.What.We're.Fighting.For.1080p.AMZN.WEB-DL.DDP5.1.Atmos.H.264.PROPER.REPACK-T6D",
        ),
    ];

    for (name, input) in inputs {
        group.throughput(Throughput::Bytes(input.len() as u64));
        group.bench_with_input(BenchmarkId::new("parse", name), input, |b, input| {
            b.iter(|| parse(black_box(input)))
        });
    }

    group.finish();
}

fn bench_all_samples(c: &mut Criterion) {
    let all_samples: Vec<&str> = MOVIE_SAMPLES
        .iter()
        .chain(TV_SAMPLES.iter())
        .chain(ANIME_SAMPLES.iter())
        .chain(COMPLEX_SAMPLES.iter())
        .copied()
        .collect();

    let mut group = c.benchmark_group("throughput");
    group.throughput(Throughput::Elements(all_samples.len() as u64));

    group.bench_function("all_samples", |b| {
        b.iter(|| {
            for sample in &all_samples {
                black_box(parse(black_box(sample)));
            }
        })
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_parse_single,
    bench_parse_batch,
    bench_input_length,
    bench_all_samples,
);

criterion_main!(benches);
