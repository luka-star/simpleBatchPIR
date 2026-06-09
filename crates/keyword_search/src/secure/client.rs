use oprf::{MaskedKeyword, PrfInput};
use rand::thread_rng;
use shared::tokenize_text;
use simplepir::SimplePIRClient;

use super::types::{
    OprfQuery, OprfResponse, RecordIdxList, SecureKeywordAnswer, SecureKeywordClientContext,
    SecureKeywordHint, SecureKeywordOprfState, SecureKeywordQuery, SecureKeywordQueryState,
};

#[derive(Debug, Clone, Copy, Default)]
pub struct SecureKeywordClient;

impl SecureKeywordClient {
    pub fn start_oprf(
        keywords: &[String],
        context: &SecureKeywordClientContext,
    ) -> Option<(SecureKeywordOprfState, OprfQuery)> {
        let normalized: Vec<String> = keywords
            .iter()
            .map(|keyword| tokenize_text(keyword).into_iter().next())
            .collect::<Option<_>>()?;
        let m = context.oprf_params.m;
        let inputs: Vec<PrfInput> = normalized
            .iter()
            .map(|keyword| {
                let slot = context.oprf_input_hash.slot(keyword);
                PrfInput {
                    x1: slot / m,
                    x2: slot % m,
                }
            })
            .collect();
        let mut rng = thread_rng();
        let (oprf_state, oprf_query) = oprf::OprfClient::init_oprf(
            &inputs,
            context.keyword_database.keyword_record_cell_count(),
            &context.oprf_params,
            &mut rng,
        );

        Some((SecureKeywordOprfState { oprf_state }, oprf_query))
    }

    pub fn query(
        state: SecureKeywordOprfState,
        context: &SecureKeywordClientContext,
        oprf_response: &OprfResponse,
    ) -> Vec<Option<(SecureKeywordQueryState, SecureKeywordQuery)>> {
        Self::finish_oprf(state, oprf_response)
            .into_iter()
            .map(|token| maskedkeyword_query(token, &context.keyword_database))
            .collect()
    }

    pub fn finish_oprf(
        state: SecureKeywordOprfState,
        oprf_response: &OprfResponse,
    ) -> Vec<MaskedKeyword> {
        oprf::OprfClient::recover(state.oprf_state, oprf_response)
    }

    pub fn recover(
        state: &SecureKeywordQueryState,
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

fn maskedkeyword_query(
    token: MaskedKeyword,
    context: &shared::keyword::KeywordClientContext,
) -> Option<(SecureKeywordQueryState, SecureKeywordQuery)> {
    let slot = context.slot_for(&token.x_hat);
    let keyword_record_cell_count = context.keyword_record_cell_count();
    let keyword_record_start_cell = slot * keyword_record_cell_count;
    let (keyword_state, whole_query) = SimplePIRClient::query_record(
        keyword_record_start_cell,
        keyword_record_cell_count,
        context.square_n,
    );

    Some((
        SecureKeywordQueryState {
            keyword_state,
            p_hat: token.p_hat,
        },
        whole_query,
    ))
}
