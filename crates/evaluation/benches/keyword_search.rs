mod support;

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use keyword_search::{
    PlainKeywordClient, PlainKeywordServer, SecureKeywordClient, SecureKeywordServer,
};
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
        keyword_search,
}

const DB_SIZE: [usize; 6] = [1024, 2048, 4096, 8192, 16384, 32768];
const OPRF_M: usize = 256;

struct KeywordFixture {
    mapping: HashMap<String, RecordIdxList>,
    keywords: Vec<String>,
    plain_server: PlainKeywordServer,
    secure_server: SecureKeywordServer,
}

fn keyword_fixture(nr_bands: usize) -> KeywordFixture {
    let bands = make_bands(nr_bands).expect("Failed to fetch bands");
    assert_requested_band_count(nr_bands, bands.len());

    let mapping = construct_keyword_mapping(&bands);
    let keywords = shared::keyword::collect_keywords(&mapping);
    assert!(
        !keywords.is_empty(),
        "keyword benchmark requires at least one keyword"
    );
    assert!(
        keywords.len() <= OPRF_M * OPRF_M,
        "keyword set has {} keywords, exceeding the OPRF input domain {}^2",
        keywords.len(),
        OPRF_M
    );
    assert!(
        mapping
            .values()
            .flatten()
            .all(|idx| *idx <= u16::MAX as usize),
        "keyword records encode indices as u16, but at least one record index exceeds u16::MAX"
    );

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

fn keyword_search(c: &mut Criterion) {
    let mut plain_group = c.benchmark_group("keyword_search_plain");

    for nr_bands in DB_SIZE {
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

    for nr_bands in DB_SIZE {
        let mut fixture = keyword_fixture(nr_bands);
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
                        .answer_oprf(black_box(&oprf_query))
                        .expect("secure keyword OPRF should answer");
                    let (state, query) = SecureKeywordClient::finish_oprf(
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
