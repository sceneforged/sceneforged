//! Benchmarks for template substitution
//!
//! Tests performance of variable substitution in command templates.

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use sceneforged::pipeline::TemplateContext;
use std::path::Path;

/// Simple template with one variable
const TEMPLATE_SIMPLE: &str = "ffmpeg -i {input} -c copy {output}";

/// Medium complexity template
const TEMPLATE_MEDIUM: &str =
    "ffmpeg -i {input} -map 0:v -map 0:a -c:v copy -c:a aac -b:a 256k {output}";

/// Complex template with many variables
const TEMPLATE_COMPLEX: &str = "ffmpeg -i {input} -map 0:v:0 -map 0:a:0 -c:v libx265 \
    -preset medium -crf 18 -c:a aac -b:a 320k \
    -metadata title=\"{filestem}\" \
    -metadata comment=\"Processed by sceneforged\" \
    {temp_dir}/intermediate.mkv && \
    mkvmerge -o {output} {temp_dir}/intermediate.mkv";

/// Template with all supported variables
const TEMPLATE_ALL_VARS: &str = "{input} {output} {temp_dir} {filename} {filestem} {extension} \
    {parent_dir} {input} {output} {temp_dir} {filename} {filestem} {extension} {parent_dir}";

/// Template with no variables (baseline)
const TEMPLATE_NO_VARS: &str = "ffmpeg -i input.mkv -c:v copy -c:a copy -map 0 output.mkv";

/// Template with custom variables
const TEMPLATE_CUSTOM: &str =
    "dovi_tool convert -i {input} --discard {dv_mode} -o {output} && echo {custom_var}";

fn create_context() -> TemplateContext {
    TemplateContext::new().with_workspace(
        Path::new("/media/movies/My Movie (2024)/My.Movie.2024.2160p.UHD.BluRay.x265-GROUP.mkv"),
        Path::new(
            "/media/movies/My Movie (2024)/My.Movie.2024.2160p.UHD.BluRay.x265-GROUP.processed.mkv",
        ),
        Path::new("/tmp/sceneforged/work-abc123"),
    )
}

fn create_context_with_custom_vars() -> TemplateContext {
    TemplateContext::new()
        .with_workspace(
            Path::new("/media/movies/movie.mkv"),
            Path::new("/media/movies/movie.processed.mkv"),
            Path::new("/tmp/work"),
        )
        .with_var("dv_mode", "el")
        .with_var("custom_var", "hello world")
        .with_var("bitrate", "8000k")
        .with_var("preset", "slow")
}

fn bench_substitute_single(c: &mut Criterion) {
    let mut group = c.benchmark_group("substitute_single");

    let ctx = create_context();

    // No variables (baseline)
    group.throughput(Throughput::Bytes(TEMPLATE_NO_VARS.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("template", "no_vars"),
        &TEMPLATE_NO_VARS,
        |b, template| {
            b.iter(|| ctx.substitute(black_box(template)));
        },
    );

    // Simple template
    group.throughput(Throughput::Bytes(TEMPLATE_SIMPLE.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("template", "simple"),
        &TEMPLATE_SIMPLE,
        |b, template| {
            b.iter(|| ctx.substitute(black_box(template)));
        },
    );

    // Medium template
    group.throughput(Throughput::Bytes(TEMPLATE_MEDIUM.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("template", "medium"),
        &TEMPLATE_MEDIUM,
        |b, template| {
            b.iter(|| ctx.substitute(black_box(template)));
        },
    );

    // Complex template
    group.throughput(Throughput::Bytes(TEMPLATE_COMPLEX.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("template", "complex"),
        &TEMPLATE_COMPLEX,
        |b, template| {
            b.iter(|| ctx.substitute(black_box(template)));
        },
    );

    // All variables template
    group.throughput(Throughput::Bytes(TEMPLATE_ALL_VARS.len() as u64));
    group.bench_with_input(
        BenchmarkId::new("template", "all_vars"),
        &TEMPLATE_ALL_VARS,
        |b, template| {
            b.iter(|| ctx.substitute(black_box(template)));
        },
    );

    group.finish();
}

fn bench_substitute_with_custom_vars(c: &mut Criterion) {
    let mut group = c.benchmark_group("substitute_custom_vars");

    let ctx = create_context_with_custom_vars();

    group.bench_with_input(
        BenchmarkId::new("template", "custom_vars"),
        &TEMPLATE_CUSTOM,
        |b, template| {
            b.iter(|| ctx.substitute(black_box(template)));
        },
    );

    group.finish();
}

fn bench_substitute_all(c: &mut Criterion) {
    let mut group = c.benchmark_group("substitute_all");

    let ctx = create_context();

    // Small batch
    let small_batch: Vec<String> = vec![
        TEMPLATE_SIMPLE.to_string(),
        TEMPLATE_MEDIUM.to_string(),
        TEMPLATE_COMPLEX.to_string(),
    ];

    group.bench_with_input(
        BenchmarkId::new("batch", "3_templates"),
        &small_batch,
        |b, templates| {
            b.iter(|| ctx.substitute_all(black_box(templates)));
        },
    );

    // Medium batch (simulating a complex pipeline)
    let medium_batch: Vec<String> = (0..10)
        .map(|i| {
            format!(
                "step{}: ffmpeg -i {{input}} -o {{temp_dir}}/step{}.mkv",
                i, i
            )
        })
        .collect();

    group.bench_with_input(
        BenchmarkId::new("batch", "10_templates"),
        &medium_batch,
        |b, templates| {
            b.iter(|| ctx.substitute_all(black_box(templates)));
        },
    );

    // Large batch
    let large_batch: Vec<String> = (0..50)
        .map(|i| format!("cmd{} {{input}} {{output}} {{temp_dir}} {{filestem}}", i))
        .collect();

    group.bench_with_input(
        BenchmarkId::new("batch", "50_templates"),
        &large_batch,
        |b, templates| {
            b.iter(|| ctx.substitute_all(black_box(templates)));
        },
    );

    group.finish();
}

fn bench_context_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("context_creation");

    let input = Path::new("/media/movies/movie.mkv");
    let output = Path::new("/media/movies/movie.processed.mkv");
    let temp = Path::new("/tmp/work");

    group.bench_function("with_workspace", |b| {
        b.iter(|| {
            TemplateContext::new().with_workspace(
                black_box(input),
                black_box(output),
                black_box(temp),
            )
        });
    });

    group.bench_function("with_workspace_and_vars", |b| {
        b.iter(|| {
            TemplateContext::new()
                .with_workspace(black_box(input), black_box(output), black_box(temp))
                .with_var("var1", "value1")
                .with_var("var2", "value2")
                .with_var("var3", "value3")
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_substitute_single,
    bench_substitute_with_custom_vars,
    bench_substitute_all,
    bench_context_creation
);
criterion_main!(benches);
