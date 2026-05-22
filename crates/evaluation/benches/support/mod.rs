#![allow(dead_code)]

use rand::{seq::index::sample, Rng};
use shared::models::Band;
use std::env;
use tokio_postgres::{Error, NoTls};

fn table_for_size(nr_bands: usize) -> String {
    match nr_bands {
        1 => "data_0".to_string(),
        _ => "data_all".to_string(),
    }
}

pub async fn make_bands(nr_bands: usize) -> Result<Vec<Band>, Error> {
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "host=localhost user=user password=password dbname=pir_db".to_string());

    let (client, connection) = tokio_postgres::connect(&database_url, NoTls).await?;

    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("Database connection error: {}", e);
        }
    });

    let table_name = table_for_size(nr_bands);
    let query = format!(
        "SELECT band_index, band_name, fans, formed, style, origin, split
         FROM (
             SELECT
                 band_index,
                 band_name,
                 fans::INT AS fans,
                 formed::INT AS formed,
                 style,
                 origin,
                 split::INT AS split
             FROM {table_name}
         ) src
         ORDER BY band_index ASC
         LIMIT $1"
    );

    let rows = client.query(&query, &[&(nr_bands as i64)]).await?;

    let bands: Vec<Band> = rows
        .iter()
        .map(|row| Band {
            id: row.get(0),
            name: row.get(1),
            fans: row.get(2),
            formed: row.get(3),
            style: row.get(4),
            origin: row.get(5),
            split: row.get(6),
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
