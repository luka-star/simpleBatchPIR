mod support;

use client::querying;
use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use server_pir::{offline_preprocess, online_process};
use shared::models::Band;
use std::env;
use std::path::Path;
use support::{assert_requested_band_count, make_bands, random_idx};
use tokio::runtime::Runtime;
use tokio_postgres::NoTls;

criterion_group! {
    name = benches;
    config = Criterion::default()
        .output_directory(Path::new("benchmark-results"))
        .measurement_time(std::time::Duration::from_secs(20));
    targets =
        query_dbsize,
}

const DB_SIZE: [usize; 10] = [1, 64, 128, 256, 512, 1024, 2048, 4096, 8192, 16384];

fn query_dbsize(c: &mut Criterion) {
    let mut group = c.benchmark_group("querying");

    for nr_bands in DB_SIZE {
        let rt = Runtime::new().unwrap();
        let bands = rt
            .block_on(make_bands(nr_bands))
            .expect("Failed to fetch bands");
        assert_requested_band_count(nr_bands, bands.len());
        let db_matrix = Band::bands_to_matrix(&bands);
        let n_elements = bands.len() * Band::SIZEOFRECORD;
        let setup_res = offline_preprocess::setup(&db_matrix);
        let actual_band_count = bands.len();

        group.bench_with_input(
            BenchmarkId::new("pir", nr_bands),
            &nr_bands,
            |b, &_nr_bands| {
                b.iter(|| {
                    let (secrets, queries) = querying::query(
                        black_box(random_idx(actual_band_count)),
                        black_box(n_elements),
                    );
                    let answers =
                        online_process::answer_query(black_box(&db_matrix), black_box(&queries));
                    querying::recover(
                        black_box(&secrets),
                        black_box(&setup_res.hint_c),
                        black_box(&answers),
                    );
                })
            },
        );
    }

    let rt = Runtime::new().unwrap();
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "host=localhost user=user password=password dbname=pir_db".to_string());

    let (client, connection) = rt
        .block_on(tokio_postgres::connect(&database_url, NoTls))
        .expect("Failed to connect to Postgres");

    rt.spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("Database error: {}", e);
        }
    });

    for nr_bands in DB_SIZE {
        let target_idx = random_idx(nr_bands);
        let sql = format!(
            "SELECT band_index, band_name, fans, formed, style, origin, split
             FROM data_10
             WHERE band_index = {target_idx}
             ORDER BY band_index ASC
             LIMIT 1"
        );

        group.bench_with_input(
            BenchmarkId::new("sql", nr_bands),
            &nr_bands,
            |b, &_nr_bands| {
                b.iter(|| {
                    rt.block_on(async {
                        let rows = client.query(&sql, &[]).await.expect("Query failed");

                        black_box(rows);
                    })
                })
            },
        );
    }

    group.finish();
}

criterion_main!(benches);
