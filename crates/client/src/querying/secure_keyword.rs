use ndarray::{Array1, Array2};
use oprf::{OprfClient, OprfClientState, OprfError, OprfQuery, OprfResponse};
use rand_08::thread_rng;
use shared::keyword::SecureKeywordClosure;
use shared::rings::Zq;
use shared::RecordFetchRequest;

use super::keyword::{
    decode_record_fetch_request, recover_keyword_block_bytes, secure_keyword_query,
    KeywordQueryState,
};
use crate::querying::normalize_keyword;

#[derive(Debug, Clone)]
pub struct SecureKeywordQueryState {
    pub keyword_state: KeywordQueryState,
    pub p_hat: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct SecureKeywordOprfState {
    oprf_state: OprfClientState,
}

pub fn sec_keyword_query_start(
    keyword: &str,
    closure: &SecureKeywordClosure,
) -> Option<(SecureKeywordOprfState, OprfQuery)> {
    let norm_keyword = normalize_keyword(keyword)?;
    let mut rng = thread_rng();
    let (oprf_state, oprf_query) =
        OprfClient::init_oprf(&norm_keyword, closure.block_cell_count(), &mut rng).ok()?;

    Some((SecureKeywordOprfState { oprf_state }, oprf_query))
}

pub fn sec_keyword_finish_query(
    state: SecureKeywordOprfState,
    closure: &SecureKeywordClosure,
    oprf_response: &OprfResponse,
) -> Result<Option<(SecureKeywordQueryState, Vec<Array1<Zq>>)>, OprfError> {
    let token = OprfClient::recover(state.oprf_state, oprf_response)?;
    let Some((keyword_state, whole_query)) = secure_keyword_query(&token.x_hat, closure) else {
        return Ok(None);
    };

    Ok(Some((
        SecureKeywordQueryState {
            keyword_state,
            p_hat: token.p_hat,
        },
        whole_query,
    )))
}

pub fn sec_keyword_recover(
    state: &SecureKeywordQueryState,
    block_cell_count: usize,
    hint_c: &Array2<Zq>,
    answers: &[Array1<Zq>],
) -> RecordFetchRequest {
    let bytes: Vec<u8> =
        recover_keyword_block_bytes(&state.keyword_state, block_cell_count, hint_c, answers)
            .into_iter()
            .zip(state.p_hat.iter())
            .map(|(byte, mask)| byte ^ mask)
            .collect();

    decode_record_fetch_request(&bytes, state.keyword_state.eof)
}
