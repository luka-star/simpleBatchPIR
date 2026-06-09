mod support;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use keyword_search::{SecureKeywordClient, SecureKeywordServer};
use rand::seq::SliceRandom;
use shared::models::construct_keyword_mapping;
use shared::RecordIdxList;
use std::collections::HashMap;
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
        keyword_oprf,
}

const LOOKUP_DB_SIZE: [usize; 5] = [512, 1024, 2048, 4096, 8192];
const OPRF_M: usize = 256;

struct KeywordOprfFixture {
    mapping: HashMap<String, RecordIdxList>,
    keywords: Vec<String>,
    secure_server: SecureKeywordServer,
}

fn keyword_mapping(nr_bands: usize) -> HashMap<String, RecordIdxList> {
    let bands = make_bands(nr_bands).expect("Failed to fetch bands");
    assert_requested_band_count(nr_bands, bands.len());
    construct_keyword_mapping(&bands)
}

fn assert_keyword_mapping_supported(mapping: &HashMap<String, RecordIdxList>) {
    assert!(
        !mapping.is_empty(),
        "keyword OPRF benchmark requires at least one keyword"
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

fn keyword_oprf_fixture(nr_bands: usize) -> KeywordOprfFixture {
    let mapping = keyword_mapping(nr_bands);
    assert_keyword_mapping_supported(&mapping);
    let keywords = shared::keyword::collect_keywords(&mapping);
    let secure_server = SecureKeywordServer::setup(&mapping);

    assert_eq!(
        secure_server.setup.keyword_client_context.oprf_params.m, OPRF_M,
        "keyword OPRF benchmark assumes fixed OPRF m={OPRF_M}"
    );

    KeywordOprfFixture {
        mapping,
        keywords,
        secure_server,
    }
}

fn random_keyword(keywords: &[String]) -> &str {
    keywords
        .choose(&mut rand::thread_rng())
        .expect("keyword list must be non-empty")
}

fn keyword_oprf(c: &mut Criterion) {
    let mut group = c.benchmark_group("keyword_oprf");

    for nr_bands in LOOKUP_DB_SIZE {
        let fixture = keyword_oprf_fixture(nr_bands);
        let secure_context = fixture.secure_server.client_context();

        group.bench_with_input(
            BenchmarkId::from_parameter(nr_bands),
            &nr_bands,
            |b, &_nr_bands| {
                b.iter(|| {
                    let keyword = black_box(random_keyword(&fixture.keywords));
                    assert!(
                        fixture.mapping.contains_key(keyword),
                        "sampled keyword must exist in mapping"
                    );
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
                    let result = SecureKeywordClient::finish_oprf(
                        black_box(oprf_state),
                        black_box(&secure_context),
                        black_box(&oprf_response),
                    )
                    .into_iter()
                    .next()
                    .expect("secure keyword OPRF should return one result")
                    .expect("sampled keyword must exist in secure keyword index");

                    assert_eq!(
                        result.0.p_hat.len(),
                        secure_context.keyword_database.keyword_record_cell_count()
                    );
                    black_box(result);
                })
            },
        );
    }

    group.finish();
}

criterion_main!(benches);
