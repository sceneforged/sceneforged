//! Benchmark sendfile::classify_peek() — runs on every TCP connection.

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use sf_server::sendfile::classify_peek;

fn make_request(method: &str, path: &str) -> Vec<u8> {
    format!("{method} {path} HTTP/1.1\r\nHost: localhost\r\n\r\n").into_bytes()
}

fn bench_classify(c: &mut Criterion) {
    let mut group = c.benchmark_group("classify_peek");

    let hls_segment = make_request(
        "GET",
        "/api/stream/550e8400-e29b-41d4-a716-446655440000/segment_5.m4s",
    );
    group.bench_function("hls_segment", |b| {
        b.iter(|| classify_peek(black_box(&hls_segment)));
    });

    let direct_stream = make_request(
        "GET",
        "/api/stream/550e8400-e29b-41d4-a716-446655440000/direct",
    );
    group.bench_function("direct_stream", |b| {
        b.iter(|| classify_peek(black_box(&direct_stream)));
    });

    let jellyfin_stream = make_request(
        "GET",
        "/Videos/550e8400-e29b-41d4-a716-446655440000/stream",
    );
    group.bench_function("jellyfin_stream", |b| {
        b.iter(|| classify_peek(black_box(&jellyfin_stream)));
    });

    let non_match = make_request("GET", "/api/items");
    group.bench_function("non_match_get", |b| {
        b.iter(|| classify_peek(black_box(&non_match)));
    });

    let post_early_exit = make_request("POST", "/api/auth/login");
    group.bench_function("post_early_exit", |b| {
        b.iter(|| classify_peek(black_box(&post_early_exit)));
    });

    group.finish();
}

criterion_group!(benches, bench_classify);
criterion_main!(benches);
