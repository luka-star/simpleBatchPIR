use ndarray::Array2;
use shared::compute_a;
use shared::models::Band;
use shared::pbc::PBCConfig;
use shared::rings::{lift_matrix_to_zq, Zp, Zq};
use std::collections::HashMap;
use std::num::Wrapping;

use super::plain::{setup_with_matrix, SetupResult};

pub fn batching_encode(db: &[Band],config: &PBCConfig) -> (Vec<Vec<Band>>, HashMap<(usize, usize), usize>) {
    let b = config.buckets;
    let w = config.w();

    let m = (db.len() * w) / b;
    let mut buckets: Vec<Vec<Band>> = vec![Vec::with_capacity(m); b];
    let mut position_map: HashMap<(usize, usize), usize> = HashMap::new();

    for record in db {
        let candidates = config.bucket_positions(&record.id);
        for &bucket_idx in &candidates {
            let pos = buckets[bucket_idx].len();
            buckets[bucket_idx].push(record.clone());
            position_map.insert((bucket_idx, record.id as usize), pos);
        }
    }
    (buckets, position_map)
}

pub fn setup_batching(db: &[Band],config: &PBCConfig) -> (Vec<SetupResult>,HashMap<(usize, usize), usize>,Vec<Array2<Zp>>,Vec<Array2<Zq>>,) {
    let (buckets, position_map) = batching_encode(db, config);
    let mut setup_res: Vec<SetupResult> = Vec::with_capacity(buckets.len());
    let mut encode_buckets: Vec<Array2<Zp>> = Vec::with_capacity(buckets.len());

    for bucket_data in &buckets {
        let matrix: Array2<Zp> = Band::bands_to_matrix(bucket_data);
        encode_buckets.push(matrix);
    }

    let padded_buckets = pad_buckets(encode_buckets);
    let lifted_buckets: Vec<Array2<Zq>> = padded_buckets.iter().map(lift_matrix_to_zq).collect();

    if let Some(first_bucket) = padded_buckets.first() {
        let matrix = compute_a(first_bucket.nrows());
        for bucket in &padded_buckets {
            let setup = setup_with_matrix(bucket, &matrix);
            setup_res.push(setup);
        }
    }

    (setup_res, position_map, padded_buckets, lifted_buckets)
}

pub fn pad_buckets(buckets: Vec<Array2<Zp>>) -> Vec<Array2<Zp>> {
    let max_size = buckets.iter().map(|b| b.len()).max().unwrap_or(0);

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
