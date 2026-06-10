mod support;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use keyword_search::{
    PlainKeywordClient, PlainKeywordServer, SecureKeywordClient, SecureKeywordServer,
};
use ndarray::Array2;
use shared::keyword::{KeywordClientContext, PerfectHash};
use shared::models::{construct_keyword_mapping, Band};
use shared::pbc::PBCConfig;
use shared::rings::{Zp, Zq};
use simplepir::types::{SimplePIRDatabase, SimplePIRHint};
use simplepir::{BatchSimplePIRClient, BatchSimplePIRServer, SimplePIRClient, SimplePIRServer};
use std::fs::File;
use std::io::Write;
use std::mem::size_of;
use std::path::Path;
use support::{assert_requested_band_count, make_bands, random_index_list};

criterion_group! {
    name = benches;
    config = Criterion::default()
        .output_directory(Path::new("benchmark-results"))
        .warm_up_time(std::time::Duration::from_secs(2))
        .measurement_time(std::time::Duration::from_secs(8))
        .sample_size(10);
    targets =
        setup_benchmarks,
        export_offline_costs,
}

const DB_SIZES: [usize; 5] = [512, 1024, 2048, 4096, 8192];
const BATCH_QUERY_SIZE: usize = 64;
const BATCH_BUCKET_COUNT: usize = 1500;
const BATCH_HASH_COUNT: usize = 3;
const ONE_GBPS_BYTES_PER_SECOND: f64 = 1_000_000_000.0 / 8.0;
const HUNDRED_MBPS_BYTES_PER_SECOND: f64 = 100_000_000.0 / 8.0;

struct SetupCostRecord {
    protocol: &'static str,
    records: usize,
    server_storage_bytes: usize,
    client_storage_bytes: usize,
    notes: &'static str,
}

struct CommunicationCostRecord {
    protocol: &'static str,
    records: usize,
    query_items: usize,
    client_upload_bytes: usize,
    server_download_bytes: usize,
}

trait MkdirIfMissing {
    fn mkdir_if_missing(&self);
}

impl MkdirIfMissing for Path {
    fn mkdir_if_missing(&self) {
        std::fs::create_dir_all(self).expect("Failed to create benchmark-results directory");
    }
}

fn load_bands(nr_bands: usize) -> Vec<Band> {
    let bands = make_bands(nr_bands).expect("Failed to fetch bands");
    assert_requested_band_count(nr_bands, bands.len());
    bands
}

fn deterministic_keyword(mapping: &std::collections::HashMap<String, Vec<usize>>) -> String {
    let mut keywords: Vec<String> = mapping.keys().cloned().collect();
    keywords.sort_unstable();
    keywords
        .into_iter()
        .next()
        .expect("keyword benchmark requires at least one keyword")
}

fn array2_bytes<T>(array: &Array2<T>) -> usize {
    array.len() * size_of::<T>()
}

fn zq_matrix_bytes(array: &Array2<Zq>) -> usize {
    array2_bytes(array)
}

fn zp_matrix_bytes(array: &Array2<Zp>) -> usize {
    array2_bytes(array)
}

fn lifted_zq_bytes_for_zp_matrix(array: &Array2<Zp>) -> usize {
    array.len() * size_of::<Zq>()
}

fn hint_bytes(hint: &SimplePIRHint) -> usize {
    zq_matrix_bytes(hint)
}

fn serialized_len<T: serde::Serialize>(value: &T) -> usize {
    bincode::serialized_size(value).expect("failed to compute serialized size") as usize
}

fn simple_server_storage(server: &SimplePIRServer) -> usize {
    zp_matrix_bytes(&server.db)
        + lifted_zq_bytes_for_zp_matrix(&server.db)
        + hint_bytes(&server.hint)
}

fn simple_client_storage(server: &SimplePIRServer) -> usize {
    hint_bytes(server.hint())
}

fn batch_config_bytes(config: &PBCConfig) -> usize {
    serialized_len(config)
}

fn position_map_bytes(entries: usize) -> usize {
    entries * (3 * size_of::<usize>())
}

fn batch_server_storage(server: &BatchSimplePIRServer) -> usize {
    let bucket_bytes: usize = server
        .buckets
        .iter()
        .map(|bucket| zp_matrix_bytes(bucket) + lifted_zq_bytes_for_zp_matrix(bucket))
        .sum();
    let hint_bytes: usize = server.hints.iter().map(hint_bytes).sum();
    bucket_bytes + hint_bytes + position_map_bytes(server.position_map.len())
}

fn batch_client_storage(server: &BatchSimplePIRServer, config: &PBCConfig) -> usize {
    let hint_bytes: usize = server.hints.iter().map(hint_bytes).sum();
    hint_bytes + position_map_bytes(server.position_map.len()) + batch_config_bytes(config)
}

fn perfect_hash_bytes(hash: &PerfectHash) -> usize {
    hash.table_size * size_of::<usize>()
}

fn keyword_context_bytes(context: &KeywordClientContext) -> usize {
    perfect_hash_bytes(&context.perfect_hash) + 2 * size_of::<usize>()
}

fn plain_keyword_server_storage(server: &PlainKeywordServer) -> usize {
    let keyword_database_bytes = zp_matrix_bytes(&server.keyword_database.matrix)
        + perfect_hash_bytes(&server.keyword_database.perfect_hash)
        + size_of::<usize>();
    keyword_database_bytes + simple_server_storage(&server.pir)
}

fn plain_keyword_client_storage(server: &PlainKeywordServer) -> usize {
    keyword_context_bytes(&server.client_context()) + simple_client_storage(&server.pir)
}

fn oprf_public_params_bytes(permutation_master_seed_len: usize) -> usize {
    (3 * size_of::<usize>()) + permutation_master_seed_len
}

fn oprf_server_key_bytes(layers: usize, m: usize) -> usize {
    layers * 2 * m * 32
}

fn secure_keyword_server_storage(server: &SecureKeywordServer) -> usize {
    let setup = &server.setup;
    let params = &setup.keyword_client_context.oprf_params;
    let keyword_database_bytes = zp_matrix_bytes(&setup.keyword_database.matrix)
        + perfect_hash_bytes(&setup.keyword_database.perfect_hash)
        + size_of::<usize>();
    keyword_database_bytes
        + simple_server_storage(&setup.pir)
        + perfect_hash_bytes(&setup.keyword_client_context.oprf_input_hash)
        + oprf_server_key_bytes(params.layers, params.m)
}

fn secure_keyword_client_storage(server: &SecureKeywordServer) -> usize {
    let context = server.client_context();
    keyword_context_bytes(&context.keyword_database)
        + perfect_hash_bytes(&context.oprf_input_hash)
        + oprf_public_params_bytes(context.oprf_params.permutation_master_seed.len())
        + simple_client_storage(&server.setup.pir)
}

fn simple_communication(server: &SimplePIRServer) -> CommunicationCostRecord {
    let block_start_cell = 0;
    let (_state, query) =
        SimplePIRClient::query_record(block_start_cell, Band::SIZEOFRECORD, server.square_n());
    let answer = server.answer(&query);

    CommunicationCostRecord {
        protocol: "simplepir",
        records: server.db.len() / Band::SIZEOFRECORD,
        query_items: 1,
        client_upload_bytes: serialized_len(&query),
        server_download_bytes: serialized_len(&answer),
    }
}

fn batch_communication(
    server: &BatchSimplePIRServer,
    config: &PBCConfig,
    nr_bands: usize,
) -> CommunicationCostRecord {
    let mut indices = random_index_list(BATCH_QUERY_SIZE, nr_bands);
    let (query, answer) = loop {
        if let Ok((_states, query, _schedule)) = BatchSimplePIRClient::query(
            &indices,
            &server.position_map,
            server.bucket_size(),
            Band::SIZEOFRECORD,
            config,
        ) {
            let answer = server.answer(&query);
            break (query, answer);
        }
        indices = random_index_list(BATCH_QUERY_SIZE, nr_bands);
    };

    CommunicationCostRecord {
        protocol: "batched_simplepir",
        records: nr_bands,
        query_items: BATCH_QUERY_SIZE,
        client_upload_bytes: serialized_len(&query),
        server_download_bytes: serialized_len(&answer),
    }
}

fn plain_keyword_communication(
    server: &PlainKeywordServer,
    keyword: &str,
    nr_bands: usize,
) -> CommunicationCostRecord {
    let context = server.client_context();
    let (_state, query) = PlainKeywordClient::query(keyword, &context)
        .expect("deterministic keyword must exist in plain keyword index");
    let answer = server.answer(&query);

    CommunicationCostRecord {
        protocol: "semi_private_keyword_search",
        records: nr_bands,
        query_items: 1,
        client_upload_bytes: serialized_len(&query),
        server_download_bytes: serialized_len(&answer),
    }
}

fn secure_keyword_communication(
    server: &SecureKeywordServer,
    keyword: &str,
    nr_bands: usize,
) -> CommunicationCostRecord {
    let context = server.client_context();
    let keywords = [keyword.to_string()];
    let (oprf_state, oprf_query) = SecureKeywordClient::start_oprf(&keywords, &context)
        .expect("deterministic keyword must normalize for secure keyword query");
    let oprf_response = server
        .answer_oprf_session(&oprf_query)
        .expect("secure keyword OPRF should answer");
    let (_state, query) = SecureKeywordClient::query(oprf_state, &context, &oprf_response)
        .into_iter()
        .next()
        .expect("secure keyword OPRF should return one result")
        .expect("deterministic keyword must exist in secure keyword index");
    let answer = server.answer(&query);

    CommunicationCostRecord {
        protocol: "private_keyword_search",
        records: nr_bands,
        query_items: 1,
        client_upload_bytes: serialized_len(&oprf_query) + serialized_len(&query),
        server_download_bytes: serialized_len(&oprf_response) + serialized_len(&answer),
    }
}

fn write_setup_costs(output_path: &Path, records: &[SetupCostRecord]) {
    output_path.parent().unwrap().mkdir_if_missing();
    let mut file = File::create(output_path).expect("Failed to create storage CSV");
    writeln!(
        file,
        "protocol,records,server_storage_bytes,client_storage_bytes,notes"
    )
    .expect("Failed to write storage CSV header");

    for record in records {
        writeln!(
            file,
            "{},{},{},{},{}",
            record.protocol,
            record.records,
            record.server_storage_bytes,
            record.client_storage_bytes,
            record.notes
        )
        .expect("Failed to write storage CSV row");
    }
}

fn transfer_ms(bytes: usize, bytes_per_second: f64) -> f64 {
    (bytes as f64 / bytes_per_second) * 1_000.0
}

fn write_communication_costs(output_path: &Path, records: &[CommunicationCostRecord]) {
    output_path.parent().unwrap().mkdir_if_missing();
    let mut file = File::create(output_path).expect("Failed to create communication CSV");
    writeln!(
        file,
        "protocol,records,query_items,client_upload_bytes,server_download_bytes,upload_100mbps_ms,download_100mbps_ms,upload_1gbps_ms,download_1gbps_ms"
    )
    .expect("Failed to write communication CSV header");

    for record in records {
        writeln!(
            file,
            "{},{},{},{},{},{:.6},{:.6},{:.6},{:.6}",
            record.protocol,
            record.records,
            record.query_items,
            record.client_upload_bytes,
            record.server_download_bytes,
            transfer_ms(record.client_upload_bytes, HUNDRED_MBPS_BYTES_PER_SECOND),
            transfer_ms(record.server_download_bytes, HUNDRED_MBPS_BYTES_PER_SECOND),
            transfer_ms(record.client_upload_bytes, ONE_GBPS_BYTES_PER_SECOND),
            transfer_ms(record.server_download_bytes, ONE_GBPS_BYTES_PER_SECOND)
        )
        .expect("Failed to write communication CSV row");
    }
}

fn setup_benchmarks(c: &mut Criterion) {
    let mut group = c.benchmark_group("offline_setup");
    let batch_config = PBCConfig::fixed_seeds(BATCH_BUCKET_COUNT, BATCH_HASH_COUNT);

    for nr_bands in DB_SIZES {
        let bands = load_bands(nr_bands);

        group.bench_with_input(
            BenchmarkId::new("simplepir", nr_bands),
            &nr_bands,
            |b, &_nr_bands| {
                b.iter(|| {
                    let db: SimplePIRDatabase = Band::bands_to_matrix(black_box(&bands));
                    black_box(SimplePIRServer::setup(db));
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("batched_simplepir", nr_bands),
            &nr_bands,
            |b, &_nr_bands| {
                b.iter(|| {
                    let db: SimplePIRDatabase = Band::bands_to_matrix(black_box(&bands));
                    black_box(BatchSimplePIRServer::setup(
                        black_box(&db),
                        black_box(Band::SIZEOFRECORD),
                        black_box(&batch_config),
                    ));
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("semi_private_keyword_search", nr_bands),
            &nr_bands,
            |b, &_nr_bands| {
                b.iter(|| {
                    let mapping = construct_keyword_mapping(black_box(&bands));
                    black_box(PlainKeywordServer::setup(black_box(&mapping)));
                })
            },
        );

        group.bench_with_input(
            BenchmarkId::new("private_keyword_search", nr_bands),
            &nr_bands,
            |b, &_nr_bands| {
                b.iter(|| {
                    let mapping = construct_keyword_mapping(black_box(&bands));
                    black_box(SecureKeywordServer::setup(black_box(&mapping)));
                })
            },
        );
    }

    group.finish();
}

fn export_offline_costs(_c: &mut Criterion) {
    let mut setup_records = Vec::new();
    let mut communication_records = Vec::new();
    let batch_config = PBCConfig::fixed_seeds(BATCH_BUCKET_COUNT, BATCH_HASH_COUNT);

    for nr_bands in DB_SIZES {
        let bands = load_bands(nr_bands);
        let mapping = construct_keyword_mapping(&bands);
        let keyword = deterministic_keyword(&mapping);

        let simple_server = {
            let db: SimplePIRDatabase = Band::bands_to_matrix(&bands);
            SimplePIRServer::setup(db)
        };
        setup_records.push(SetupCostRecord {
            protocol: "simplepir",
            records: nr_bands,
            server_storage_bytes: simple_server_storage(&simple_server),
            client_storage_bytes: simple_client_storage(&simple_server),
            notes: "server storage includes db,hint,lifted_db",
        });
        communication_records.push(simple_communication(&simple_server));

        let batch_server = {
            let db: SimplePIRDatabase = Band::bands_to_matrix(&bands);
            BatchSimplePIRServer::setup(&db, Band::SIZEOFRECORD, &batch_config)
        };
        setup_records.push(SetupCostRecord {
            protocol: "batched_simplepir",
            records: nr_bands,
            server_storage_bytes: batch_server_storage(&batch_server),
            client_storage_bytes: batch_client_storage(&batch_server, &batch_config),
            notes:
                "w=3,b=1500,server storage includes bucket dbs,hints,lifted_buckets,position_map",
        });
        communication_records.push(batch_communication(&batch_server, &batch_config, nr_bands));

        let plain_server = PlainKeywordServer::setup(&mapping);
        setup_records.push(SetupCostRecord {
            protocol: "semi_private_keyword_search",
            records: nr_bands,
            server_storage_bytes: plain_keyword_server_storage(&plain_server),
            client_storage_bytes: plain_keyword_client_storage(&plain_server),
            notes: "storage includes keyword matrix,MPHF estimate,SimplePIR state",
        });
        communication_records.push(plain_keyword_communication(
            &plain_server,
            &keyword,
            nr_bands,
        ));

        let secure_server = SecureKeywordServer::setup(&mapping);
        setup_records.push(SetupCostRecord {
            protocol: "private_keyword_search",
            records: nr_bands,
            server_storage_bytes: secure_keyword_server_storage(&secure_server),
            client_storage_bytes: secure_keyword_client_storage(&secure_server),
            notes: "storage includes keyword matrix,MPHF estimate,SimplePIR state,OPRF key/public params",
        });
        communication_records.push(secure_keyword_communication(
            &secure_server,
            &keyword,
            nr_bands,
        ));
    }

    write_setup_costs(
        Path::new("benchmark-results/storage_costs.csv"),
        &setup_records,
    );
    write_communication_costs(
        Path::new("benchmark-results/communication_costs.csv"),
        &communication_records,
    );
}

criterion_main!(benches);
