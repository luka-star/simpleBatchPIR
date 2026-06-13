use ndarray::{Array1, Array2};
use rand::prelude::*;
use rand::rngs::OsRng;
use rand_chacha::ChaCha20Rng;
use rand_distr::{Distribution, Normal};
use shared::rings::Zp;
use shared::{compute_a, DELTA, Q, SEC_PARAM_N};
use std::num::Wrapping;

use crate::types::{
    BatchSimplePIRAnswer, BatchSimplePIRBucketOracle, BatchSimplePIRQuery, BatchSimplePIRSchedule,
    PBCConfig, SimplePIRClientState, SimplePIRHint, SimplePIRQuery, SimplePIRRecordAnswer,
    SimplePIRRecordQuery,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct SimplePIRClient;

impl SimplePIRClient {
    pub fn query_record(
        start_cell: usize,
        cell_count: usize,
        square_n: usize,
    ) -> (SimplePIRClientState, SimplePIRRecordQuery) {
        query_record(start_cell, cell_count, square_n)
    }

    pub fn recover_record(
        state: &SimplePIRClientState,
        hint: &SimplePIRHint,
        answers: &SimplePIRRecordAnswer,
    ) -> Vec<Zp> {
        recover_record(state, hint, answers)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct BatchSimplePIRClient;

impl BatchSimplePIRClient {
    pub fn query(
        indices: &[usize],
        position_map: &BatchSimplePIRBucketOracle,
        bucket_size: usize,
        record_cell_count: usize,
        config: &PBCConfig,
    ) -> Result<
        (
            Vec<SimplePIRClientState>,
            BatchSimplePIRQuery,
            BatchSimplePIRSchedule,
        ),
        String,
    > {
        batch_query(
            indices,
            position_map,
            bucket_size,
            record_cell_count,
            config,
        )
    }

    pub fn recover(
        states: &[SimplePIRClientState],
        answers: &BatchSimplePIRAnswer,
        indices: &[usize],
        schedule: &BatchSimplePIRSchedule,
        hints: &[SimplePIRHint],
    ) -> Vec<Array1<Zp>> {
        batch_recover(states, answers, indices, schedule, hints)
    }
}

fn generate_secret_and_error(
    root_of_n: usize,
) -> (Array1<shared::rings::Zq>, Array1<Wrapping<i32>>) {
    let mut seed = [0u8; 32];
    OsRng.fill_bytes(&mut seed);
    let mut rng = ChaCha20Rng::from_seed(seed);
    let s = Array1::from_shape_fn(SEC_PARAM_N, |_| rng.gen::<shared::rings::Zq>());
    let sigma = 6.4;
    let normal = Normal::new(0.0, sigma).expect("sigma must be positive");
    let e = Array1::from_shape_fn(root_of_n, |_| {
        let sample: f64 = normal.sample(&mut rng);
        Wrapping(sample.round() as i32)
    });
    (s, e)
}

fn single_query(
    i_col: usize,
    matrix: &Array2<shared::rings::Zq>,
    root_of_n: usize,
) -> (SimplePIRQuery, Array1<shared::rings::Zq>) {
    let (s, e) = generate_secret_and_error(root_of_n);
    let e_u32 = e.mapv(|x| Wrapping(x.0 as u32));
    let mut query = matrix.dot(&s) + e_u32;
    query[i_col] += Wrapping(DELTA as u32);

    (query, s)
}

fn recover_single(
    s: &Array1<shared::rings::Zq>,
    row_idx: usize,
    hint: &Array2<shared::rings::Zq>,
    answer: &Array1<shared::rings::Zq>,
) -> Zp {
    let ans_val = answer[row_idx];
    let hint_dot_s = hint.row(row_idx).dot(s);
    let mut d_hat = ans_val.0 as i64 - hint_dot_s.0 as i64;
    let q = Q as i64;
    d_hat = ((d_hat % q) + q) % q;
    let delta = DELTA as i64;
    let d = ((d_hat + (delta / 2)) / delta) as u8;
    Wrapping(d)
}

fn batch_recover(
    states: &[SimplePIRClientState],
    answers: &BatchSimplePIRAnswer,
    indices: &[usize],
    schedule: &BatchSimplePIRSchedule,
    hints: &[SimplePIRHint],
) -> Vec<Array1<Zp>> {
    let mut results = Vec::new();

    for index in indices {
        let bucket = schedule[index];
        let recovered = Array1::from_vec(recover_record(
            &states[bucket],
            &hints[bucket],
            &answers[bucket],
        ));
        results.push(recovered);
    }
    results
}

fn oracle_index(bucket: usize, record: usize, map: &BatchSimplePIRBucketOracle) -> Option<usize> {
    map.get(&(bucket, record)).copied()
}

fn batch_query(
    indices: &[usize],
    position_map: &BatchSimplePIRBucketOracle,
    bucket_size: usize,
    record_cell_count: usize,
    config: &PBCConfig,
) -> Result<
    (
        Vec<SimplePIRClientState>,
        BatchSimplePIRQuery,
        BatchSimplePIRSchedule,
    ),
    String,
> {
    let bucket_root = (bucket_size as f64).sqrt().ceil() as usize;
    let a_matrix: Array2<shared::rings::Zq> = compute_a(bucket_root);
    let schedule = shared::pbc::gen_schedule(config, indices)?;
    let mut rng = rand::thread_rng();

    let bucket_to_index: BatchSimplePIRSchedule = schedule
        .iter()
        .map(|(index, bucket)| (*bucket, *index))
        .collect();

    let mut states = Vec::with_capacity(config.buckets);
    let mut queries = Vec::with_capacity(config.buckets);
    for bucket in 0..config.buckets {
        let bucket_capacity = (bucket_size / record_cell_count).max(1);
        let record_index = if let Some(index) = bucket_to_index.get(&bucket) {
            oracle_index(bucket, *index, position_map).expect("Missing oracle index")
        } else {
            rng.gen_range(0..bucket_capacity)
        };
        let block_start_cell = record_index * record_cell_count;

        let (state, q_vecs) =
            query_record_with_matrix(block_start_cell, record_cell_count, bucket_root, &a_matrix);
        states.push(state);
        queries.push(q_vecs);
    }

    Ok((states, queries, schedule))
}

fn block_positions(start_cell: usize, cell_count: usize, square_n: usize) -> Vec<(usize, usize)> {
    (0..cell_count)
        .map(|offset| {
            let cell_index = start_cell + offset;
            (cell_index / square_n, cell_index % square_n)
        })
        .collect()
}

fn query_record(
    start_cell: usize,
    cell_count: usize,
    square_n: usize,
) -> (SimplePIRClientState, SimplePIRRecordQuery) {
    let a_matrix: Array2<shared::rings::Zq> = compute_a(square_n);
    query_record_with_matrix(start_cell, cell_count, square_n, &a_matrix)
}

fn query_record_with_matrix(
    start_cell: usize,
    cell_count: usize,
    square_n: usize,
    a_matrix: &Array2<shared::rings::Zq>,
) -> (SimplePIRClientState, SimplePIRRecordQuery) {
    let mut queried_columns = Vec::with_capacity(cell_count.min(square_n));
    for (_, col_idx) in block_positions(start_cell, cell_count, square_n) {
        if !queried_columns.contains(&col_idx) {
            queried_columns.push(col_idx);
        }
    }

    let mut whole_query = Vec::with_capacity(queried_columns.len());
    let mut secrets = Vec::with_capacity(queried_columns.len());

    for &i_col in &queried_columns {
        let (query, secret) = single_query(i_col, a_matrix, square_n);
        whole_query.push(query);
        secrets.push(secret);
    }

    let state = SimplePIRClientState {
        s: secrets,
        queried_columns,
        start_cell,
        cell_count,
        square_n,
    };

    (state, whole_query)
}

fn recover_record(
    state: &SimplePIRClientState,
    hint: &SimplePIRHint,
    answers: &SimplePIRRecordAnswer,
) -> Vec<Zp> {
    let positions = block_positions(state.start_cell, state.cell_count, state.square_n);
    let mut recovered = Array1::zeros(state.cell_count);

    for (offset, (row_idx, col_idx)) in positions.into_iter().enumerate() {
        let answer_idx = state
            .queried_columns
            .iter()
            .position(|&queried_col| queried_col == col_idx)
            .expect("missing answer for queried record column");
        recovered[offset] =
            recover_single(&state.s[answer_idx], row_idx, hint, &answers[answer_idx]);
    }

    recovered.to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn batch_query_returns_schedule_errors() {
        let config = PBCConfig::fixed_seeds(1, 1);
        let position_map = BatchSimplePIRBucketOracle::new();

        let result = BatchSimplePIRClient::query(&[0, 1], &position_map, 1, 1, &config);

        assert_eq!(
            result.unwrap_err(),
            "Cannot schedule more indices than buckets"
        );
    }
}
