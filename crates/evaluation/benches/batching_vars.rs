mod support;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use rand::seq::SliceRandom;
use shared::models::Band;
use shared::pbc;
use simplepir::types::{
    BatchSimplePIRBucketOracle, BatchSimplePIRQuery, BatchSimplePIRSchedule, PBCConfig,
    SimplePIRClientState,
};
use simplepir::{BatchSimplePIRClient, BatchSimplePIRServer};
use std::path::Path;
use support::{assert_requested_band_count, make_bands, random_index_list};

criterion_group! {
    name = benches;
    config = Criterion::default()
        .output_directory(Path::new("benchmark-results"))
        .warm_up_time(std::time::Duration::from_secs(2))
        .measurement_time(std::time::Duration::from_secs(8))
        .sample_size(10);
    targets =
        online_query,
}

const NUMBER_OF_BANDS: usize = 131072;
const QUERY_BATCH_SIZE: usize = 64;
const HASH_FUNCTION_COUNTS: [usize; 3] = [2, 3, 4];
const BUCKET_COUNTS: [usize; 10] = [
    256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536, 131072,
];
struct ScheduledSubquery {
    indices: Vec<usize>,
    states: Vec<SimplePIRClientState>,
    queries: BatchSimplePIRQuery,
    schedule: BatchSimplePIRSchedule,
}

fn load_bands(nr_bands: usize) -> Vec<shared::models::Band> {
    let bands = make_bands(nr_bands).expect("Failed to fetch bands");
    assert_requested_band_count(nr_bands, bands.len());
    bands
}

fn query_with_split_fallback(
    indices: &[usize],
    position_map: &BatchSimplePIRBucketOracle,
    bucket_size: usize,
    record_cell_count: usize,
    config: &PBCConfig,
) -> Vec<ScheduledSubquery> {
    match BatchSimplePIRClient::query(
        indices,
        position_map,
        bucket_size,
        record_cell_count,
        config,
    ) {
        Ok((states, queries, schedule)) => vec![ScheduledSubquery {
            indices: indices.to_vec(),
            states,
            queries,
            schedule,
        }],
        Err(error) if indices.len() == 1 => {
            panic!("failed to schedule single-index batch: {error}")
        }
        Err(_) => {
            let mut shuffled = indices.to_vec();
            shuffled.shuffle(&mut rand::thread_rng());
            let mid = shuffled.len() / 2;

            let mut subqueries = query_with_split_fallback(
                &shuffled[..mid],
                position_map,
                bucket_size,
                record_cell_count,
                config,
            );
            subqueries.extend(query_with_split_fallback(
                &shuffled[mid..],
                position_map,
                bucket_size,
                record_cell_count,
                config,
            ));
            subqueries
        }
    }
}

fn online_query(c: &mut Criterion) {
    let bands = load_bands(NUMBER_OF_BANDS);
    let db = Band::bands_to_matrix(&bands);
    let mut group = c.benchmark_group(format!(
        "batching_online/db_{NUMBER_OF_BANDS}/k_{QUERY_BATCH_SIZE}"
    ));

    for w in HASH_FUNCTION_COUNTS {
        for bucket_count in BUCKET_COUNTS {
            let config = pbc::PBCConfig::fixed_seeds(bucket_count, w);
            let batch_server = BatchSimplePIRServer::setup(&db, Band::SIZEOFRECORD, &config);
            let bucket_size = batch_server.bucket_size();
            let hint_cs = batch_server.hints();
            let index_list = random_index_list(QUERY_BATCH_SIZE, bands.len());

            group.bench_with_input(
                BenchmarkId::new(format!("w_{w}"), bucket_count),
                &bucket_count,
                |b, &_bucket_count| {
                    b.iter(|| {
                        let subqueries = query_with_split_fallback(
                            black_box(&index_list),
                            black_box(&batch_server.position_map),
                            black_box(bucket_size),
                            black_box(Band::SIZEOFRECORD),
                            black_box(&config),
                        );

                        for subquery in subqueries {
                            let answers = batch_server.answer(black_box(&subquery.queries));
                            BatchSimplePIRClient::recover(
                                black_box(&subquery.states),
                                black_box(&answers),
                                black_box(&subquery.indices),
                                black_box(&subquery.schedule),
                                black_box(&hint_cs),
                            );
                        }
                    })
                },
            );
        }
    }

    group.finish();
}

criterion_main!(benches);
