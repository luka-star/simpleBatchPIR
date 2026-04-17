#[cfg(test)]
mod integration_tests {
    use client::querying::{
        batch_querying, batch_recovering, keyword_query, recover_keyword_block,
    };
    use server_pir::{
        offline_preprocess::{setup, setup_batching},
        online_process::{answer_query, batch_answering},
    };
    use shared::{keyword::build_keyword_index, models::Band, pbc::PBCConfig};
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
        // First, let the server tokenize, map the tokens and build a PIR-able matrix based on the DB
        let keyword_index = build_keyword_index(&bands);
        let keyword_closure = keyword_index.closure();
        let keyword_setup = setup(&keyword_index.matrix);

        // Query the set database the carries the keywords -> list(indices)
        let (state, queries) = keyword_query(&keyword, &keyword_closure)
            .expect("keyword should exist in the keyword index");
        let answers = answer_query(&keyword_index.matrix, &queries);
        let record_fetch_request = recover_keyword_block(
            &state,
            keyword_closure.block_cell_count(),
            &keyword_setup.hint_c,
            &answers,
        );

        println!("keyword: {}", keyword);
        println!(
            "normalized slot count: {}",
            keyword_index.perfect_hash.len()
        );
        println!(
            "recovered posting ids: {:?}",
            record_fetch_request.record_ids()
        );

        assert!(
            !record_fetch_request.is_empty(),
            "keyword search should recover at least one band for the chosen keyword"
        );

        // use batchPIR to query the list of indices.
        let config = PBCConfig::new(1500, 3);
        let (setup_res, position_map, buckets, lifted_buckets) = setup_batching(&bands, &config);
        let bucket_element_counts: Vec<usize> = buckets.iter().map(|bucket| bucket.len()).collect();
        let (states, query_results, batch_schedule) = batch_querying(
            record_fetch_request.record_ids(),
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
