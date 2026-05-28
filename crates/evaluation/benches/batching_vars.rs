mod support;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use shared::models::Band;
use shared::pbc;
use simplepir::{BatchSimplePIRClient, BatchSimplePIRServer};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::Once;
use support::{assert_requested_band_count, make_bands, random_index_list};

criterion_group! {
    name = benches;
    config = Criterion::default()
        .output_directory(Path::new("benchmark-results"))
        .warm_up_time(std::time::Duration::from_secs(2))
        .measurement_time(std::time::Duration::from_secs(8))
        .sample_size(10);
    targets =
        failure_rate_export,
        setup_vars,
        multiple_query,
}

const NUMBER_OF_BANDS: usize = 4069;
const HASH_FUNCTION_COUNTS: [usize; 4] = [2, 3, 4, 5];
const BUCKET_COUNTS: [usize; 5] = [500, 1000, 1500, 2000, 2500];
const QUERY_BATCH_SIZES: [usize; 4] = [4, 8, 16, 32];
const MAX_SCHEDULE_ATTEMPTS: usize = 128;

const FAILURE_DB_SIZES: [usize; 5] = [1024, 2048, 4096, 8192, 16384];
const FAILURE_BUCKET_RATIOS: [(usize, usize, &str); 5] = [
    (1, 10, "0.1"),
    (1, 8, "0.125"),
    (1, 4, "0.25"),
    (1, 2, "0.5"),
    (1, 1, "1.0"),
];
const FAILURE_HASH_FUNCTION_COUNTS: [usize; 3] = [2, 3, 4];
const FAILURE_QUERY_BATCH_SIZES: [usize; 16] =
    [2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28, 30, 32];
const FAILURE_RATE_TRIALS: usize = 10000;

static FAILURE_EXPORT_ONCE: Once = Once::new();

struct FailureRateRecord {
    n: usize,
    b: usize,
    b_ratio: &'static str,
    w: usize,
    k: usize,
    trials: usize,
    successes: usize,
    failure_rate: f64,
}

fn load_bands(nr_bands: usize) -> Vec<shared::models::Band> {
    let bands = make_bands(nr_bands).expect("Failed to fetch bands");
    assert_requested_band_count(nr_bands, bands.len());
    bands
}

fn failure_only_mode() -> bool {
    std::env::var("BATCHING_VARS_MODE")
        .map(|value| value == "failure")
        .unwrap_or(false)
}

fn sample_schedulable_indices(
    nr_indices: usize,
    upper: usize,
    config: &pbc::PBCConfig,
) -> Option<Vec<usize>> {
    for _ in 0..MAX_SCHEDULE_ATTEMPTS {
        let index_list = random_index_list(nr_indices, upper);
        if pbc::gen_schedule(config, &index_list).is_ok() {
            return Some(index_list);
        }
    }

    None
}

fn compute_failure_rate(
    upper: usize,
    bucket_count: usize,
    bucket_ratio: &'static str,
    config: &pbc::PBCConfig,
    nr_indices: usize,
    trials: usize,
) -> FailureRateRecord {
    let mut successes = 0usize;

    for _ in 0..trials {
        let index_list = random_index_list(nr_indices, upper);
        if pbc::gen_schedule(config, &index_list).is_ok() {
            successes += 1;
        }
    }

    FailureRateRecord {
        n: upper,
        b: bucket_count,
        b_ratio: bucket_ratio,
        w: config.w(),
        k: nr_indices,
        trials,
        successes,
        failure_rate: 1.0 - (successes as f64 / trials as f64),
    }
}

fn write_failure_rate_csv(output_path: &Path, records: &[FailureRateRecord]) {
    output_path.parent().unwrap().mkdir_if_missing();
    let mut file = File::create(output_path).expect("Failed to create failure rate CSV");
    writeln!(file, "n,b,b_ratio,w,k,trials,successes,failure_rate")
        .expect("Failed to write CSV header");

    for record in records {
        writeln!(
            file,
            "{},{},{},{},{},{},{},{}",
            record.n,
            record.b,
            record.b_ratio,
            record.w,
            record.k,
            record.trials,
            record.successes,
            record.failure_rate
        )
        .expect("Failed to write failure rate row");
    }
}

trait MkdirIfMissing {
    fn mkdir_if_missing(&self);
}

impl MkdirIfMissing for Path {
    fn mkdir_if_missing(&self) {
        std::fs::create_dir_all(self).expect("Failed to create benchmark-results directory");
    }
}

fn export_failure_rate_data() {
    let mut records = Vec::new();

    for db_size in FAILURE_DB_SIZES {
        for (num, den, ratio_label) in FAILURE_BUCKET_RATIOS {
            let bucket_count = (db_size * num) / den;

            for w in FAILURE_HASH_FUNCTION_COUNTS {
                let config = pbc::PBCConfig::fixed_seeds(bucket_count, w);

                for nr_indices in FAILURE_QUERY_BATCH_SIZES {
                    records.push(compute_failure_rate(
                        db_size,
                        bucket_count,
                        ratio_label,
                        &config,
                        nr_indices,
                        FAILURE_RATE_TRIALS,
                    ));
                }
            }
        }
    }

    write_failure_rate_csv(
        Path::new("benchmark-results/failure_rate_summary.csv"),
        &records,
    );
}

fn failure_rate_export(c: &mut Criterion) {
    if failure_only_mode() {
        FAILURE_EXPORT_ONCE.call_once(export_failure_rate_data);
        return;
    }

    FAILURE_EXPORT_ONCE.call_once(export_failure_rate_data);

    c.bench_function("failure_rate_export", |b| b.iter(|| black_box(())));
}

fn setup_vars(c: &mut Criterion) {
    if failure_only_mode() {
        return;
    }

    let bands = load_bands(NUMBER_OF_BANDS);
    let mut group = c.benchmark_group(format!("batching_setup_vars/db_{NUMBER_OF_BANDS}"));

    for w in HASH_FUNCTION_COUNTS {
        for bucket_count in BUCKET_COUNTS {
            let config = pbc::PBCConfig::fixed_seeds(bucket_count, w);

            group.bench_with_input(
                BenchmarkId::new(format!("w_{w}"), bucket_count),
                &config,
                |b, config| {
                    let db = Band::bands_to_matrix(&bands);
                    b.iter(|| {
                        BatchSimplePIRServer::setup(
                            black_box(&db),
                            black_box(Band::SIZEOFRECORD),
                            black_box(config),
                        )
                    })
                },
            );
        }
    }

    group.finish();
}

fn multiple_query(c: &mut Criterion) {
    if failure_only_mode() {
        return;
    }

    let bands = load_bands(NUMBER_OF_BANDS);

    for w in HASH_FUNCTION_COUNTS {
        for bucket_count in BUCKET_COUNTS {
            let config = pbc::PBCConfig::fixed_seeds(bucket_count, w);
            let batch_server = BatchSimplePIRServer::setup(
                &Band::bands_to_matrix(&bands),
                Band::SIZEOFRECORD,
                &config,
            );
            let bucket_size = batch_server.bucket_size();
            let hint_cs = batch_server.hints();
            let mut group = c.benchmark_group(format!(
                "multiple_query_batchpir/db_{NUMBER_OF_BANDS}/w_{w}/b_{bucket_count}"
            ));

            for nr_indices in QUERY_BATCH_SIZES {
                let Some(index_list) = sample_schedulable_indices(nr_indices, bands.len(), &config)
                else {
                    eprintln!(
                        "Skipping db={}, w={}, b={}, k={} after {} failed schedule attempts",
                        NUMBER_OF_BANDS, w, bucket_count, nr_indices, MAX_SCHEDULE_ATTEMPTS
                    );
                    continue;
                };

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
                            );
                            let schedule =
                                schedule.expect("batch querying should succeed in benchmark");
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
}

criterion_main!(benches);
