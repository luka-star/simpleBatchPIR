use oprf::{OprfQuery, OprfResponse, OprfServer};
use rand::thread_rng;
use shared::keyword::{
    build_perfect_hash, collect_keywords, pack_keyword_blocks_into_square_matrix,
    pack_posting_block, pack_prebuilt_blocks, RecordId, SecureKeywordClosure, SecureKeywordIndex,
};
use shared::rings::Zp;
use simplepir::SimplePIRServer;
use std::collections::HashMap;
use std::num::Wrapping;

use super::types::{SecureKeywordAnswer, SecureKeywordQuery};

pub struct SecureKeywordSetup {
    pub oprf: OprfServer,
    pub keyword_index: SecureKeywordIndex,
    pub keyword_closure: SecureKeywordClosure,
    pub pir: SimplePIRServer,
}

pub struct SecureKeywordServer {
    pub setup: SecureKeywordSetup,
}

impl SecureKeywordServer {
    pub fn setup(mapping: &HashMap<String, Vec<RecordId>>) -> Self {
        let mut rng = thread_rng();
        let oprf = OprfServer::setup(&mut rng);
        let keyword_index = build_secure_keyword_index(mapping, &oprf);
        let keyword_closure = keyword_index.closure();
        let pir = SimplePIRServer::setup(keyword_index.matrix.clone());

        Self {
            setup: SecureKeywordSetup {
                oprf,
                keyword_index,
                keyword_closure,
                pir,
            },
        }
    }

    pub fn closure(&self) -> SecureKeywordClosure {
        self.setup.keyword_closure.clone()
    }

    pub fn answer_oprf(&mut self, query: &OprfQuery) -> Result<OprfResponse, oprf::OprfError> {
        let mut rng = thread_rng();
        self.setup.oprf.answer(query, &mut rng)
    }

    pub fn answer(&self, query: &SecureKeywordQuery) -> SecureKeywordAnswer {
        self.setup.pir.answer(query)
    }
}

fn build_secure_keyword_index(
    mapping: &HashMap<String, Vec<RecordId>>,
    oprf: &OprfServer,
) -> SecureKeywordIndex {
    let (masked_mapping, record_size) = build_masked_posting_blocks(mapping, oprf);
    let secure_keywords = collect_keywords(&masked_mapping);
    let perfect_hash = build_perfect_hash(&secure_keywords);
    let blocks = pack_prebuilt_blocks(&masked_mapping, &perfect_hash, record_size * 2);
    let matrix = pack_keyword_blocks_into_square_matrix(&blocks);

    SecureKeywordIndex {
        perfect_hash,
        matrix,
        record_size,
        entry_width_bytes: 2,
    }
}

fn build_masked_posting_blocks(
    mapping: &HashMap<String, Vec<usize>>,
    oprf: &OprfServer,
) -> (HashMap<String, Vec<Zp>>, usize) {
    let max_posting_len = mapping.values().map(|posts| posts.len()).max().unwrap_or(0);
    let record_size = max_posting_len.saturating_add(1);
    let block_cell_count = record_size * 2;
    let mut masked_mapping = HashMap::with_capacity(mapping.len());

    for (keyword, postings) in mapping {
        let token = oprf.mask_keyword(keyword, block_cell_count);
        let block = pack_posting_block(postings, record_size);
        let masked_block: Vec<Zp> = block
            .into_iter()
            .zip(token.p_hat.into_iter())
            .map(|(cell, mask)| Wrapping(cell.0 ^ mask))
            .collect();

        masked_mapping.insert(token.x_hat, masked_block);
    }

    (masked_mapping, record_size)
}
