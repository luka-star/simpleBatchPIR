mod support;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use shared::models::Band;
use simplepir::{SimplePIRClient, SimplePIRServer};
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
        let pir_server = SimplePIRServer::setup(Band::bands_to_matrix(&bands));
        let actual_band_count = bands.len();

        group.bench_with_input(
            BenchmarkId::new("pir", nr_bands),
            &nr_bands,
            |b, &_nr_bands| {
                b.iter(|| {
                    let record_index = black_box(random_idx(actual_band_count));
                    let block_start_cell = record_index * Band::SIZEOFRECORD;
                    let (secrets, queries) = SimplePIRClient::query_record(
                        black_box(block_start_cell),
                        black_box(Band::SIZEOFRECORD),
                        black_box(pir_server.square_n()),
                    );
                    let answers = pir_server.answer(black_box(&queries));
                    SimplePIRClient::recover_record(
                        black_box(&secrets),
                        black_box(pir_server.hint()),
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
