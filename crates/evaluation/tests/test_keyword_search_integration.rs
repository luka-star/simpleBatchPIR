#[cfg(test)]
mod integration_tests {
    use keyword_search::{PlainKeywordClient, PlainKeywordServer};
    use shared::{models::Band, pbc::PBCConfig};
    use simplepir::{BatchSimplePIRClient, BatchSimplePIRServer};
    use std::env;
    use tokio_postgres::NoTls;

    #[tokio::test]
    async fn test_keyword_search_pipeline_on_real_1024_row_dataset(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
            "host=localhost user=user password=password dbname=pir_db".to_string()
        });
        let keyword = env::var("KEYWORD").unwrap_or_else(|_| "heavy".to_string());

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
        let keyword_mapping = shared::models::construct_keyword_mapping(&bands);
        let keyword_server = PlainKeywordServer::setup(&keyword_mapping);
        let keyword_closure = keyword_server.closure();

        let (state, queries) = PlainKeywordClient::query(&keyword, &keyword_closure)
            .expect("keyword should exist in the keyword index");
        let answers = keyword_server.answer(&queries);
        let record_fetch_request = PlainKeywordClient::recover(
            &state,
            &keyword_closure,
            keyword_server.pir.hint(),
            &answers,
        );

        println!("keyword: {}", keyword);
        println!(
            "normalized slot count: {}",
            keyword_server.index.perfect_hash.len()
        );
        println!(
            "recovered posting ids: {:?}",
            record_fetch_request.record_ids()
        );

        assert!(
            !record_fetch_request.is_empty(),
            "keyword search should recover at least one band for the chosen keyword"
        );

        let config = PBCConfig::new(1500, 3);
        let batch_server = BatchSimplePIRServer::setup(
            &Band::bands_to_matrix(&bands),
            Band::SIZEOFRECORD,
            &config,
        );
        let bucket_element_counts = batch_server.bucket_element_counts();
        let (states, query_results, batch_schedule) = BatchSimplePIRClient::query(
            record_fetch_request.record_ids(),
            &batch_server.position_map,
            &bucket_element_counts,
            Band::SIZEOFRECORD,
            &config,
        );
        let batch_schedule = batch_schedule.expect("batch scheduling should succeed");
        let answers = batch_server.answer(&query_results);
        let hint_cs = batch_server.hints();
        let recovered_rows = BatchSimplePIRClient::recover(
            &states,
            &answers,
            record_fetch_request.record_ids(),
            &batch_schedule,
            &hint_cs,
        );

        println!("matching records:");
        for row in &recovered_rows {
            let band = Band::unpack_band_from_zp(&row.to_vec());
            println!("{:?}", band);
        }

        Ok(())
    }
}
