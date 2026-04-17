pub mod keyword;
pub mod models;
pub mod pbc;
pub mod rings;
use crate::rings::*;
pub use keyword::tokenize_text;
use ndarray::Array2;
use rand::prelude::*;
use rand_chacha::ChaCha20Rng;
use std::num::Wrapping;

pub const P: usize = 1 << 8;
pub const SIZEOFRECORD: usize = 64;
pub const SEC_PARAM_N: usize = 1 << 10;
pub const Q: usize = 1 << 32;
pub const DELTA: usize = Q / P;
pub const SHARED_SEED: u64 = 42;

pub fn compute_a(root_of_n: usize) -> Array2<Zq> {
    let mut rng = ChaCha20Rng::seed_from_u64(SHARED_SEED);
    Array2::from_shape_fn((root_of_n, SEC_PARAM_N), |_| Wrapping(rng.random::<u32>()))
}
