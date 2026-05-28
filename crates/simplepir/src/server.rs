use ndarray::{Array1, Array2};
use shared::compute_a;
use shared::rings::{lift_matrix_to_zq, Zp, Zq};
use std::num::Wrapping;

use crate::types::{
    BatchSimplePIRAnswer, BatchSimplePIRBucketOracle, BatchSimplePIRHint, BatchSimplePIRQuery,
    PBCConfig, SimplePIRDatabase, SimplePIRHint, SimplePIRRecord, SimplePIRRecordAnswer,
    SimplePIRRecordQuery,
};

pub struct SimplePIRServer {
    pub db: SimplePIRDatabase,
    pub hint: SimplePIRHint,
}

impl SimplePIRServer {
    pub fn setup(db: SimplePIRDatabase) -> Self {
        let matrix = compute_a(db.nrows());
        let hint = compute_hint(&db, &matrix);
        Self { db, hint }
    }

    pub fn hint(&self) -> &SimplePIRHint {
        &self.hint
    }

    pub fn square_n(&self) -> usize {
        self.db.nrows()
    }

    pub fn answer(&self, query: &SimplePIRRecordQuery) -> SimplePIRRecordAnswer {
        answer_query(&self.db, query)
    }
}

pub struct BatchSimplePIRServer {
    pub hints: BatchSimplePIRHint,
    pub position_map: BatchSimplePIRBucketOracle,
    pub buckets: Vec<SimplePIRDatabase>,
}

impl BatchSimplePIRServer {
    pub fn setup(db: &SimplePIRDatabase, record_cell_count: usize, config: &PBCConfig) -> Self {
        let (hints, position_map, buckets) = setup_batching(db, record_cell_count, config);

        Self {
            hints,
            position_map,
            buckets,
        }
    }

    pub fn bucket_size(&self) -> usize {
        self.buckets.first().map(|bucket| bucket.len()).unwrap_or(0)
    }

    pub fn hints(&self) -> Vec<SimplePIRHint> {
        self.hints.clone()
    }

    pub fn answer(&self, query: &BatchSimplePIRQuery) -> BatchSimplePIRAnswer {
        let lifted_buckets: Vec<Array2<Zq>> = self.buckets.iter().map(lift_matrix_to_zq).collect();
        batch_answer(query, &lifted_buckets)
    }
}

fn compute_hint(db: &SimplePIRDatabase, matrix: &Array2<Zq>) -> SimplePIRHint {
    let db_lifted = lift_matrix_to_zq(db);
    db_lifted.dot(matrix)
}

fn answer_query(db: &SimplePIRDatabase, query: &SimplePIRRecordQuery) -> SimplePIRRecordAnswer {
    let db_lifted = lift_matrix_to_zq(db);
    query.iter().map(|q| db_lifted.dot(q)).collect()
}

fn batch_answer(queries: &BatchSimplePIRQuery, buckets: &[Array2<Zq>]) -> BatchSimplePIRAnswer {
    buckets
        .iter()
        .zip(queries.iter())
        .map(|(bucket, query_bundle)| query_bundle.iter().map(|query| bucket.dot(query)).collect())
        .collect()
}

fn batching_encode(
    records: &[SimplePIRRecord],
    config: &PBCConfig,
) -> (Vec<Vec<Zp>>, BatchSimplePIRBucketOracle) {
    let b = config.buckets;
    let w = config.w();

    let m = (records.len() * w) / b;
    let mut raw_buckets: Vec<Vec<SimplePIRRecord>> = vec![Vec::with_capacity(m); b];
    let mut position_map = BatchSimplePIRBucketOracle::new();
    for (record_id, record) in records.iter().enumerate() {
        let candidates = config.bucket_positions(&record_id);
        for &bucket_idx in &candidates {
            let pos = raw_buckets[bucket_idx].len();
            raw_buckets[bucket_idx].push(record.clone());
            position_map.insert((bucket_idx, record_id), pos);
        }
    }

    let buckets = raw_buckets
        .iter()
        .map(|bucket| {
            bucket
                .iter()
                .flat_map(|record| record.iter().copied())
                .collect()
        })
        .collect();

    (buckets, position_map)
}

fn setup_batching(
    db: &SimplePIRDatabase,
    record_size: usize,
    config: &PBCConfig,
) -> (
    BatchSimplePIRHint,
    BatchSimplePIRBucketOracle,
    Vec<SimplePIRDatabase>,
) {
    let records = database_records(db, record_size);
    let (buckets, position_map) = batching_encode(&records, config);
    let mut hints = Vec::with_capacity(buckets.len());
    let padded_buckets = pad_buckets(buckets);
    if let Some(first_bucket) = padded_buckets.first() {
        let matrix = compute_a(first_bucket.nrows());
        for bucket in &padded_buckets {
            hints.push(compute_hint(bucket, &matrix));
        }
    }

    (hints, position_map, padded_buckets)
}

fn database_records(db: &SimplePIRDatabase, record_cell_count: usize) -> Vec<SimplePIRRecord> {
    assert!(record_cell_count > 0, "record cell count must be positive");

    db.iter()
        .copied()
        .collect::<Vec<Zp>>()
        .chunks(record_cell_count)
        .filter(|chunk| chunk.len() == record_cell_count)
        .map(|chunk| Array1::from_vec(chunk.to_vec()))
        .collect()
}

fn pad_buckets(buckets: Vec<Vec<Zp>>) -> Vec<SimplePIRDatabase> {
    let max_size = buckets.iter().map(|bucket| bucket.len()).max().unwrap_or(0);
    let sqrt_size = if max_size == 0 {
        0
    } else {
        (max_size as f64).sqrt().ceil() as usize
    };
    let padded_size = sqrt_size * sqrt_size;

    buckets
        .into_iter()
        .map(|bucket| {
            let mut padded = vec![Wrapping(0); padded_size];
            padded[..bucket.len()].clone_from_slice(&bucket);
            Array2::from_shape_vec((sqrt_size, sqrt_size), padded).unwrap()
        })
        .collect()
}
