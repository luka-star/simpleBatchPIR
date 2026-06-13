#![allow(dead_code)]

use postgres::{Error, NoTls};
use rand::{seq::index::sample, Rng};
use shared::models::Band;
use std::env;

pub fn table_for_size(db_size: usize) -> &'static str {
    match db_size {
        1..=1024 => "data_10",
        1025..=2048 => "data_11",
        2049..=4096 => "data_12",
        4097..=8192 => "data_13",
        8193..=16384 => "data_14",
        16385..=32768 => "data_15",
        32769..=65536 => "data_16",
        65537..=131072 => "data_17",
        _ => panic!("unsupported SQL benchmark database size: {db_size}"),
    }
}

pub fn make_bands(nr_bands: usize) -> Result<Vec<Band>, Error> {
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "host=127.0.0.1 user=user password=password dbname=pir_db".to_string());

    let mut client = postgres::Client::connect(&database_url, NoTls)?;

    let table_name = table_for_size(nr_bands);
    let query = format!(
        "SELECT band_index, band_name, country, genre, status
         FROM (
             SELECT
                 band_index,
                 band_name,
                 country,
                 genre,
                 status
             FROM {table_name}
         ) src
         ORDER BY band_index ASC
         LIMIT $1"
    );

    let rows = client.query(&query, &[&(nr_bands as i64)])?;

    let bands: Vec<Band> = rows
        .iter()
        .map(|row| Band {
            id: row.get(0),
            name: row.get(1),
            country: row.get(2),
            genre: row.get(3),
            status: row.get(4),
        })
        .collect();

    Ok(bands)
}

pub fn assert_requested_band_count(requested: usize, actual: usize) {
    assert_eq!(
        actual, requested,
        "expected {requested} bands from benchmark fixture, got {actual}"
    );
}

pub fn random_idx(upper: usize) -> usize {
    assert!(upper > 0, "upper bound must be positive");
    rand::thread_rng().gen_range(0..upper)
}

pub fn random_index_list(count: usize, upper: usize) -> Vec<usize> {
    assert!(
        count <= upper,
        "cannot sample {count} unique indices from upper bound {upper}"
    );

    if count == 0 {
        return Vec::new();
    }

    sample(&mut rand::thread_rng(), upper, count).into_vec()
}
