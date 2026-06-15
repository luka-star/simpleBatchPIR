#[cfg(test)]
mod integration_tests {
    use postgres::NoTls;
    use shared::models::Band;
    use simplepir::{SimplePIRClient, SimplePIRServer};
    use std::env;

    #[test]
    fn test_full_pir_pipeline() -> Result<(), Box<dyn std::error::Error>> {
        let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
            "host=127.0.0.1 user=user password=password dbname=pir_db".to_string()
        });

        let mut client =
            postgres::Client::connect(&database_url, NoTls).expect("Failed to connect to Postgres");

        let exp = 10;

        let query_string = format!(
            "SELECT band_index, band_name, country, genre, status FROM data_{exp} ORDER BY band_index ASC"
        );

        let rows = client.query(&query_string, &[])?;

        println!("Number of rows: {:?}", rows.len());

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

        let pir_server = SimplePIRServer::setup(Band::bands_to_matrix(&bands));

        let target_band_idx = 979;

        let original_band = &bands[target_band_idx];

        let block_start_cell = target_band_idx * Band::SIZEOFRECORD;
        let (secrets, queries) = SimplePIRClient::query_record(
            block_start_cell,
            Band::SIZEOFRECORD,
            pir_server.square_n(),
        );
        let answers = pir_server.answer(&queries);

        let recovered_record =
            SimplePIRClient::recover_record(&secrets, pir_server.hint(), &answers);
        let desired_band = Band::pack_band_to_zp(original_band);

        println!("The original band: {:?}", desired_band);

        println!("Recovered: {:?}", recovered_record);

        assert_eq!(recovered_record.len(), desired_band.len());

        let recovered_band = Band::unpack_band_from_zp(&recovered_record);
        println!("The final recovered band: {:?}", recovered_band);
        assert_eq!(recovered_band.id, original_band.id);

        Ok(())
    }
}
