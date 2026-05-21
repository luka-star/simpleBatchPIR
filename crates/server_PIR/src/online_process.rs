use ndarray::{Array1, Array2};
use shared::rings::{lift_matrix_to_zq, Zp, Zq};

pub fn answer_query(db: &Array2<Zp>, queries: &[Array1<Zq>]) -> Vec<Array1<Zq>> {
    let db_lifted: Array2<Zq> = lift_matrix_to_zq(db);
    queries.iter().map(|q| db_lifted.dot(q)).collect()
}

pub fn batch_answering(
    queries: &[Vec<Array1<Zq>>],
    buckets: &[Array2<Zq>],
) -> Vec<Vec<Array1<Zq>>> {
    buckets
        .iter()
        .zip(queries.iter())
        .map(|(bucket, query_bundle)| query_bundle.iter().map(|query| bucket.dot(query)).collect())
        .collect()
}
