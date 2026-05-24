use oprf::OprfError;
use rand::thread_rng;

use super::types::{
    BatchOprfQuery, BatchOprfResponse, OprfQuery, OprfResponse, RecordFetchRequest,
    SecureKeywordAnswer, SecureKeywordBatchOprfState, SecureKeywordClosure, SecureKeywordHint,
    SecureKeywordOprfState, SecureKeywordQuery, SecureKeywordQueryState,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct SecureKeywordClient;

impl SecureKeywordClient {
    pub fn start_oprf(
        keyword: &str,
        closure: &SecureKeywordClosure,
    ) -> Option<(SecureKeywordOprfState, OprfQuery)> {
        let norm_keyword = crate::plain::normalize_keyword(keyword)?;
        let mut rng = thread_rng();
        let (oprf_state, oprf_query) =
            oprf::OprfClient::init_oprf(&norm_keyword, closure.block_cell_count(), &mut rng)
                .ok()?;

        Some((SecureKeywordOprfState { oprf_state }, oprf_query))
    }

    pub fn start_batch_oprf(
        keywords: &[String],
        closure: &SecureKeywordClosure,
    ) -> Option<(SecureKeywordBatchOprfState, BatchOprfQuery)> {
        let normalized: Vec<String> = keywords
            .iter()
            .map(|keyword| crate::plain::normalize_keyword(keyword))
            .collect::<Option<_>>()?;
        let mut rng = thread_rng();
        let params = oprf::default_public_params();
        let (oprf_state, oprf_query) = oprf::OprfClient::init_batch_oprf(
            &normalized,
            closure.block_cell_count(),
            &params,
            &mut rng,
        )
        .ok()?;

        Some((SecureKeywordBatchOprfState { oprf_state }, oprf_query))
    }

    pub fn finish_query(
        state: SecureKeywordOprfState,
        closure: &SecureKeywordClosure,
        oprf_response: &OprfResponse,
    ) -> Result<Option<(SecureKeywordQueryState, SecureKeywordQuery)>, OprfError> {
        let token = oprf::OprfClient::recover(state.oprf_state, oprf_response)?;
        let Some((keyword_state, whole_query)) =
            crate::plain::secure_keyword_query(&token.x_hat, closure)
        else {
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

    pub fn finish_batch_query(
        state: SecureKeywordBatchOprfState,
        closure: &SecureKeywordClosure,
        oprf_response: &BatchOprfResponse,
    ) -> Result<Vec<Option<(SecureKeywordQueryState, SecureKeywordQuery)>>, OprfError> {
        let tokens = oprf::OprfClient::recover_batch(state.oprf_state, oprf_response)?;
        let mut queries = Vec::with_capacity(tokens.len());

        for token in tokens {
            let Some((keyword_state, whole_query)) =
                crate::plain::secure_keyword_query(&token.x_hat, closure)
            else {
                queries.push(None);
                continue;
            };

            queries.push(Some((
                SecureKeywordQueryState {
                    keyword_state,
                    p_hat: token.p_hat,
                },
                whole_query,
            )));
        }

        Ok(queries)
    }

    pub fn recover(
        state: &SecureKeywordQueryState,
        _closure: &SecureKeywordClosure,
        hint: &SecureKeywordHint,
        answers: &SecureKeywordAnswer,
    ) -> RecordFetchRequest {
        let bytes: Vec<u8> =
            crate::plain::recover_keyword_block_bytes(&state.keyword_state, hint, answers)
                .into_iter()
                .zip(state.p_hat.iter())
                .map(|(byte, mask)| byte ^ mask)
                .collect();

        shared::keyword::decode_posting_block(&bytes)
    }
}
