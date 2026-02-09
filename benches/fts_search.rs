//! Benchmark FTS5 search performance at various dataset sizes.

use criterion::{criterion_group, criterion_main, Criterion};
use sf_db::pool::init_memory_pool;

fn seed_items(
    conn: &rusqlite::Connection,
    count: usize,
    lib_id: sf_core::LibraryId,
) {
    for i in 0..count {
        sf_db::queries::items::create_item(
            conn,
            lib_id,
            "movie",
            &format!("Movie Title Number {i} Searchable"),
            None,
            Some(2024),
            Some(&format!(
                "Overview for movie {i} with unique keywords alpha bravo charlie"
            )),
            Some(120),
            Some(7.5),
            None,
            None,
            None,
            None,
        )
        .expect("insert item");
    }
}

fn bench_search(c: &mut Criterion) {
    for &size in &[100, 1000, 5000] {
        let pool = init_memory_pool().expect("pool");
        let conn = pool.get().expect("conn");

        let lib = sf_db::queries::libraries::create_library(
            &conn,
            "Lib1",
            "movies",
            &[],
            &serde_json::json!({}),
        )
        .unwrap();
        let lib_id = lib.id;

        seed_items(&conn, size, lib_id);

        let mut group = c.benchmark_group(format!("search_{size}"));
        group.sample_size(50);

        // FTS5 prefix search (no filters).
        group.bench_function("fts_prefix", |b| {
            b.iter(|| {
                sf_db::queries::items::search_items_fts(&conn, "Searchable", None, None, 20)
                    .unwrap()
            });
        });

        // FTS5 with library_id filter.
        group.bench_function("fts_library_filter", |b| {
            b.iter(|| {
                sf_db::queries::items::search_items_fts(
                    &conn,
                    "Searchable",
                    Some(lib_id),
                    None,
                    20,
                )
                .unwrap()
            });
        });

        // FTS5 with item_kind filter.
        group.bench_function("fts_kind_filter", |b| {
            b.iter(|| {
                sf_db::queries::items::search_items_fts(
                    &conn,
                    "Searchable",
                    None,
                    Some("movie"),
                    20,
                )
                .unwrap()
            });
        });

        // LIKE fallback (search_items tries FTS then falls back).
        group.bench_function("like_fallback", |b| {
            b.iter(|| {
                sf_db::queries::items::search_items(&conn, "Searchable", 20).unwrap()
            });
        });

        group.finish();
    }
}

criterion_group!(benches, bench_search);
criterion_main!(benches);
