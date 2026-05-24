use crate::ot::{TreeOtReceiver, TreeOtSenderMessage};
use crate::{
    default_public_params, encode_keyword, eval_layer, masked_keyword_from_raw, permute_input,
    xor_into, BatchOprfClientState, BatchOprfQuery, BatchOprfResponse, GKey, MaskedKeyword,
    OprfClientState, OprfError, OprfLayerClientState, OprfLayerQuery, OprfPublicParams, OprfQuery,
    OprfResponse, DEFAULT_X_HAT_LEN,
};
use rand::{CryptoRng, RngCore};

pub const DEFAULT_M: usize = 256;

#[derive(Debug, Clone, Copy, Default)]
pub struct OprfClient;

impl OprfClient {
    pub fn init_oprf(
        keyword: &str,
        payload_len: usize,
        rng: &mut (impl RngCore + CryptoRng),
    ) -> Result<(OprfClientState, OprfQuery), OprfError> {
        let params = default_public_params();
        init_keyword_oprf(keyword, payload_len, &params, rng)
    }

    pub fn init_batch_oprf(
        keywords: &[String],
        payload_len: usize,
        params: &OprfPublicParams,
        rng: &mut (impl RngCore + CryptoRng),
    ) -> Result<(BatchOprfClientState, BatchOprfQuery), OprfError> {
        if keywords.len() > params.max_queries {
            return Err(OprfError::TooManyQueries);
        }

        let mut states = Vec::with_capacity(keywords.len());
        let mut queries = Vec::with_capacity(keywords.len());
        for keyword in keywords {
            let (state, query) = init_keyword_oprf(keyword, payload_len, params, rng)?;
            states.push(state);
            queries.push(query);
        }

        Ok((
            BatchOprfClientState { keywords: states },
            BatchOprfQuery { queries },
        ))
    }

    pub fn recover(
        state: OprfClientState,
        response: &OprfResponse,
    ) -> Result<MaskedKeyword, OprfError> {
        if state.layers.len() != response.layers.len() {
            return Err(OprfError::QueryShapeMismatch);
        }

        let out_len = DEFAULT_X_HAT_LEN + state.payload_len;
        let mut raw = vec![0u8; out_len];

        for (state_layer, response_layer) in state.layers.into_iter().zip(&response.layers) {
            let left_key = recover_key(state_layer.left_receiver, &response_layer.left)?;
            let right_key = recover_key(state_layer.right_receiver, &response_layer.right)?;
            let layer = eval_layer(&left_key, &right_key, state_layer.input, out_len);
            xor_into(&mut raw, &layer);
        }

        Ok(masked_keyword_from_raw(raw))
    }

    pub fn recover_batch(
        state: BatchOprfClientState,
        response: &BatchOprfResponse,
    ) -> Result<Vec<MaskedKeyword>, OprfError> {
        if state.keywords.len() != response.responses.len() {
            return Err(OprfError::QueryShapeMismatch);
        }

        state
            .keywords
            .into_iter()
            .zip(&response.responses)
            .map(|(keyword_state, keyword_response)| Self::recover(keyword_state, keyword_response))
            .collect()
    }
}

fn init_keyword_oprf(
    keyword: &str,
    payload_len: usize,
    params: &OprfPublicParams,
    rng: &mut (impl RngCore + CryptoRng),
) -> Result<(OprfClientState, OprfQuery), OprfError> {
    if params.layers != params.permutation_seeds.len() {
        return Err(OprfError::QueryShapeMismatch);
    }

    let base_input = encode_keyword(keyword, params.m);
    let mut layers = Vec::with_capacity(params.layers);
    let mut layer_queries = Vec::with_capacity(params.layers);

    for layer_idx in 0..params.layers {
        let input = permute_input(base_input, params, layer_idx);
        let (left_receiver, left) =
            TreeOtReceiver::choose_leaf(input.x1, params.m, std::mem::size_of::<GKey>(), rng)?;
        let (right_receiver, right) =
            TreeOtReceiver::choose_leaf(input.x2, params.m, std::mem::size_of::<GKey>(), rng)?;

        layers.push(OprfLayerClientState {
            input,
            left_receiver,
            right_receiver,
        });
        layer_queries.push(OprfLayerQuery { left, right });
    }

    Ok((
        OprfClientState {
            payload_len,
            layers,
        },
        OprfQuery {
            layers: layer_queries,
        },
    ))
}

fn recover_key(
    receiver: TreeOtReceiver,
    sender_msg: &TreeOtSenderMessage,
) -> Result<GKey, OprfError> {
    let output = receiver.recover_leaf(sender_msg)?;
    output
        .message
        .try_into()
        .map_err(|_| OprfError::WrongRecoveredKeyLength)
}
