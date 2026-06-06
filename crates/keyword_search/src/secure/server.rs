use oprf::{OprfError, OprfQuery, OprfResponse, OprfServer, PrfInput};
use rand::thread_rng;
use shared::keyword::{
    build_perfect_hash, collect_keywords, encode_keyword_record,
    pack_keyword_records_into_square_matrix, pack_prebuilt_keyword_records, KeywordRecord,
    PerfectHash, RecordIdxList, SecureKeywordDatabase,
};
use simplepir::SimplePIRServer;
use std::collections::HashMap;
use std::num::Wrapping;

use super::types::{SecureKeywordAnswer, SecureKeywordClientContext, SecureKeywordQuery};

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
        let keywords = collect_keywords(mapping);
        let oprf_input_hash = build_perfect_hash(&keywords);
        let oprf_params = oprf.public_params().clone();
        assert!(
            oprf_input_hash.table_size <= oprf_params.m * oprf_params.m,
            "keyword set does not fit into the OPRF input domain"
        );
        let keyword_database = build_secure_keyword_database(mapping, &oprf, &oprf_input_hash);
        let keyword_client_context = SecureKeywordClientContext {
            keyword_database: keyword_database.client_context(),
            oprf_input_hash,
            oprf_params,
        };
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

    pub fn answer_oprf(&mut self, query: &OprfQuery) -> Result<OprfResponse, OprfError> {
        let mut rng = thread_rng();
        self.setup.oprf.answer(query, &mut rng)
    }

    pub fn answer(&self, query: &SecureKeywordQuery) -> SecureKeywordAnswer {
        self.setup.pir.answer(query)
    }
}

fn build_secure_keyword_database(
    mapping: &HashMap<String, RecordIdxList>,
    oprf: &OprfServer,
    oprf_input_hash: &PerfectHash,
) -> SecureKeywordDatabase {
    let (masked_mapping, record_size) =
        build_masked_keyword_records(mapping, oprf, oprf_input_hash);
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
    oprf_input_hash: &PerfectHash,
) -> (HashMap<String, KeywordRecord>, usize) {
    let max_record_idx_count = mapping.values().map(|idxs| idxs.len()).max().unwrap_or(0);
    let record_size = max_record_idx_count.saturating_add(1);
    let keyword_record_cell_count = record_size * 2;
    let mut masked_mapping = HashMap::with_capacity(mapping.len());
    let m = oprf.public_params().m;

    for (keyword, record_idxs) in mapping {
        let slot = oprf_input_hash.slot(keyword);
        let input = PrfInput {
            x1: slot / m,
            x2: slot % m,
        };
        let pseudo_pair = oprf.mask_input(input, keyword_record_cell_count);
        let record = encode_keyword_record(record_idxs, record_size);
        let masked_record: KeywordRecord = record
            .into_iter()
            .zip(pseudo_pair.p_hat.into_iter())
            .map(|(cell, mask)| Wrapping(cell.0 ^ mask))
            .collect();

        masked_mapping.insert(pseudo_pair.x_hat, masked_record);
    }

    (masked_mapping, record_size)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn oprf_input_hash_assigns_unique_inputs_to_indexed_keywords() {
        let keywords = vec![
            "black".to_string(),
            "death".to_string(),
            "doom".to_string(),
            "heavy".to_string(),
            "metal".to_string(),
        ];
        let input_hash = build_perfect_hash(&keywords);
        let m = oprf::DEFAULT_M;
        let inputs: HashSet<_> = keywords
            .iter()
            .map(|keyword| {
                let slot = input_hash.slot(keyword);
                (slot / m, slot % m)
            })
            .collect();

        assert_eq!(inputs.len(), keywords.len());
    }
}
