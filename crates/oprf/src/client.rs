use crate::ot::{TreeOtReceiver, TreeOtSenderMessage};
use crate::{
    eval_layer, masked_keyword_from_raw, permute_input, xor_into, GKey, MaskedKeyword,
    OprfClientState, OprfKeywordClientState, OprfKeywordQuery, OprfKeywordResponse,
    OprfLayerClientState, OprfLayerQuery, OprfPublicParams, OprfQuery, OprfResponse, PrfInput,
    DEFAULT_X_HAT_LEN,
};
use rand::{CryptoRng, RngCore};

#[derive(Debug, Clone, Copy, Default)]
pub struct OprfClient;

impl OprfClient {
    pub fn init_oprf(
        inputs: &[PrfInput],
        payload_len: usize,
        params: &OprfPublicParams,
        rng: &mut (impl RngCore + CryptoRng),
    ) -> (OprfClientState, OprfQuery) {
        let mut states = Vec::with_capacity(inputs.len());
        let mut queries = Vec::with_capacity(inputs.len());
        for input in inputs {
            let (state, query) = init_keyword_oprf(*input, payload_len, params, rng);
            states.push(state);
            queries.push(query);
        }

        (OprfClientState { keywords: states }, OprfQuery { queries })
    }

    pub fn recover(state: OprfClientState, response: &OprfResponse) -> Vec<MaskedKeyword> {
        state
            .keywords
            .into_iter()
            .zip(&response.responses)
            .map(|(keyword_state, keyword_response)| {
                recover_keyword(keyword_state, keyword_response)
            })
            .collect()
    }
}

fn recover_keyword(state: OprfKeywordClientState, response: &OprfKeywordResponse) -> MaskedKeyword {
    let out_len = DEFAULT_X_HAT_LEN + state.payload_len;
    let mut raw = vec![0u8; out_len];

    for (state_layer, response_layer) in state.layers.into_iter().zip(&response.layers) {
        let left_key = recover_key(state_layer.left_receiver, &response_layer.left);
        let right_key = recover_key(state_layer.right_receiver, &response_layer.right);
        let layer = eval_layer(&left_key, &right_key, state_layer.input, out_len);
        xor_into(&mut raw, &layer);
    }

    masked_keyword_from_raw(raw)
}

fn init_keyword_oprf(
    base_input: PrfInput,
    payload_len: usize,
    params: &OprfPublicParams,
    rng: &mut (impl RngCore + CryptoRng),
) -> (OprfKeywordClientState, OprfKeywordQuery) {
    let mut layers = Vec::with_capacity(params.layers);
    let mut layer_queries = Vec::with_capacity(params.layers);

    for layer_idx in 0..params.layers {
        let input = permute_input(base_input, params, layer_idx);
        let (left_receiver, left) =
            TreeOtReceiver::choose_leaf(input.x1, params.m, std::mem::size_of::<GKey>(), rng);
        let (right_receiver, right) =
            TreeOtReceiver::choose_leaf(input.x2, params.m, std::mem::size_of::<GKey>(), rng);

        layers.push(OprfLayerClientState {
            input,
            left_receiver,
            right_receiver,
        });
        layer_queries.push(OprfLayerQuery { left, right });
    }

    (
        OprfKeywordClientState {
            payload_len,
            layers,
        },
        OprfKeywordQuery {
            layers: layer_queries,
        },
    )
}

fn recover_key(receiver: TreeOtReceiver, sender_msg: &TreeOtSenderMessage) -> GKey {
    let output = receiver.recover_leaf(sender_msg);
    output
        .message
        .try_into()
        .expect("tree OT returned a key with the wrong length")
}
