use shared::keyword::{KeywordClientContext, RecordIdxList};
use shared::tokenize_text;
use simplepir::types::*;
use simplepir::SimplePIRClient;

#[derive(Debug, Clone, Copy, Default)]
pub struct PlainKeywordClient;

impl PlainKeywordClient {
    pub fn query(
        keyword: &str,
        context: &KeywordClientContext,
    ) -> Option<(SimplePIRClientState, SimplePIRRecordQuery)> {
        keyword_query(keyword, context)
    }

    pub fn recover(
        state: &SimplePIRClientState,
        _context: &KeywordClientContext,
        hint: &SimplePIRHint,
        answers: &SimplePIRRecordAnswer,
    ) -> RecordIdxList {
        recover_keyword_record(state, hint, answers)
    }
}

pub(crate) fn normalize_keyword(keyword: &str) -> Option<String> {
    tokenize_text(keyword).into_iter().next()
}

fn keyword_query(
    keyword: &str,
    context: &KeywordClientContext,
) -> Option<(SimplePIRClientState, SimplePIRRecordQuery)> {
    let normalized = normalize_keyword(keyword)?;
    query_context_key(&normalized, context)
}

pub(crate) fn query_context_key(
    key: &str,
    context: &KeywordClientContext,
) -> Option<(SimplePIRClientState, SimplePIRRecordQuery)> {
    let slot = context.slot_for(key);
    Some(query_slot(
        slot,
        context.keyword_record_cell_count(),
        context.square_n,
    ))
}

fn query_slot(
    slot: usize,
    keyword_record_cell_count: usize,
    square_n: usize,
) -> (SimplePIRClientState, SimplePIRRecordQuery) {
    let keyword_record_start_cell = slot * keyword_record_cell_count;
    let (state, whole_query) = SimplePIRClient::query_record(
        keyword_record_start_cell,
        keyword_record_cell_count,
        square_n,
    );

    (state, whole_query)
}

fn recover_keyword_record(
    state: &SimplePIRClientState,
    hint: &SimplePIRHint,
    answers: &SimplePIRRecordAnswer,
) -> RecordIdxList {
    let bytes = recover_keyword_record_bytes(state, hint, answers);
    shared::keyword::decode_keyword_record(&bytes)
}

pub(crate) fn recover_keyword_record_bytes(
    state: &SimplePIRClientState,
    hint: &SimplePIRHint,
    answers: &SimplePIRRecordAnswer,
) -> Vec<u8> {
    SimplePIRClient::recover_record(&state, hint, answers)
        .into_iter()
        .map(|z| z.0)
        .collect()
}