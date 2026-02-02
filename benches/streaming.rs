//! Benchmarks for streaming performance.
//!
//! Measures throughput and latency of HLS segment serving.

use bytes::Bytes;
use criterion::{black_box, criterion_group, criterion_main, Criterion, Throughput};
use futures::stream::{self, StreamExt};
use std::io::Cursor;
use tokio_util::io::ReaderStream;

/// Benchmark the old approach: buffer concatenation via extend_from_slice.
fn bench_buffer_concat(c: &mut Criterion) {
    let mut group = c.benchmark_group("segment_concat");

    // Simulate typical segment sizes
    for segment_size in [64 * 1024, 256 * 1024, 1024 * 1024, 4 * 1024 * 1024] {
        let header_size = 512; // Typical moof header size

        group.throughput(Throughput::Bytes(segment_size as u64));
        group.bench_function(format!("extend_from_slice_{}", segment_size), |b| {
            let header = vec![0u8; header_size];
            let segment_data = vec![0u8; segment_size];

            b.iter(|| {
                // Old approach: allocate, extend
                let mut response_data = header.clone();
                response_data.extend_from_slice(&segment_data);
                black_box(response_data)
            });
        });

        group.bench_function(format!("bytes_chain_{}", segment_size), |b| {
            let header = Bytes::from(vec![0u8; header_size]);
            let segment_data = Bytes::from(vec![0u8; segment_size]);

            b.iter(|| {
                // New approach: chain Bytes (zero-copy reference counting)
                let combined: Vec<Bytes> = vec![header.clone(), segment_data.clone()];
                black_box(combined)
            });
        });
    }

    group.finish();
}

/// Benchmark stream chaining overhead.
fn bench_stream_chain(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    let mut group = c.benchmark_group("stream_chain");

    for segment_size in [64 * 1024, 256 * 1024, 1024 * 1024] {
        let header_size = 512;

        group.throughput(Throughput::Bytes(segment_size as u64));

        group.bench_function(format!("collect_chained_{}", segment_size), |b| {
            let header = Bytes::from(vec![0u8; header_size]);
            let segment_data = vec![0u8; segment_size];

            b.iter(|| {
                rt.block_on(async {
                    let header_stream =
                        stream::once(async { Ok::<_, std::io::Error>(header.clone()) });
                    let data_stream = ReaderStream::new(Cursor::new(segment_data.clone()));

                    let combined = header_stream.chain(data_stream);

                    // Collect all chunks (simulates sending over network)
                    let chunks: Vec<_> = combined.collect().await;
                    black_box(chunks)
                })
            });
        });
    }

    group.finish();
}

/// Benchmark Bytes clone (reference counting) vs Vec clone (copy).
fn bench_bytes_vs_vec_clone(c: &mut Criterion) {
    let mut group = c.benchmark_group("clone_overhead");

    for size in [512, 4096, 65536, 262144] {
        group.throughput(Throughput::Bytes(size as u64));

        group.bench_function(format!("vec_clone_{}", size), |b| {
            let data = vec![0u8; size];
            b.iter(|| black_box(data.clone()));
        });

        group.bench_function(format!("bytes_clone_{}", size), |b| {
            let data = Bytes::from(vec![0u8; size]);
            b.iter(|| black_box(data.clone()));
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_buffer_concat,
    bench_stream_chain,
    bench_bytes_vs_vec_clone
);
criterion_main!(benches);
