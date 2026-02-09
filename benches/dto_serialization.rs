//! Benchmark Jellyfin DTO conversion and serialization.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sf_db::pool::init_memory_pool;
use sf_server::routes::jellyfin::dto::{item_to_dto, ItemsResult};

fn make_test_item(conn: &rusqlite::Connection, lib_id: sf_core::LibraryId) -> sf_db::models::Item {
    sf_db::queries::items::create_item(
        conn,
        lib_id,
        "movie",
        "Benchmark Movie",
        None,
        Some(2024),
        Some("A movie used for benchmarking DTO serialization"),
        Some(120),
        Some(7.5),
        None,
        None,
        None,
        None,
    )
    .unwrap()
}

fn bench_dto(c: &mut Criterion) {
    let pool = init_memory_pool().expect("pool");
    let conn = pool.get().expect("conn");

    let lib = sf_db::queries::libraries::create_library(
        &conn,
        "Bench Movies",
        "movies",
        &[],
        &serde_json::json!({}),
    )
    .unwrap();

    let item = make_test_item(&conn, lib.id);
    let images: Vec<sf_db::models::Image> = vec![];

    let mut group = c.benchmark_group("dto");

    group.bench_function("item_to_dto", |b| {
        b.iter(|| item_to_dto(black_box(&item), black_box(&images), None));
    });

    let dto = item_to_dto(&item, &images, None);
    group.bench_function("serialize_base_item_dto", |b| {
        b.iter(|| serde_json::to_string(black_box(&dto)).unwrap());
    });

    // Build a 50-item response payload.
    let mut items = Vec::with_capacity(50);
    for _ in 0..50 {
        let it = make_test_item(&conn, lib.id);
        items.push(item_to_dto(&it, &images, None));
    }
    let result = ItemsResult {
        items,
        total_record_count: 50,
    };

    group.bench_function("serialize_items_result_50", |b| {
        b.iter(|| serde_json::to_string(black_box(&result)).unwrap());
    });

    group.finish();
}

criterion_group!(benches, bench_dto);
criterion_main!(benches);
