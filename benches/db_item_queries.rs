//! Benchmark core DB item queries with a 1000-item dataset.

use criterion::{criterion_group, criterion_main, Criterion};
use sf_db::pool::init_memory_pool;

fn setup() -> (
    r2d2::PooledConnection<r2d2_sqlite::SqliteConnectionManager>,
    sf_core::LibraryId,
    sf_core::ItemId,
    sf_core::ItemId, // parent (season) for children query
) {
    let pool = init_memory_pool().expect("pool");
    let conn = pool.get().expect("conn");

    let lib = sf_db::queries::libraries::create_library(
        &conn,
        "Bench",
        "movies",
        &[],
        &serde_json::json!({}),
    )
    .unwrap();
    let lib_id = lib.id;

    // Insert 1000 items using the DB API.
    let mut first_id = None;
    for i in 0..1000 {
        let item = sf_db::queries::items::create_item(
            &conn,
            lib_id,
            "movie",
            &format!("Item {i:04}"),
            None,
            Some(2024),
            Some("Overview text"),
            Some(120),
            Some(7.5),
            None,
            None,
            None,
            None,
        )
        .unwrap();
        if i == 0 {
            first_id = Some(item.id);
        }
    }

    // Create a season with 20 episodes for list_children bench.
    let season = sf_db::queries::items::create_item(
        &conn,
        lib_id,
        "season",
        "Season 1",
        None,
        None,
        None,
        None,
        None,
        None,
        None,
        Some(1),
        None,
    )
    .unwrap();

    for ep in 1..=20 {
        sf_db::queries::items::create_item(
            &conn,
            lib_id,
            "episode",
            &format!("Episode {ep}"),
            None,
            None,
            None,
            Some(45),
            None,
            None,
            Some(season.id),
            Some(1),
            Some(ep),
        )
        .unwrap();
    }

    (conn, lib_id, first_id.unwrap(), season.id)
}

fn bench_db_queries(c: &mut Criterion) {
    let (conn, lib_id, item_id, season_id) = setup();

    let mut group = c.benchmark_group("db_items");

    group.bench_function("get_item_by_id", |b| {
        b.iter(|| {
            sf_db::queries::items::get_item(&conn, item_id).unwrap();
        });
    });

    group.bench_function("list_items_by_library", |b| {
        b.iter(|| {
            sf_db::queries::items::list_items_by_library(&conn, lib_id, 0, 50).unwrap();
        });
    });

    group.bench_function("list_children_ordered", |b| {
        b.iter(|| {
            sf_db::queries::items::list_children(&conn, season_id).unwrap();
        });
    });

    group.bench_function("count_items_by_library", |b| {
        b.iter(|| {
            sf_db::queries::items::count_items_by_library(&conn, lib_id).unwrap();
        });
    });

    group.finish();
}

criterion_group!(benches, bench_db_queries);
criterion_main!(benches);
