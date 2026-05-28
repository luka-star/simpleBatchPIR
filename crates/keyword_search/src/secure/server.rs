use oprf::{BatchOprfQuery, BatchOprfResponse, OprfQuery, OprfResponse, OprfServer};
use rand::thread_rng;
use shared::keyword::{
    build_perfect_hash, collect_keywords, encode_keyword_record,
    pack_keyword_records_into_square_matrix, pack_prebuilt_keyword_records, KeywordRecord,
    RecordIdxList, SecureKeywordClientContext, SecureKeywordDatabase,
};
use simplepir::SimplePIRServer;
use std::collections::HashMap;
use std::num::Wrapping;

use super::types::{SecureKeywordAnswer, SecureKeywordQuery};

pub struct SecureKeywordSetup {
    pub oprf: OprfServer,
    pub keyword_database: SecureKeywordDatabase,
    pub keyword_client_context: SecureKeywordClientContext,
    pub pir: SimplePIRServer,
}

pub struct SecureKeywordServer {
    pub setup: SecureKeywordSetup,
}

impl SecureKeywordServer {
    pub fn setup(mapping: &HashMap<String, RecordIdxList>) -> Self {
        let mut rng = thread_rng();
        let oprf = OprfServer::setup(&mut rng);
        let keyword_database = build_secure_keyword_database(mapping, &oprf);
        let keyword_client_context = keyword_database.client_context();
        let pir = SimplePIRServer::setup(keyword_database.matrix.clone());

        Self {
            setup: SecureKeywordSetup {
                oprf,
                keyword_database,
                keyword_client_context,
                pir,
            },
        }
    }

    pub fn client_context(&self) -> SecureKeywordClientContext {
        self.setup.keyword_client_context.clone()
    }

    pub fn answer_oprf(&mut self, query: &OprfQuery) -> Result<OprfResponse, oprf::OprfError> {
        let mut rng = thread_rng();
        self.setup.oprf.answer(query, &mut rng)
    }

    pub fn answer_batch_oprf(
        &mut self,
        query: &BatchOprfQuery,
    ) -> Result<BatchOprfResponse, oprf::OprfError> {
        let mut rng = thread_rng();
        self.setup.oprf.answer_batch(query, &mut rng)
    }

    pub fn answer(&self, query: &SecureKeywordQuery) -> SecureKeywordAnswer {
        self.setup.pir.answer(query)
    }
}

fn build_secure_keyword_database(
    mapping: &HashMap<String, RecordIdxList>,
    oprf: &OprfServer,
) -> SecureKeywordDatabase {
    let (masked_mapping, record_size) = build_masked_keyword_records(mapping, oprf);
    let secure_keywords = collect_keywords(&masked_mapping);
    let perfect_hash = build_perfect_hash(&secure_keywords);
    let records = pack_prebuilt_keyword_records(&masked_mapping, &perfect_hash, record_size * 2);
    let matrix = pack_keyword_records_into_square_matrix(&records);

    SecureKeywordDatabase {
        perfect_hash,
        matrix,
        record_size,
    }
}

fn build_masked_keyword_records(
    mapping: &HashMap<String, RecordIdxList>,
    oprf: &OprfServer,
) -> (HashMap<String, KeywordRecord>, usize) {
    let max_record_idx_count = mapping.values().map(|idxs| idxs.len()).max().unwrap_or(0);
    let record_size = max_record_idx_count.saturating_add(1);
    let keyword_record_cell_count = record_size * 2;
    let mut masked_mapping = HashMap::with_capacity(mapping.len());

    for (keyword, record_idxs) in mapping {
        let token = oprf.mask_keyword(keyword, keyword_record_cell_count);
        let record = encode_keyword_record(record_idxs, record_size);
        let masked_record: KeywordRecord = record
            .into_iter()
            .zip(token.p_hat.into_iter())
            .map(|(cell, mask)| Wrapping(cell.0 ^ mask))
            .collect();

        masked_mapping.insert(token.x_hat, masked_record);
    }

    (masked_mapping, record_size)
}
