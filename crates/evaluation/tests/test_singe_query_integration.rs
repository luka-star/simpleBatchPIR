#[cfg(test)]
mod integration_tests {
    use client::querying::{query, recover};
    use server_pir::{offline_preprocess::setup, online_process::answer_query};
    use shared::models::Band;
    use std::env;
    use tokio_postgres::NoTls;

    #[tokio::test]
    async fn test_full_pir_pipeline() -> Result<(), Box<dyn std::error::Error>> {
        let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
            "host=localhost user=user password=password dbname=pir_db".to_string()
        });

        let (client, connection) = tokio_postgres::connect(&database_url, NoTls)
            .await
            .expect("Failed to connect to Postgres");

        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("Database error: {}", e);
            }
        });

        let exp = 10;

        let query_string = format!(
            "SELECT band_index, band_name, fans, formed, style, origin, split FROM data_{exp} ORDER BY band_index ASC"
        );

        let rows = client.query(&query_string, &[]).await?;

        println!("Number of rows: {:?}", rows.len());

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

        let db_matrix = Band::bands_to_matrix(&bands);
        let setup_res = setup(&db_matrix);

        let target_band_idx = 979;

        let original_band = &bands[target_band_idx];

        let matrix_n = db_matrix.len();
        let (secrets, queries) = query(target_band_idx, matrix_n);
        let answers = answer_query(&db_matrix, &queries);

        let recovered_record = recover(&secrets, &setup_res.hint_c, &answers);
        let desired_band = Band::pack_band_to_zp(original_band);

        println!("The original band: {:?}", desired_band);

        println!("Recovered: {:?}", recovered_record);

        assert_eq!(recovered_record.len(), desired_band.len());

        let recovered_band = Band::unpack_band_from_zp(&recovered_record.to_vec());
        println!("The final recovered band: {:?}", recovered_band);
        assert_eq!(recovered_band.id, original_band.id);

        Ok(())
    }
}
