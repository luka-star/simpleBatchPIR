mod support;

use client::querying;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use server_pir::{offline_preprocess, online_process};
use shared::{models::Band, pbc, rings::Zq};
use std::path::Path;
use support::{assert_requested_band_count, make_bands, random_index_list};
use tokio::runtime::Runtime;

criterion_group! {
    name = benches;
    config = Criterion::default()
        .output_directory(Path::new("benchmark-results"))
        .measurement_time(std::time::Duration::from_secs(20));
    targets =
        multiple_query,
}

const NUMBER_OF_BANDS: [usize; 6] = [1024, 2048, 4096, 8192, 16384, 32768];
const NUMBER_OF_ITEMS: [usize; 7] = [2, 4, 8, 16, 20, 32, 64];

fn multiple_query(c: &mut Criterion) {
    for nr_bands in NUMBER_OF_BANDS {
        let mut group = c.benchmark_group(format!("multiple_query_simplepir/db_{nr_bands}"));

        for nr_indices in NUMBER_OF_ITEMS {
            let rt = Runtime::new().unwrap();
            let bands = rt
                .block_on(make_bands(nr_bands))
                .expect("Failed to fetch bands");
            assert_requested_band_count(nr_bands, bands.len());
            let db_matrix = Band::bands_to_matrix(&bands);
            let n_elements = bands.len() * Band::SIZEOFRECORD;
            let setup_res = offline_preprocess::setup(&db_matrix);
            let index_list = random_index_list(nr_indices, bands.len());

            group.bench_with_input(
                BenchmarkId::from_parameter(nr_indices),
                &nr_indices,
                |b, &_nr_indices| {
                    b.iter(|| {
                        for i in &index_list {
                            let (secrets, queries) =
                                querying::query(black_box(*i), black_box(n_elements));
                            let answers = online_process::answer_query(
                                black_box(&db_matrix),
                                black_box(&queries),
                            );
                            querying::recover(
                                black_box(&secrets),
                                black_box(&setup_res.hint_c),
                                black_box(&answers),
                            );
                        }
                    })
                },
            );
        }

        group.finish();
    }

    for nr_bands in NUMBER_OF_BANDS {
        let mut group = c.benchmark_group(format!("multiple_query_batchpir/db_{nr_bands}"));

        for nr_indices in NUMBER_OF_ITEMS {
            let rt = Runtime::new().unwrap();
            let bands = rt
                .block_on(make_bands(nr_bands))
                .expect("Failed to fetch bands");
            assert_requested_band_count(nr_bands, bands.len());
            let config = pbc::PBCConfig::new(1500, 3);
            let (setup_res, position_map, padded_buckets, lifted_buckets) =
                offline_preprocess::setup_batching(&bands, &config);
            let index_list = random_index_list(nr_indices, bands.len());
            let bucket_element_counts: Vec<usize> =
                padded_buckets.iter().map(|bucket| bucket.len()).collect();
            let hint_cs: Vec<_> = setup_res
                .iter()
                .map(|result| result.hint_c.clone())
                .collect::<Vec<ndarray::Array2<Zq>>>();

            group.bench_with_input(
                BenchmarkId::from_parameter(nr_indices),
                &nr_indices,
                |b, &_nr_indices| {
                    b.iter(|| {
                        let (states, queries, schedule) = querying::batch_querying(
                            black_box(&index_list),
                            black_box(&position_map),
                            black_box(&bucket_element_counts),
                            black_box(&config),
                        );
                        let schedule =
                            schedule.expect("batch querying should succeed in benchmark");
                        let answers = online_process::batch_answering(
                            black_box(&queries),
                            black_box(&lifted_buckets),
                        );
                        querying::batch_recovering(
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
