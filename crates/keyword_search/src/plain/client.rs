use shared::keyword::SecureKeywordClosure;
use shared::tokenize_text;
use simplepir::types::{SimplePIRHint, SimplePIRRecordAnswer};
use simplepir::SimplePIRClient;

use super::types::{
    KeywordClosure, PlainKeywordAnswer, PlainKeywordClientState, PlainKeywordQuery,
    RecordFetchRequest,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct PlainKeywordClient;

impl PlainKeywordClient {
    pub fn query(
        keyword: &str,
        closure: &KeywordClosure,
    ) -> Option<(PlainKeywordClientState, PlainKeywordQuery)> {
        keyword_query(keyword, closure)
    }

    pub fn recover(
        state: &PlainKeywordClientState,
        _closure: &KeywordClosure,
        hint: &super::types::PlainKeywordHint,
        answers: &PlainKeywordAnswer,
    ) -> RecordFetchRequest {
        recover_keyword_block(state, hint, answers)
    }
}

pub(crate) fn normalize_keyword(keyword: &str) -> Option<String> {
    tokenize_text(keyword).into_iter().next()
}

fn keyword_to_slot(keyword: &str, closure: &KeywordClosure) -> Option<usize> {
    let normalized = normalize_keyword(keyword)?;
    Some(closure.slot_for(&normalized))
}

fn keyword_query(
    keyword: &str,
    closure: &KeywordClosure,
) -> Option<(PlainKeywordClientState, PlainKeywordQuery)> {
    let slot = keyword_to_slot(keyword, closure)?;
    Some(query_slot(
        slot,
        closure.block_cell_count(),
        closure.square_n,
    ))
}

pub(crate) fn secure_keyword_query(
    keyword: &str,
    closure: &SecureKeywordClosure,
) -> Option<(PlainKeywordClientState, PlainKeywordQuery)> {
    let slot = closure.slot_for(keyword);
    Some(query_slot(
        slot,
        closure.block_cell_count(),
        closure.square_n,
    ))
}

fn query_slot(
    slot: usize,
    block_cell_count: usize,
    square_n: usize,
) -> (PlainKeywordClientState, PlainKeywordQuery) {
    let block_start_cell = slot * block_cell_count;
    let (pir_state, whole_query) =
        SimplePIRClient::query_record(block_start_cell, block_cell_count, square_n);

    let state = PlainKeywordClientState { pir_state };

    (state, whole_query)
}

fn recover_keyword_block(
    state: &PlainKeywordClientState,
    hint: &SimplePIRHint,
    answers: &SimplePIRRecordAnswer,
) -> RecordFetchRequest {
    let bytes = recover_keyword_block_bytes(state, hint, answers);
    shared::keyword::decode_posting_block(&bytes)
}

pub(crate) fn recover_keyword_block_bytes(
    state: &PlainKeywordClientState,
    hint: &SimplePIRHint,
    answers: &SimplePIRRecordAnswer,
) -> Vec<u8> {
    SimplePIRClient::recover_record(&state.pir_state, hint, answers)
        .into_iter()
        .map(|z| z.0)
        .collect()
}
