mod support;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use rayon::prelude::*;
use shared::models::Band;
use shared::pbc;
use simplepir::{BatchSimplePIRClient, BatchSimplePIRServer, SimplePIRClient, SimplePIRServer};
use std::path::Path;
use support::{assert_requested_band_count, make_bands, random_index_list};

criterion_group! {
    name = benches;
    config = Criterion::default()
        .output_directory(Path::new("benchmark-results"))
        .measurement_time(std::time::Duration::from_secs(20));
    targets =
        parallel_multiple_query,
}

const NUMBER_OF_BANDS: [usize; 8] = [1024, 2048, 4096, 8192, 16384, 32768, 65536, 131072];
const NUMBER_OF_ITEMS: [usize; 6] = [2, 4, 8, 16, 32, 64];

fn parallel_multiple_query(c: &mut Criterion) {
    for nr_bands in NUMBER_OF_BANDS {
        let mut group =
            c.benchmark_group(format!("multiple_query_parallel_simplepir/db_{nr_bands}"));

        for nr_indices in NUMBER_OF_ITEMS {
            let bands = make_bands(nr_bands).expect("Failed to fetch bands");
            assert_requested_band_count(nr_bands, bands.len());
            let pir_server = SimplePIRServer::setup(Band::bands_to_matrix(&bands));
            let index_list = random_index_list(nr_indices, bands.len());

            group.bench_with_input(
                BenchmarkId::from_parameter(nr_indices),
                &nr_indices,
                |b, &_nr_indices| {
                    b.iter(|| {
                        let records: Vec<_> = index_list
                            .par_iter()
                            .map(|i| {
                                let block_start_cell = *i * Band::SIZEOFRECORD;
                                let (secrets, queries) = SimplePIRClient::query_record(
                                    black_box(block_start_cell),
                                    black_box(Band::SIZEOFRECORD),
                                    black_box(pir_server.square_n()),
                                );
                                let answers = pir_server.answer(black_box(&queries));
                                SimplePIRClient::recover_record(
                                    black_box(&secrets),
                                    black_box(pir_server.hint()),
                                    black_box(&answers),
                                )
                            })
                            .collect();
                        black_box(records);
                    })
                },
            );
        }

        group.finish();
    }

    for nr_bands in NUMBER_OF_BANDS {
        let mut group = c.benchmark_group(format!("multiple_query_batchpir/db_{nr_bands}"));

        for nr_indices in NUMBER_OF_ITEMS {
            let bands = make_bands(nr_bands).expect("Failed to fetch bands");
            assert_requested_band_count(nr_bands, bands.len());
            let config = pbc::PBCConfig::fixed_seeds(1500, 3);
            let batch_server = BatchSimplePIRServer::setup(
                &Band::bands_to_matrix(&bands),
                Band::SIZEOFRECORD,
                &config,
            );
            let index_list = random_index_list(nr_indices, bands.len());
            let bucket_size = batch_server.bucket_size();
            let hint_cs = batch_server.hints();

            group.bench_with_input(
                BenchmarkId::from_parameter(nr_indices),
                &nr_indices,
                |b, &_nr_indices| {
                    b.iter(|| {
                        let (states, queries, schedule) = BatchSimplePIRClient::query(
                            black_box(&index_list),
                            black_box(&batch_server.position_map),
                            black_box(bucket_size),
                            black_box(Band::SIZEOFRECORD),
                            black_box(&config),
                        )
                        .expect("batch querying should succeed in benchmark");
                        let answers = batch_server.answer(black_box(&queries));
                        BatchSimplePIRClient::recover(
                            black_box(&states),
                            black_box(&answers),
                            black_box(&index_list),
                            black_box(&schedule),
                            black_box(&hint_cs),
                        );
                    })
                },
            );
        }

        group.finish();
    }
}

criterion_main!(benches);
