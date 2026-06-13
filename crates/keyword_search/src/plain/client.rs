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
        hint: &SimplePIRHint,
        answers: &SimplePIRRecordAnswer,
    ) -> RecordIdxList {
        recover_keyword_record(state, hint, answers)
    }
}

fn keyword_query(
    keyword: &str,
    context: &KeywordClientContext,
) -> Option<(SimplePIRClientState, SimplePIRRecordQuery)> {
    let normalized = tokenize_text(keyword).into_iter().next()?;
    let slot = context.slot_for(&normalized);
    let keyword_record_cell_count = context.keyword_record_cell_count();
    let keyword_record_start_cell = slot * keyword_record_cell_count;
    let (state, whole_query) = SimplePIRClient::query_record(
        keyword_record_start_cell,
        keyword_record_cell_count,
        context.square_n,
    );

    Some((state, whole_query))
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
