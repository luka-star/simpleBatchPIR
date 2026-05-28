mod support;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use ndarray::Array2;
use shared::models::Band;
use shared::pbc;
use shared::rings::{lift_matrix_to_zq, Zq};
use simplepir::types::{SimplePIRDatabase, SimplePIRRecordAnswer, SimplePIRRecordQuery};
use simplepir::{BatchSimplePIRServer, SimplePIRClient};
use std::path::Path;
use std::time::{Duration, Instant};
use support::{assert_requested_band_count, make_bands};

criterion_group! {
    name = benches;
    config = Criterion::default()
        .output_directory(Path::new("benchmark-results"))
        .measurement_time(std::time::Duration::from_secs(20));
    targets =
        padded_position_answer,
}

const NUMBER_OF_BANDS: usize = 4096;
const BUCKET_COUNT: usize = 1500;
const HASH_FUNCTION_COUNT: usize = 3;
const QUERY_SAMPLES: usize = 512;
const SUMMARY_REPETITIONS: u32 = 50;

#[derive(Debug, Clone)]
struct PaddingTarget {
    bucket: usize,
    real_records: usize,
    padded_record_capacity: usize,
}

impl PaddingTarget {
    fn padding_records(&self) -> usize {
        self.padded_record_capacity - self.real_records
    }
}

fn bucket_record_counts(record_count: usize, config: &pbc::PBCConfig) -> Vec<usize> {
    let mut counts = vec![0; config.buckets];

    for record_id in 0..record_count {
        for bucket in config.bucket_positions(&record_id) {
            counts[bucket] += 1;
        }
    }

    counts
}

fn most_padded_nonempty_bucket(
    buckets: &[SimplePIRDatabase],
    real_record_counts: &[usize],
    record_size: usize,
) -> PaddingTarget {
    buckets
        .iter()
        .enumerate()
        .filter_map(|(bucket, matrix)| {
            let real_records = real_record_counts[bucket];
            let padded_record_capacity = matrix.len() / record_size;
            if real_records == 0 || real_records >= padded_record_capacity {
                return None;
            }

            Some(PaddingTarget {
                bucket,
                real_records,
                padded_record_capacity,
            })
        })
        .max_by(|left, right| {
            let left_padding = left.padding_records() * right.padded_record_capacity;
            let right_padding = right.padding_records() * left.padded_record_capacity;
            left_padding.cmp(&right_padding)
        })
        .expect("expected at least one nonempty bucket with padded record slots")
}

fn query_record(record_index: usize, record_size: usize, square_n: usize) -> SimplePIRRecordQuery {
    let block_start_cell = record_index * record_size;
    let (_state, query) = SimplePIRClient::query_record(block_start_cell, record_size, square_n);
    query
}

fn answer_bucket(bucket: &Array2<Zq>, query: &SimplePIRRecordQuery) -> SimplePIRRecordAnswer {
    query.iter().map(|q| bucket.dot(q)).collect()
}

fn answer_query_batch(
    bucket: &Array2<Zq>,
    queries: &[SimplePIRRecordQuery],
) -> Vec<SimplePIRRecordAnswer> {
    queries
        .iter()
        .map(|query| answer_bucket(bucket, query))
        .collect()
}

fn generate_query_samples(
    start_record: usize,
    record_count: usize,
    samples: usize,
    record_size: usize,
    square_n: usize,
) -> Vec<SimplePIRRecordQuery> {
    (0..samples)
        .map(|sample| {
            let record = start_record + (sample % record_count);
            query_record(record, record_size, square_n)
        })
        .collect()
}

fn average_batch_time(
    bucket: &Array2<Zq>,
    queries: &[SimplePIRRecordQuery],
    repetitions: u32,
) -> Duration {
    let mut total = Duration::ZERO;

    for _ in 0..repetitions {
        let start = Instant::now();
        black_box(answer_query_batch(black_box(bucket), black_box(queries)));
        total += start.elapsed();
    }

    total / repetitions
}

fn padded_position_answer(c: &mut Criterion) {
    let bands = make_bands(NUMBER_OF_BANDS).expect("Failed to fetch bands");
    assert_requested_band_count(NUMBER_OF_BANDS, bands.len());

    let config = pbc::PBCConfig::fixed_seeds(BUCKET_COUNT, HASH_FUNCTION_COUNT);
    let batch_server =
        BatchSimplePIRServer::setup(&Band::bands_to_matrix(&bands), Band::SIZEOFRECORD, &config);
    let real_record_counts = bucket_record_counts(bands.len(), &config);
    let target = most_padded_nonempty_bucket(
        &batch_server.buckets,
        &real_record_counts,
        Band::SIZEOFRECORD,
    );
    let bucket = &batch_server.buckets[target.bucket];
    let lifted_bucket = lift_matrix_to_zq(bucket);

    let real_queries = generate_query_samples(
        0,
        target.real_records,
        QUERY_SAMPLES,
        Band::SIZEOFRECORD,
        bucket.nrows(),
    );
    let padded_queries = generate_query_samples(
        target.real_records,
        target.padding_records(),
        QUERY_SAMPLES,
        Band::SIZEOFRECORD,
        bucket.nrows(),
    );

    eprintln!(
        "bucket_padding target: bucket={}, real_records={}, padded_capacity={}, padding_records={}, query_samples={}, summary_repetitions={}",
        target.bucket,
        target.real_records,
        target.padded_record_capacity,
        target.padding_records(),
        QUERY_SAMPLES,
        SUMMARY_REPETITIONS
    );

    let real_average = average_batch_time(&lifted_bucket, &real_queries, SUMMARY_REPETITIONS);
    let padded_average = average_batch_time(&lifted_bucket, &padded_queries, SUMMARY_REPETITIONS);
    eprintln!(
        "bucket_padding summary: real_avg_batch={:.3?}, padded_avg_batch={:.3?}, real_avg_query={:.3?}, padded_avg_query={:.3?}",
        real_average,
        padded_average,
        real_average / QUERY_SAMPLES as u32,
        padded_average / QUERY_SAMPLES as u32
    );

    let mut group = c.benchmark_group("bucket_padding_answer");
    group.bench_with_input(
        BenchmarkId::new("same_bucket_real_positions", target.bucket),
        &real_queries,
        |b, queries| {
            b.iter(|| {
                black_box(answer_query_batch(
                    black_box(&lifted_bucket),
                    black_box(queries),
                ));
            })
        },
    );
    group.bench_with_input(
        BenchmarkId::new("same_bucket_padded_positions", target.bucket),
        &padded_queries,
        |b, queries| {
            b.iter(|| {
                black_box(answer_query_batch(
                    black_box(&lifted_bucket),
                    black_box(queries),
                ));
            })
        },
    );
    group.finish();
}

criterion_main!(benches);
