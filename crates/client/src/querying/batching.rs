use ndarray::{Array1, Array2};
use rand::prelude::*;
use shared::rings::{Zp, Zq};
use shared::{compute_a, pbc, SIZEOFRECORD};
use std::collections::HashMap;

use super::plain::{query, query_with_matrix, recover, QueryState};

pub fn batch_recovering(
    states: &[QueryState],
    answers: &[Vec<Array1<Zq>>],
    indices: &[usize],
    schedule: &HashMap<usize, usize>,
    hint_c: &[Array2<Zq>],
) -> Vec<Array1<Zp>> {
    let mut results = Vec::new();

    for index in indices {
        let bucket = schedule[index];
        let recovered = recover(&states[bucket], &hint_c[bucket], &answers[bucket]);
        results.push(recovered);
    }
    results
}

pub fn oracle_index(
    bucket: usize,
    record: usize,
    map: &HashMap<(usize, usize), usize>,
) -> Option<usize> {
    map.get(&(bucket, record)).copied()
}

pub fn batch_querying(
    indices: &[usize],
    position_map: &HashMap<(usize, usize), usize>,
    bucket_element_counts: &[usize],
    config: &pbc::PBCConfig,
) -> (
    Vec<QueryState>,
    Vec<Vec<Array1<Zq>>>,
    Result<HashMap<usize, usize>, String>,
) {
    let schedule = pbc::gen_schedule(config, indices).expect("Failed to generate a schedule");

    let mut rng = rand::rng();

    let bucket_to_index: HashMap<usize, usize> = schedule
        .iter()
        .map(|(index, bucket)| (*bucket, *index))
        .collect();

    let mut states: Vec<QueryState> = Vec::with_capacity(config.buckets);
    let mut queries: Vec<Vec<Array1<Zq>>> = Vec::with_capacity(config.buckets);
    let common_root = bucket_element_counts
        .first()
        .map(|elements| (*elements as f64).sqrt().ceil() as usize);
    let shared_matrix = common_root.map(compute_a);

    for j in 0..config.buckets {
        let bucket_elements = bucket_element_counts[j];
        let bucket_root = (bucket_elements as f64).sqrt().ceil() as usize;
        let bucket_capacity = (bucket_elements / SIZEOFRECORD).max(1);
        let idx_j = if let Some(index) = bucket_to_index.get(&j) {
            oracle_index(j, *index, position_map).expect("Missing oracle index")
        } else {
            rng.random_range(0..bucket_capacity)
        };

        let (state, q_vecs) = match (&shared_matrix, common_root) {
            (Some(matrix), Some(root)) if root == bucket_root => {
                query_with_matrix(idx_j, bucket_root, matrix)
            }
            _ => query(idx_j, bucket_elements),
        };
        states.push(state);
        queries.push(q_vecs);
    }

    (states, queries, Ok(schedule))
}
