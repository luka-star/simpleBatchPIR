#[cfg(test)]
mod integration_tests {
    use postgres::NoTls;
    use shared::{
        models::Band,
        pbc::{self, gen_schedule},
    };
    use simplepir::{BatchSimplePIRClient, BatchSimplePIRServer};
    use std::{collections::HashSet, env};

    fn assert_bucket_shapes(
        hints: &[simplepir::types::SimplePIRHint],
        buckets: &[ndarray::Array2<shared::rings::Zp>],
        bucket_count: usize,
    ) {
        assert_eq!(
            hints.len(),
            bucket_count,
            "setup must return one result per bucket"
        );
        assert_eq!(
            buckets.len(),
            bucket_count,
            "encoding must return one matrix per bucket"
        );

        for (bucket_idx, (hint, bucket)) in hints.iter().zip(buckets.iter()).enumerate() {
            assert_eq!(
                bucket.nrows(),
                bucket.ncols(),
                "bucket {bucket_idx} is not square"
            );
            assert_eq!(
                hint.nrows(),
                bucket.nrows(),
                "bucket {bucket_idx} hint rows must match bucket rows"
            );
            assert_eq!(
                hint.ncols(),
                shared::SEC_PARAM_N,
                "bucket {bucket_idx} hint width must match security parameter"
            );
        }
    }

    fn assert_oracle_entries(
        bands: &[Band],
        target_indices: &[usize],
        config: &pbc::PBCConfig,
        position_map: &std::collections::HashMap<(usize, usize), usize>,
        buckets: &[ndarray::Array2<shared::rings::Zp>],
    ) {
        for &target_idx in target_indices {
            let original = &bands[target_idx];
            for bucket_idx in config.bucket_positions(&original.id) {
                let pos = *position_map
                    .get(&(bucket_idx, original.id as usize))
                    .expect("missing oracle entry for replicated record");

                let recovered = shared::models::Band::matrix_to_bands(&buckets[bucket_idx]);
                assert!(
                    pos < recovered.len(),
                    "oracle position {} is out of range for bucket {}",
                    pos,
                    bucket_idx
                );
                assert_eq!(
                    recovered[pos].id, original.id,
                    "oracle points to the wrong record in bucket {}",
                    bucket_idx
                );
            }
        }
    }

    fn assert_schedule_invariants(
        targets: &[usize],
        config: &pbc::PBCConfig,
        schedule: &std::collections::HashMap<usize, usize>,
    ) {
        let mut used_buckets = HashSet::new();

        for &target in targets {
            let bucket = *schedule.get(&target).expect("missing scheduled bucket");
            assert!(
                config.bucket_positions(&target).contains(&bucket),
                "scheduled bucket {bucket} is not a candidate for target {target}"
            );
            assert!(
                used_buckets.insert(bucket),
                "scheduled bucket {bucket} was reused"
            );
        }
    }

    #[test]
    fn test_batch_pir_pipeline() -> Result<(), Box<dyn std::error::Error>> {
        let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
            "host=localhost user=user password=password dbname=pir_db".to_string()
        });

        let mut client =
            postgres::Client::connect(&database_url, NoTls).expect("Failed to connect to Postgres");

        let exp = 10;

        let query_string = format!(
            "SELECT band_index, band_name, country, genre, status FROM data_{exp} ORDER BY band_index ASC"
        );

        let rows = client.query(&query_string, &[])?;

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

        // as described in the paper.
        let w: usize = 3;
        let b: usize = 1500;

        let new_config: pbc::PBCConfig = pbc::PBCConfig::random_seeds(b, w);
        let batch_server = BatchSimplePIRServer::setup(
            &Band::bands_to_matrix(&bands),
            Band::SIZEOFRECORD,
            &new_config,
        );

        let target_bands_idx = [1, 42, 320];
        assert_bucket_shapes(&batch_server.hints, &batch_server.buckets, b);
        assert_oracle_entries(
            &bands,
            &target_bands_idx,
            &new_config,
            &batch_server.position_map,
            &batch_server.buckets,
        );

        let schedule = gen_schedule(&new_config, &target_bands_idx)
            .expect("test targets should admit a batching schedule");
        assert_schedule_invariants(&target_bands_idx, &new_config, &schedule);

        let bucket_size = batch_server.bucket_size();
        let (states, query_results, batch_schedule) = BatchSimplePIRClient::query(
            &target_bands_idx,
            &batch_server.position_map,
            bucket_size,
            Band::SIZEOFRECORD,
            &new_config,
        )
        .expect("batch querying should surface schedule success");

        assert_eq!(
            batch_schedule, schedule,
            "batch querying should preserve the chosen schedule"
        );
        assert_eq!(
            states.len(),
            b,
            "client state should contain one entry per bucket"
        );
        assert_eq!(
            query_results.len(),
            b,
            "batch querying should return one logical CPIR query per bucket"
        );
        assert!(
            query_results.iter().all(|bundle| !bundle.is_empty()),
            "each bucket should contain a full SimplePIR query bundle"
        );

        let answers = batch_server.answer(&query_results);
        let hints = batch_server.hints();

        assert_eq!(
            answers.len(),
            b,
            "batch answering should return one answer per bucket"
        );
        assert!(
            answers.iter().all(|bundle| !bundle.is_empty()),
            "each bucket should produce a full SimplePIR answer bundle"
        );

        let recovered_row = BatchSimplePIRClient::recover(
            &states,
            &answers,
            &target_bands_idx,
            &batch_schedule,
            &hints,
        );

        assert_eq!(
            recovered_row.len(),
            target_bands_idx.len(),
            "batch recovering should return one codeword per target"
        );
        for row in &recovered_row {
            assert_eq!(
                row.len(),
                Band::SIZEOFRECORD,
                "each extracted codeword should contain a full record"
            );
        }

        for row in &recovered_row {
            println!("{:?}", row);
        }

        Ok(())
    }
}
