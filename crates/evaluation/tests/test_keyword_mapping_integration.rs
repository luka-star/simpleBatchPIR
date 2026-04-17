#[cfg(test)]
mod integration_tests {
    use shared::keyword::construct_keyword_mapping;
    use shared::models::Band;
    use std::{collections::BTreeMap, env};
    use tokio_postgres::NoTls;

    #[tokio::test]
    async fn test_keyword_mapping_on_real_1024_row_dataset(
    ) -> Result<(), Box<dyn std::error::Error>> {
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

        let query_string = "SELECT band_index, band_name, fans, formed, style, origin, split \
                            FROM data_10 ORDER BY band_index ASC";
        let rows = client.query(query_string, &[]).await?;

        assert_eq!(
            rows.len(),
            1024,
            "expected the real dataset size to be 1024 rows"
        );

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

        let mapping = construct_keyword_mapping(&bands);

        let sorted_mapping: BTreeMap<String, Vec<usize>> = mapping
            .into_iter()
            .map(|(keyword, mut band_ids)| {
                band_ids.sort_unstable();
                (keyword, band_ids)
            })
            .collect();

        println!("keyword mapping size: {}", sorted_mapping.len());

        assert!(
            !sorted_mapping.is_empty(),
            "keyword mapping should not be empty for the real dataset"
        );

        Ok(())
    }
}
