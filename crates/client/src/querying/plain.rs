use ndarray::{Array1, Array2};
use rand::prelude::*;
use rand_chacha::ChaCha20Rng;
use rand_distr::{Distribution, Normal};
use shared::rings::{Zp, Zq};
use shared::{compute_a, DELTA, Q, SEC_PARAM_N, SIZEOFRECORD};
use std::num::Wrapping;

#[allow(dead_code)]
pub struct QueryState {
    pub s: Vec<Array1<Zq>>,
    pub row_index: usize,
    pub record_index: usize,
}

pub(crate) fn get_row(i: usize, root_of_n: usize) -> usize {
    let global_idx = i * SIZEOFRECORD;
    global_idx / root_of_n
}

pub(crate) fn get_record_positions(i: usize, root_of_n: usize) -> Vec<(usize, usize)> {
    let global_start = i * SIZEOFRECORD;
    (0..SIZEOFRECORD)
        .map(|offset| {
            let global_idx = global_start + offset;
            (global_idx / root_of_n, global_idx % root_of_n)
        })
        .collect()
}

pub fn generate_secret_and_error(root_of_n: usize) -> (Array1<Zq>, Array1<Wrapping<i32>>) {
    let mut seed = [0u8; 32];
    rand::rng().fill_bytes(&mut seed);
    let mut rng = ChaCha20Rng::from_seed(seed);
    let s = Array1::from_shape_fn(SEC_PARAM_N, |_| rng.random::<Zq>());
    let sigma = 6.4;
    let normal = Normal::new(0.0, sigma).expect("Invalid distribution parameters");
    let e = Array1::from_shape_fn(root_of_n, |_| {
        let sample: f64 = normal.sample(&mut rng);
        Wrapping(sample.round() as i32)
    });
    (s, e)
}

pub fn query(band_idx: usize, n_elements: usize) -> (QueryState, Vec<Array1<Zq>>) {
    let root_of_n = (n_elements as f64).sqrt().ceil() as usize;
    let matrix: Array2<Zq> = compute_a(root_of_n);
    query_with_matrix(band_idx, root_of_n, &matrix)
}

pub(crate) fn query_with_matrix(
    band_idx: usize,
    root_of_n: usize,
    matrix: &Array2<Zq>,
) -> (QueryState, Vec<Array1<Zq>>) {
    let mut whole_query = Vec::with_capacity(root_of_n);
    let mut secrets = Vec::with_capacity(root_of_n);
    let i_row = get_row(band_idx, root_of_n);
    for i_col in 0..root_of_n {
        let (qu, s) = single_query(i_col, &matrix, root_of_n);
        whole_query.push(qu);
        secrets.push(s);
    }
    let state = QueryState {
        s: secrets,
        row_index: i_row,
        record_index: band_idx,
    };
    (state, whole_query)
}

pub fn single_query(
    i_col: usize,
    matrix: &Array2<Zq>,
    root_of_n: usize,
) -> (Array1<Zq>, Array1<Zq>) {
    let (s, e) = generate_secret_and_error(root_of_n);
    let e_u32 = e.mapv(|x| Wrapping(x.0 as u32));
    let mut qu = matrix.dot(&s) + e_u32;
    qu[i_col] += Wrapping(DELTA as u32);

    (qu, s)
}

pub fn recover_single(s: &Array1<Zq>, row_idx: usize, hint_c: &Array2<Zq>, ans: &Array1<Zq>) -> Zp {
    let ans_val: Zq = ans[row_idx];
    let hint_dot_s: Zq = hint_c.row(row_idx).dot(s);
    let mut d_hat = ans_val.0 as i64 - hint_dot_s.0 as i64;
    let q = Q as i64;
    d_hat = ((d_hat % q) + q) % q;
    let delta = DELTA as i64;
    let d = ((d_hat + (delta / 2)) / delta) as u8;
    Wrapping(d)
}

pub fn recover(state: &QueryState, hint_c: &Array2<Zq>, answers: &[Array1<Zq>]) -> Array1<Zp> {
    let root_of_n = answers.len();
    let positions = get_record_positions(state.record_index, root_of_n);
    let mut recovered = Array1::zeros(SIZEOFRECORD);

    for (offset, (row_idx, col_idx)) in positions.into_iter().enumerate() {
        recovered[offset] = recover_single(&state.s[col_idx], row_idx, hint_c, &answers[col_idx]);
    }
    recovered
}
