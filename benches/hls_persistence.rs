//! Benchmark the HLS persistence pipeline: bincode serialization/deserialization
//! of PreparedMedia and the DB tier (set_hls_prepared / get_hls_prepared).
//!
//! Uses the Big Buck Bunny Profile B fixture to build a real PreparedMedia,
//! then measures bincode encode/decode and SQLite blob store/load performance.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::io::BufReader;
use std::path::Path;

fn fixture_path() -> String {
    let manifest = env!("CARGO_MANIFEST_DIR");
    format!("{manifest}/tests/fixtures/bbb_profile_b.mp4")
}

fn build_prepared() -> sf_media::PreparedMedia {
    let path = fixture_path();
    let mut reader = BufReader::new(std::fs::File::open(&path).unwrap());
    let metadata = sf_media::parse_moov(&mut reader).unwrap();
    sf_media::build_prepared_media(&metadata, Path::new(&path)).unwrap()
}

/// Create an in-memory DB and return a connection + a valid MediaFileId.
fn setup_db() -> (
    r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>,
    sf_core::MediaFileId,
) {
    let pool = sf_db::pool::init_memory_pool().expect("pool");
    let conn = pool.get().expect("conn");

    let lib = sf_db::queries::libraries::create_library(
        &conn,
        "Bench",
        "movies",
        &[],
        &serde_json::json!({}),
    )
    .unwrap();

    let item = sf_db::queries::items::create_item(
        &conn,
        lib.id,
        "movie",
        "Bench Item",
        None, None, None, None, None, None, None, None, None,
    )
    .unwrap();

    let mf = sf_db::queries::media_files::create_media_file(
        &conn,
        item.id,
        "/bench.mp4",
        "bench.mp4",
        1024,
        Some("mp4"),
        Some("h264"),
        Some("aac"),
        Some(640),
        Some(360),
        None,
        false,
        None,
        "source",
        "B",
        Some(24.0),
    )
    .unwrap();

    (conn, mf.id)
}

fn bench_hls_persistence(c: &mut Criterion) {
    let prepared = build_prepared();
    let bytes = prepared.to_bincode().unwrap();

    let mut group = c.benchmark_group("hls_persistence");

    // 1. Bincode serialize
    group.bench_function("bincode_serialize", |b| {
        b.iter(|| {
            black_box(&prepared).to_bincode().unwrap();
        });
    });

    // 2. Bincode deserialize
    group.bench_function("bincode_deserialize", |b| {
        b.iter(|| {
            sf_media::PreparedMedia::from_bincode(black_box(&bytes)).unwrap();
        });
    });

    // 3. Bincode round-trip (serialize then deserialize)
    group.bench_function("bincode_roundtrip", |b| {
        b.iter(|| {
            let encoded = black_box(&prepared).to_bincode().unwrap();
            sf_media::PreparedMedia::from_bincode(&encoded).unwrap();
        });
    });

    // 4. DB write
    {
        let (conn, mf_id) = setup_db();
        group.bench_function("db_write", |b| {
            b.iter(|| {
                sf_db::queries::media_files::set_hls_prepared(
                    &conn,
                    mf_id,
                    black_box(&bytes),
                )
                .unwrap();
            });
        });
    }

    // 5. DB read (pre-populate first)
    {
        let (conn, mf_id) = setup_db();
        sf_db::queries::media_files::set_hls_prepared(&conn, mf_id, &bytes).unwrap();
        group.bench_function("db_read", |b| {
            b.iter(|| {
                sf_db::queries::media_files::get_hls_prepared(&conn, black_box(mf_id)).unwrap();
            });
        });
    }

    // 6. Full DB round-trip (write blob, read it back, deserialize)
    {
        let (conn, mf_id) = setup_db();
        group.bench_function("db_full_roundtrip", |b| {
            b.iter(|| {
                let enc = black_box(&prepared).to_bincode().unwrap();
                sf_db::queries::media_files::set_hls_prepared(&conn, mf_id, &enc).unwrap();
                let blob = sf_db::queries::media_files::get_hls_prepared(&conn, mf_id)
                    .unwrap()
                    .expect("blob should exist");
                sf_media::PreparedMedia::from_bincode(&blob).unwrap();
            });
        });
    }

    group.finish();
}

criterion_group!(benches, bench_hls_persistence);
criterion_main!(benches);
