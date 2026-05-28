use oprf::{MaskedKeyword, OprfError};
use rand::thread_rng;

use super::types::{
    BatchOprfQuery, BatchOprfResponse, OprfQuery, OprfResponse, RecordIdxList, SecureKeywordAnswer,
    SecureKeywordBatchOprfState, SecureKeywordClientContext, SecureKeywordHint,
    SecureKeywordOprfState, SecureKeywordQuery, SecureKeywordQueryState,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct SecureKeywordClient;

impl SecureKeywordClient {
    pub fn start_oprf(
        keyword: &str,
        context: &SecureKeywordClientContext,
    ) -> Option<(SecureKeywordOprfState, OprfQuery)> {
        let norm_keyword = crate::plain::normalize_keyword(keyword)?;
        let mut rng = thread_rng();
        let (oprf_state, oprf_query) = oprf::OprfClient::init_oprf(
            &norm_keyword,
            context.keyword_record_cell_count(),
            &mut rng,
        )
        .ok()?;

        Some((SecureKeywordOprfState { oprf_state }, oprf_query))
    }

    pub fn start_batch_oprf(
        keywords: &[String],
        context: &SecureKeywordClientContext,
    ) -> Option<(SecureKeywordBatchOprfState, BatchOprfQuery)> {
        let normalized: Vec<String> = keywords
            .iter()
            .map(|keyword| crate::plain::normalize_keyword(keyword))
            .collect::<Option<_>>()?;
        let mut rng = thread_rng();
        let params = oprf::default_public_params();
        let (oprf_state, oprf_query) = oprf::OprfClient::init_batch_oprf(
            &normalized,
            context.keyword_record_cell_count(),
            &params,
            &mut rng,
        )
        .ok()?;

        Some((SecureKeywordBatchOprfState { oprf_state }, oprf_query))
    }

    pub fn finish_query(
        state: SecureKeywordOprfState,
        context: &SecureKeywordClientContext,
        oprf_response: &OprfResponse,
    ) -> Result<Option<(SecureKeywordQueryState, SecureKeywordQuery)>, OprfError> {
        let token = oprf::OprfClient::recover(state.oprf_state, oprf_response)?;
        Ok(query_from_token(token, context))
    }

    pub fn finish_batch_query(
        state: SecureKeywordBatchOprfState,
        context: &SecureKeywordClientContext,
        oprf_response: &BatchOprfResponse,
    ) -> Result<Vec<Option<(SecureKeywordQueryState, SecureKeywordQuery)>>, OprfError> {
        let tokens = oprf::OprfClient::recover_batch(state.oprf_state, oprf_response)?;
        Ok(tokens
            .into_iter()
            .map(|token| query_from_token(token, context))
            .collect())
    }

    pub fn recover(
        state: &SecureKeywordQueryState,
        _context: &SecureKeywordClientContext,
        hint: &SecureKeywordHint,
        answers: &SecureKeywordAnswer,
    ) -> RecordIdxList {
        let bytes: Vec<u8> =
            crate::plain::recover_keyword_record_bytes(&state.keyword_state, hint, answers)
                .into_iter()
                .zip(state.p_hat.iter())
                .map(|(byte, mask)| byte ^ mask)
                .collect();

        shared::keyword::decode_keyword_record(&bytes)
    }
}

fn query_from_token(
    token: MaskedKeyword,
    context: &SecureKeywordClientContext,
) -> Option<(SecureKeywordQueryState, SecureKeywordQuery)> {
    let (keyword_state, whole_query) = crate::plain::query_context_key(&token.x_hat, context)?;
    Some((
        SecureKeywordQueryState {
            keyword_state,
            p_hat: token.p_hat,
        },
        whole_query,
    ))
}
