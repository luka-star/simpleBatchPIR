mod support;

use criterion::{criterion_group, criterion_main, Criterion};
use shared::pbc;
use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::time::{Duration, Instant};
use support::random_index_list;

criterion_group! {
    name = benches;
    config = Criterion::default()
        .output_directory(Path::new("benchmark-results"));
    targets =
        export_failure_rate_data,
}

const NUMBER_OF_BANDS: usize = 131072;
const QUERY_BATCH_SIZE: usize = 64;
const HASH_FUNCTION_COUNTS: [usize; 3] = [2, 3, 4];
const BUCKET_COUNTS: [usize; 10] = [
    256, 512, 1024, 2048, 4096, 8192, 16384, 32768, 65536, 131072,
];
const DEFAULT_FAILURE_RATE_TRIALS: usize = 100_000_000;
const TRIALS_ENV_VAR: &str = "BATCHING_FAILURE_RATE_TRIALS";

struct FailureRateRecord {
    n: usize,
    b: usize,
    b_ratio: String,
    w: usize,
    k: usize,
    trials: usize,
    successes: usize,
    failure_rate: f64,
}

fn compute_failure_rate(
    upper: usize,
    bucket_count: usize,
    config: &pbc::PBCConfig,
    nr_indices: usize,
    trials: usize,
    cell_index: usize,
    cell_count: usize,
) -> FailureRateRecord {
    let mut successes = 0usize;
    let started = Instant::now();
    let report_step = (trials / 100).max(1);

    eprintln!(
        "[{cell_index}/{cell_count}] w={}, b={bucket_count}: starting {trials} trials",
        config.w()
    );

    for trial in 1..=trials {
        let index_list = random_index_list(nr_indices, upper);
        if pbc::gen_schedule(config, &index_list).is_ok() {
            successes += 1;
        }

        if trial == trials || trial % report_step == 0 {
            let elapsed = started.elapsed();
            let progress = trial as f64 / trials as f64;
            let eta = estimate_remaining(elapsed, progress);
            eprintln!(
                "[{cell_index}/{cell_count}] w={}, b={bucket_count}: {:>6.2}% ({trial}/{trials}), elapsed {}, ETA {}",
                config.w(),
                progress * 100.0,
                format_duration(elapsed),
                format_duration(eta)
            );
        }
    }

    let record = FailureRateRecord {
        n: upper,
        b: bucket_count,
        b_ratio: format!("{:.5}", bucket_count as f64 / upper as f64),
        w: config.w(),
        k: nr_indices,
        trials,
        successes,
        failure_rate: 1.0 - (successes as f64 / trials as f64),
    };

    eprintln!(
        "[{cell_index}/{cell_count}] w={}, b={bucket_count}: done, failures={}, failure_rate={:.8}%",
        config.w(),
        trials - successes,
        record.failure_rate * 100.0
    );

    record
}

fn create_failure_rate_csv(output_path: &Path) -> File {
    output_path.parent().unwrap().mkdir_if_missing();
    let mut file = File::create(output_path).expect("Failed to create failure rate CSV");
    writeln!(file, "n,b,b_ratio,w,k,trials,successes,failure_rate")
        .expect("Failed to write CSV header");
    file
}

fn write_failure_rate_record(file: &mut File, record: &FailureRateRecord) {
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
    file.flush().expect("Failed to flush failure rate CSV");
}

fn trial_count() -> usize {
    env::var(TRIALS_ENV_VAR)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(DEFAULT_FAILURE_RATE_TRIALS)
}

fn estimate_remaining(elapsed: Duration, progress: f64) -> Duration {
    if progress <= 0.0 {
        return Duration::ZERO;
    }

    let total_secs = elapsed.as_secs_f64() / progress;
    Duration::from_secs_f64((total_secs - elapsed.as_secs_f64()).max(0.0))
}

fn format_duration(duration: Duration) -> String {
    let total_secs = duration.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{hours}h {minutes:02}m {seconds:02}s")
    } else if minutes > 0 {
        format!("{minutes}m {seconds:02}s")
    } else {
        format!("{seconds}s")
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

fn export_failure_rate_data(_c: &mut Criterion) {
    let trials = trial_count();
    let cell_count = HASH_FUNCTION_COUNTS.len() * BUCKET_COUNTS.len();
    let mut cell_index = 0usize;
    let mut file = create_failure_rate_csv(Path::new("benchmark-results/failure_rate_summary.csv"));

    eprintln!(
        "Running schedule failure-rate export with {trials} trials per parameter setting ({cell_count} settings). Override with {TRIALS_ENV_VAR}=..."
    );

    for w in HASH_FUNCTION_COUNTS {
        for bucket_count in BUCKET_COUNTS {
            cell_index += 1;
            let config = pbc::PBCConfig::fixed_seeds(bucket_count, w);
            let record = compute_failure_rate(
                NUMBER_OF_BANDS,
                bucket_count,
                &config,
                QUERY_BATCH_SIZE,
                trials,
                cell_index,
                cell_count,
            );
            write_failure_rate_record(&mut file, &record);
        }
    }
}

criterion_main!(benches);
