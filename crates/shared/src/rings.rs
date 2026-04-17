use ndarray::{Array1, Array2};
use std::num::Wrapping;

pub type Zq = Wrapping<u32>;
pub type Zp = Wrapping<u8>;

pub trait LiftToZq {
    fn lift_to_zq(self) -> Zq;
}

impl LiftToZq for Zp {
    #[inline]
    fn lift_to_zq(self) -> Zq {
        Wrapping(self.0 as u32)
    }
}

pub trait ReduceToZp {
    fn reduce_to_zp(self) -> Zp;
}

impl ReduceToZp for Zq {
    #[inline]
    fn reduce_to_zp(self) -> Zp {
        Wrapping((self.0 % 256) as u8)
    }
}

#[inline]
pub fn lift_matrix_to_zq(mat: &Array2<Zp>) -> Array2<Zq> {
    mat.mapv(|x| x.lift_to_zq())
}

#[inline]
pub fn lift_vector_to_zq(vec: &Array1<Zp>) -> Array1<Zq> {
    vec.mapv(|x| x.lift_to_zq())
}

#[inline]
pub fn reduce_matrix_to_zp(mat: &Array2<Zq>) -> Array2<Zp> {
    mat.mapv(|x| x.reduce_to_zp())
}

#[inline]
pub fn reduce_vector_to_zp(vec: &Array1<Zq>) -> Array1<Zp> {
    vec.mapv(|x| x.reduce_to_zp())
}
