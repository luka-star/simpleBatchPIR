use ndarray::{Array1, Array2};
use shared::compute_a;
use shared::rings::{lift_matrix_to_zq, Zp, Zq};
use std::num::Wrapping;

use crate::types::{
    BatchSimplePIRAnswer, BatchSimplePIRBucketOracle, BatchSimplePIRQuery, PBCConfig,
    SimplePIRDatabase, SimplePIRHint, SimplePIRRecord, SimplePIRRecordAnswer, SimplePIRRecordQuery,
    SimplePIRServerSetup,
};

pub struct SimplePIRServer {
    pub db: SimplePIRDatabase,
    pub setup: SimplePIRServerSetup,
}

impl SimplePIRServer {
    pub fn setup(db: SimplePIRDatabase) -> Self {
        let setup = setup(&db);
        Self { db, setup }
    }

    pub fn hint(&self) -> &SimplePIRHint {
        &self.setup.hint
    }

    pub fn square_n(&self) -> usize {
        self.db.nrows()
    }

    pub fn answer(&self, query: &SimplePIRRecordQuery) -> SimplePIRRecordAnswer {
        answer_query(&self.db, query)
    }
}

pub struct BatchSimplePIRServer {
    pub setups: Vec<SimplePIRServerSetup>,
    pub position_map: BatchSimplePIRBucketOracle,
    pub buckets: Vec<SimplePIRDatabase>,
}

impl BatchSimplePIRServer {
    pub fn setup(db: &SimplePIRDatabase, record_cell_count: usize, config: &PBCConfig) -> Self {
        let (setups, position_map, buckets) = setup_batching(db, record_cell_count, config);

        Self {
            setups,
            position_map,
            buckets,
        }
    }

    pub fn bucket_element_counts(&self) -> Vec<usize> {
        self.buckets.iter().map(|bucket| bucket.len()).collect()
    }

    pub fn hints(&self) -> Vec<SimplePIRHint> {
        self.setups.iter().map(|setup| setup.hint.clone()).collect()
    }

    pub fn answer(&self, query: &BatchSimplePIRQuery) -> BatchSimplePIRAnswer {
        let lifted_buckets: Vec<Array2<Zq>> = self.buckets.iter().map(lift_matrix_to_zq).collect();
        batch_answer(query, &lifted_buckets)
    }
}

fn setup(db: &SimplePIRDatabase) -> SimplePIRServerSetup {
    let nrows = db.nrows();
    let matrix = compute_a(nrows);
    setup_with_matrix(db, &matrix)
}

fn setup_with_matrix(db: &SimplePIRDatabase, matrix: &Array2<Zq>) -> SimplePIRServerSetup {
    let db_lifted = lift_matrix_to_zq(db);
    let hint = db_lifted.dot(matrix);

    SimplePIRServerSetup { hint }
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
) -> (Vec<SimplePIRDatabase>, BatchSimplePIRBucketOracle) {
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
        .map(|bucket| records_to_matrix(bucket))
        .collect();

    (buckets, position_map)
}

fn setup_batching(
    db: &SimplePIRDatabase,
    record_cell_count: usize,
    config: &PBCConfig,
) -> (
    Vec<SimplePIRServerSetup>,
    BatchSimplePIRBucketOracle,
    Vec<SimplePIRDatabase>,
) {
    let records = database_records(db, record_cell_count);
    let (buckets, position_map) = batching_encode(&records, config);
    let mut setup_res = Vec::with_capacity(buckets.len());
    let padded_buckets = pad_buckets(buckets);

    if let Some(first_bucket) = padded_buckets.first() {
        let matrix = compute_a(first_bucket.nrows());
        for bucket in &padded_buckets {
            setup_res.push(setup_with_matrix(bucket, &matrix));
        }
    }

    (setup_res, position_map, padded_buckets)
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

fn records_to_matrix(records: &[SimplePIRRecord]) -> SimplePIRDatabase {
    let mut flat: Vec<Zp> = records
        .iter()
        .flat_map(|record| record.iter().copied())
        .collect();
    let total_elements = flat.len();
    let dim = if total_elements == 0 {
        0
    } else {
        (total_elements as f64).sqrt().ceil() as usize
    };
    flat.resize(dim * dim, Wrapping(0));
    Array2::from_shape_vec((dim, dim), flat).expect("failed to reshape records into matrix")
}

fn pad_buckets(buckets: Vec<SimplePIRDatabase>) -> Vec<SimplePIRDatabase> {
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
            let flat: Vec<Zp> = bucket.iter().cloned().collect();
            let mut padded = vec![Wrapping(0); padded_size];
            padded[..flat.len()].clone_from_slice(&flat);
            Array2::from_shape_vec((sqrt_size, sqrt_size), padded).unwrap()
        })
        .collect()
}
