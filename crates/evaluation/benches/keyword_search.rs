mod support;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use keyword_search::{
    PlainKeywordClient, PlainKeywordServer, SecureKeywordClient, SecureKeywordServer,
};
use rand::seq::SliceRandom;
use shared::models::construct_keyword_mapping;
use shared::RecordIdxList;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use support::{assert_requested_band_count, make_bands};

criterion_group! {
    name = benches;
    config = Criterion::default()
        .output_directory(Path::new("benchmark-results"))
        .warm_up_time(std::time::Duration::from_secs(2))
        .measurement_time(std::time::Duration::from_secs(8))
        .sample_size(10);
    targets =
        export_keyword_index_metadata,
        keyword_search,
}

const LOOKUP_DB_SIZE: [usize; 5] = [512, 1024, 2048, 4096, 8192];
const METADATA_DB_SIZE: [usize; 7] = [512, 1024, 2048, 4096, 8192, 16384, 32768];
const OPRF_M: usize = 256;

struct KeywordFixture {
    mapping: HashMap<String, RecordIdxList>,
    keywords: Vec<String>,
    plain_server: PlainKeywordServer,
    secure_server: SecureKeywordServer,
}

struct KeywordIndexMetadata {
    bands: usize,
    distinct_keywords: usize,
    keyword_record_size: usize,
    keyword_matrix_dim: usize,
}

fn keyword_mapping(nr_bands: usize) -> HashMap<String, RecordIdxList> {
    let bands = make_bands(nr_bands).expect("Failed to fetch bands");
    assert_requested_band_count(nr_bands, bands.len());
    construct_keyword_mapping(&bands)
}

fn assert_keyword_mapping_supported(mapping: &HashMap<String, RecordIdxList>) {
    assert!(
        !mapping.is_empty(),
        "keyword benchmark requires at least one keyword"
    );
    assert!(
        mapping.len() <= OPRF_M * OPRF_M,
        "keyword set has {} keywords, exceeding the OPRF input domain {}^2",
        mapping.len(),
        OPRF_M
    );
    assert!(
        mapping
            .values()
            .flatten()
            .all(|idx| *idx <= u16::MAX as usize),
        "keyword records encode indices as u16, but at least one record index exceeds u16::MAX"
    );
}

fn keyword_index_metadata(nr_bands: usize) -> KeywordIndexMetadata {
    let mapping = keyword_mapping(nr_bands);
    assert_keyword_mapping_supported(&mapping);

    let distinct_keywords = mapping.len();
    let max_index_list_len = mapping.values().map(|idxs| idxs.len()).max().unwrap_or(0);
    let keyword_record_size = max_index_list_len.saturating_add(1);
    let keyword_record_cell_count = keyword_record_size * 2;
    let matrix_cells = distinct_keywords * keyword_record_cell_count;
    let keyword_matrix_dim = (matrix_cells as f64).sqrt().ceil() as usize;

    KeywordIndexMetadata {
        bands: nr_bands,
        distinct_keywords,
        keyword_record_size,
        keyword_matrix_dim,
    }
}

fn export_keyword_index_metadata(_c: &mut Criterion) {
    let output_path = Path::new("benchmark-results/keyword_index_metadata.csv");
    output_path
        .parent()
        .expect("metadata output must have a parent")
        .mkdir_if_missing();
    let mut file = File::create(output_path).expect("Failed to create keyword metadata CSV");
    writeln!(
        file,
        "bands,distinct_keywords,max_index_list_len,keyword_record_size,keyword_record_cell_count,keyword_matrix_dim"
    )
    .expect("Failed to write keyword metadata CSV header");

    for nr_bands in METADATA_DB_SIZE {
        let metadata = keyword_index_metadata(nr_bands);
        writeln!(
            file,
            "{},{},{},{}",
            metadata.bands,
            metadata.distinct_keywords,
            metadata.keyword_record_size,
            metadata.keyword_matrix_dim
        )
        .expect("Failed to write keyword metadata CSV row");
    }
}

fn keyword_fixture(nr_bands: usize) -> KeywordFixture {
    let mapping = keyword_mapping(nr_bands);
    assert_keyword_mapping_supported(&mapping);
    let keywords = shared::keyword::collect_keywords(&mapping);

    let plain_server = PlainKeywordServer::setup(&mapping);
    let secure_server = SecureKeywordServer::setup(&mapping);
    assert_eq!(
        secure_server.setup.keyword_client_context.oprf_params.m, OPRF_M,
        "keyword benchmark assumes fixed OPRF m={OPRF_M}"
    );

    KeywordFixture {
        mapping,
        keywords,
        plain_server,
        secure_server,
    }
}

fn random_keyword(keywords: &[String]) -> &str {
    keywords
        .choose(&mut rand::thread_rng())
        .expect("keyword list must be non-empty")
}

trait MkdirIfMissing {
    fn mkdir_if_missing(&self);
}

impl MkdirIfMissing for Path {
    fn mkdir_if_missing(&self) {
        std::fs::create_dir_all(self).expect("Failed to create benchmark-results directory");
    }
}

fn keyword_search(c: &mut Criterion) {
    let mut plain_group = c.benchmark_group("keyword_search_plain");

    for nr_bands in LOOKUP_DB_SIZE {
        let fixture = keyword_fixture(nr_bands);
        let plain_context = fixture.plain_server.client_context();

        plain_group.bench_with_input(
            BenchmarkId::from_parameter(nr_bands),
            &nr_bands,
            |b, &_nr_bands| {
                b.iter(|| {
                    let keyword = black_box(random_keyword(&fixture.keywords));
                    let expected = fixture
                        .mapping
                        .get(keyword)
                        .expect("sampled keyword must exist in mapping");
                    let (state, query) =
                        PlainKeywordClient::query(black_box(keyword), black_box(&plain_context))
                            .expect("sampled keyword must exist in plain keyword index");
                    let answers = fixture.plain_server.answer(black_box(&query));
                    let recovered = PlainKeywordClient::recover(
                        black_box(&state),
                        black_box(fixture.plain_server.pir.hint()),
                        black_box(&answers),
                    );
                    assert_eq!(&recovered, expected);
                    black_box(recovered);
                })
            },
        );
    }

    plain_group.finish();

    let mut secure_group = c.benchmark_group("keyword_search_secure");

    for nr_bands in LOOKUP_DB_SIZE {
        let fixture = keyword_fixture(nr_bands);
        let secure_context = fixture.secure_server.client_context();

        secure_group.bench_with_input(
            BenchmarkId::from_parameter(nr_bands),
            &nr_bands,
            |b, &_nr_bands| {
                b.iter(|| {
                    let keyword = black_box(random_keyword(&fixture.keywords));
                    let expected = fixture
                        .mapping
                        .get(keyword)
                        .expect("sampled keyword must exist in mapping");
                    let keywords = [keyword.to_string()];
                    let (oprf_state, oprf_query) = SecureKeywordClient::start_oprf(
                        black_box(&keywords),
                        black_box(&secure_context),
                    )
                    .expect("sampled keyword must normalize for secure keyword query");
                    let oprf_response = fixture
                        .secure_server
                        .answer_oprf_session(black_box(&oprf_query))
                        .expect("secure keyword OPRF should answer");
                    let (state, query) = SecureKeywordClient::query(
                        black_box(oprf_state),
                        black_box(&secure_context),
                        black_box(&oprf_response),
                    )
                    .into_iter()
                    .next()
                    .expect("secure keyword OPRF should return one result")
                    .expect("sampled keyword must exist in secure keyword index");
                    let answers = fixture.secure_server.answer(black_box(&query));
                    let recovered = SecureKeywordClient::recover(
                        black_box(&state),
                        black_box(fixture.secure_server.setup.pir.hint()),
                        black_box(&answers),
                    );
                    assert_eq!(&recovered, expected);
                    black_box(recovered);
                })
            },
        );
    }

    secure_group.finish();
}

criterion_main!(benches);
