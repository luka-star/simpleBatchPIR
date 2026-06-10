use ndarray::{Array2};
use std::num::Wrapping;

pub type Zq = Wrapping<u32>;
pub type Zp = Wrapping<u8>;

#[inline]
pub fn lift_matrix_to_zq(mat: &Array2<Zp>) -> Array2<Zq> {
    mat.mapv(|x| Wrapping(x.0 as u32))
}