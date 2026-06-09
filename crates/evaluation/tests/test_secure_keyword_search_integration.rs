#[cfg(test)]
mod integration_tests {
    use keyword_search::{
        PlainKeywordClient, PlainKeywordServer, SecureKeywordClient, SecureKeywordServer,
    };
    use postgres::NoTls;
    use shared::{models::Band, pbc::PBCConfig};
    use simplepir::{BatchSimplePIRClient, BatchSimplePIRServer};
    use std::env;

    #[test]
    fn test_secure_keyword_search_pipeline_on_real_1024_row_dataset(
    ) -> Result<(), Box<dyn std::error::Error>> {
        let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
            "host=localhost user=user password=password dbname=pir_db".to_string()
        });
        let keywords = [env::var("KEYWORD").unwrap_or_else(|_| "heavy".to_string())];
        let keyword = keywords
            .first()
            .expect("secure keyword query should contain at least one keyword");

        let mut client =
            postgres::Client::connect(&database_url, NoTls).expect("Failed to connect to Postgres");

        let query_string = "SELECT band_index, band_name, country, genre, status \
                            FROM data_10 ORDER BY band_index ASC";
        let rows = client.query(query_string, &[])?;

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

        let keyword_mapping = shared::models::construct_keyword_mapping(&bands);

        let plain_server = PlainKeywordServer::setup(&keyword_mapping);
        let plain_client_context = plain_server.client_context();

        let (plain_state, plain_queries) =
            PlainKeywordClient::query(keyword, &plain_client_context)
                .expect("keyword should exist in the plaintext keyword index");
        let plain_answers = plain_server.answer(&plain_queries);
        let plain_record_fetch_request =
            PlainKeywordClient::recover(&plain_state, plain_server.pir.hint(), &plain_answers);

        let mut secure_server = SecureKeywordServer::setup(&keyword_mapping);
        let secure_client_context = secure_server.client_context();
        let (secure_oprf_state, secure_oprf_query) =
            SecureKeywordClient::start_oprf(&keywords, &secure_client_context)
                .expect("keyword should normalize for secure keyword query");
        let secure_oprf_response = secure_server
            .answer_oprf(&secure_oprf_query)
            .expect("secure keyword OPRF should answer");
        let (secure_state, secure_queries) = SecureKeywordClient::query(
            secure_oprf_state,
            &secure_client_context,
            &secure_oprf_response,
        )
        .into_iter()
        .next()
        .expect("secure keyword OPRF should return the first keyword result")
        .expect("keyword should exist in the secure keyword index");
        let secure_answers = secure_server.answer(&secure_queries);
        let secure_record_fetch_request = SecureKeywordClient::recover(
            &secure_state,
            secure_server.setup.pir.hint(),
            &secure_answers,
        );

        println!("keyword: {}", keyword);
        println!(
            "plaintext recovered record indices: {:?}",
            plain_record_fetch_request
        );
        println!(
            "secure recovered record indices: {:?}",
            secure_record_fetch_request
        );

        assert_eq!(
            secure_record_fetch_request, plain_record_fetch_request,
            "secure keyword search should recover the same record indices as plaintext keyword search"
        );

        assert!(
            !secure_record_fetch_request.is_empty(),
            "secure keyword search should recover at least one band for the chosen keyword"
        );

        let config = PBCConfig::random_seeds(1500, 3);
        let batch_server = BatchSimplePIRServer::setup(
            &Band::bands_to_matrix(&bands),
            Band::SIZEOFRECORD,
            &config,
        );
        let bucket_size = batch_server.bucket_size();
        let (states, query_results, batch_schedule) = BatchSimplePIRClient::query(
            &secure_record_fetch_request,
            &batch_server.position_map,
            bucket_size,
            Band::SIZEOFRECORD,
            &config,
        )
        .expect("batch scheduling should succeed");
        let answers = batch_server.answer(&query_results);
        let hint_cs = batch_server.hints();
        let recovered_rows = BatchSimplePIRClient::recover(
            &states,
            &answers,
            &secure_record_fetch_request,
            &batch_schedule,
            &hint_cs,
        );

        println!("matching records:");
        for row in &recovered_rows {
            let band = Band::unpack_band_from_zp(&row.to_vec());
            println!("{:?}", band);
        }

        assert!(
            !recovered_rows.is_empty(),
            "secure keyword search should recover at least one full record"
        );

        Ok(())
    }
}
