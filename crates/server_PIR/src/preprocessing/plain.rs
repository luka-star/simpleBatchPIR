use ndarray::Array2;
use shared::compute_a;
use shared::rings::{lift_matrix_to_zq, Zp, Zq};

pub struct SetupResult {
    pub hint_s: (),
    pub hint_c: Array2<Zq>,
}

pub fn setup(db: &Array2<Zp>) -> SetupResult {
    let nrows = db.nrows();
    let matrix: Array2<Zq> = compute_a(nrows);
    setup_with_matrix(db, &matrix)
}

pub(crate) fn setup_with_matrix(db: &Array2<Zp>, matrix: &Array2<Zq>) -> SetupResult {
    let db_lifted: Array2<Zq> = lift_matrix_to_zq(db);
    let hint_c = db_lifted.dot(matrix);

    SetupResult { hint_s: (), hint_c }
}
