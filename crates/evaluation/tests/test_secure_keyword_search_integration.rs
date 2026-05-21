#[cfg(test)]
mod integration_tests {
    use client::querying::{
        batch_querying, batch_recovering, keyword_query, recover_keyword_block, sec_keyword_query_start,
        sec_keyword_recover,
    };
    use server_pir::{
        offline_preprocess::{
            answer_secure_keyword_oprf, build_secure_keyword_setup, setup, setup_batching,
        },
        online_process::{answer_query, batch_answering},
    };
    use shared::{keyword::build_keyword_index, models::Band, pbc::PBCConfig};
    use std::env;
    use tokio_postgres::NoTls;

    #[tokio::test]
    async fn test_secure_keyword_search_pipeline_on_real_1024_row_dataset(
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

        let plain_keyword_index = build_keyword_index(&bands);
        let plain_keyword_closure = plain_keyword_index.closure();
        let plain_keyword_setup = setup(&plain_keyword_index.matrix);

        let (plain_state, plain_queries) = keyword_query(&keyword, &plain_keyword_closure)
            .expect("keyword should exist in the plaintext keyword index");
        let plain_answers = answer_query(&plain_keyword_index.matrix, &plain_queries);
        let plain_record_fetch_request = recover_keyword_block(
            &plain_state,
            plain_keyword_closure.block_cell_count(),
            &plain_keyword_setup.hint_c,
            &plain_answers,
        );

        let mut secure_setup = build_secure_keyword_setup(&bands);
        let (secure_oprf_state, secure_oprf_query) =
            sec_keyword_query_start(&keyword, &secure_setup.keyword_closure)
                .expect("keyword should normalize for secure keyword query");
        let secure_oprf_response =
            answer_secure_keyword_oprf(&mut secure_setup, &secure_oprf_query)
                .expect("secure keyword OPRF should answer");
        let (secure_state, secure_queries) = client::querying::sec_keyword_finish_query(
            secure_oprf_state,
            &secure_setup.keyword_closure,
            &secure_oprf_response,
        )
        .expect("secure keyword OPRF should recover")
        .expect("keyword should exist in the secure keyword index");
        let secure_answers = answer_query(&secure_setup.keyword_index.matrix, &secure_queries);
        let secure_record_fetch_request = sec_keyword_recover(
            &secure_state,
            secure_setup.keyword_closure.block_cell_count(),
            &secure_setup.setup_result.hint_c,
            &secure_answers,
        );

        println!("keyword: {}", keyword);
        println!(
            "plaintext recovered posting ids: {:?}",
            plain_record_fetch_request.record_ids()
        );
        println!(
            "secure recovered posting ids: {:?}",
            secure_record_fetch_request.record_ids()
        );

        assert_eq!(
            secure_record_fetch_request.record_ids(),
            plain_record_fetch_request.record_ids(),
            "secure keyword search should recover the same posting ids as plaintext keyword search"
        );

        assert!(
            !secure_record_fetch_request.is_empty(),
            "secure keyword search should recover at least one band for the chosen keyword"
        );

        let config = PBCConfig::new(1500, 3);
        let (setup_res, position_map, buckets, lifted_buckets) = setup_batching(&bands, &config);
        let bucket_element_counts: Vec<usize> = buckets.iter().map(|bucket| bucket.len()).collect();
        let (states, query_results, batch_schedule) = batch_querying(
            secure_record_fetch_request.record_ids(),
            &position_map,
            &bucket_element_counts,
            &config,
        );
        let batch_schedule = batch_schedule.expect("batch scheduling should succeed");
        let answers = batch_answering(&query_results, &lifted_buckets);
        let hint_cs: Vec<_> = setup_res.iter().map(|r| r.hint_c.clone()).collect();
        let recovered_rows = batch_recovering(
            &states,
            &answers,
            secure_record_fetch_request.record_ids(),
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
