mod support;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use server_pir::offline_preprocess;
use shared::{models::Band, pbc};
use std::path::Path;
use support::{assert_requested_band_count, make_bands};
use tokio::runtime::Runtime;

criterion_group! {
    name = benches;
    config = Criterion::default()
        .output_directory(Path::new("benchmark-results"))
        .warm_up_time(std::time::Duration::from_secs(3))
        .measurement_time(std::time::Duration::from_secs(8))
        .sample_size(10);
    targets =
        offline_setup,
}

const DB_SIZE: [usize; 10] = [1, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384];

fn offline_setup(c: &mut Criterion) {
    let mut group = c.benchmark_group("offline_setup");

    for nr_bands in DB_SIZE {
        group.bench_with_input(
            BenchmarkId::new("simplepir", nr_bands),
            &nr_bands,
            |b, &nr_bands| {
                b.iter(|| {
                    let rt = Runtime::new().unwrap();
                    let bands = rt
                        .block_on(make_bands(nr_bands))
                        .expect("Failed to fetch bands");
                    assert_requested_band_count(nr_bands, bands.len());
                    let db_matrix = Band::bands_to_matrix(&bands);
                    offline_preprocess::setup(black_box(&db_matrix))
                })
            },
        );
    }

    for nr_bands in DB_SIZE {
        group.bench_with_input(
            BenchmarkId::new("batchpir", nr_bands),
            &nr_bands,
            |b, &nr_bands| {
                b.iter(|| {
                    let rt = Runtime::new().unwrap();
                    let bands = rt
                        .block_on(make_bands(nr_bands))
                        .expect("Failed to fetch bands");
                    assert_requested_band_count(nr_bands, bands.len());
                    let config = pbc::PBCConfig::new(1500, 3);
                    offline_preprocess::setup_batching(black_box(&bands), black_box(&config))
                })
            },
        );
    }

    group.finish();
}

criterion_main!(benches);
