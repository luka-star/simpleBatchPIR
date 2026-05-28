#[cfg(test)]
mod integration_tests {
    use postgres::NoTls;
    use shared::models::construct_keyword_mapping;
    use shared::models::Band;
    use std::{collections::BTreeMap, env};

    #[test]
    fn test_keyword_mapping_on_real_1024_row_dataset() -> Result<(), Box<dyn std::error::Error>> {
        let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
            "host=localhost user=user password=password dbname=pir_db".to_string()
        });

        let mut client =
            postgres::Client::connect(&database_url, NoTls).expect("Failed to connect to Postgres");

        let query_string = "SELECT band_index, band_name, country, genre, status \
                            FROM data_10 ORDER BY band_index ASC";
        let rows = client.query(query_string, &[])?;

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
                country: row.get(2),
                genre: row.get(3),
                status: row.get(4),
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
