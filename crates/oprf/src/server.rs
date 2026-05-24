use crate::ot::TreeOtSender;
use crate::{
    default_public_params, encode_keyword, eval_layer, masked_keyword_from_raw, permute_input,
    xor_into, BatchOprfQuery, BatchOprfResponse, MaskedKeyword, OprfError, OprfLayerResponse,
    OprfPublicParams, OprfQuery, OprfResponse, DEFAULT_X_HAT_LEN,
};
use rand::{CryptoRng, RngCore};

pub(crate) type GKey = [u8; 32];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OprfLayerKey {
    pub row1: Vec<GKey>,
    pub row2: Vec<GKey>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OprfKey {
    pub params: OprfPublicParams,
    pub layers: Vec<OprfLayerKey>,
}

#[derive(Debug, Clone)]
pub struct OprfServer {
    key: OprfKey,
    answered: bool,
}

impl OprfServer {
    pub fn setup(rng: &mut (impl RngCore + CryptoRng)) -> Self {
        Self::new(keygen_with_params(default_public_params(), rng))
    }

    pub(crate) fn new(key: OprfKey) -> Self {
        Self {
            key,
            answered: false,
        }
    }

    pub fn public_params(&self) -> &OprfPublicParams {
        &self.key.params
    }

    pub fn answer(
        &mut self,
        query: &OprfQuery,
        rng: &mut (impl RngCore + CryptoRng),
    ) -> Result<OprfResponse, OprfError> {
        let batch_response = self.answer_batch(
            &BatchOprfQuery {
                queries: vec![query.clone()],
            },
            rng,
        )?;

        batch_response
            .responses
            .into_iter()
            .next()
            .ok_or(OprfError::QueryShapeMismatch)
    }

    pub fn answer_batch(
        &mut self,
        query: &BatchOprfQuery,
        rng: &mut (impl RngCore + CryptoRng),
    ) -> Result<BatchOprfResponse, OprfError> {
        if self.answered {
            return Err(OprfError::AlreadyAnswered);
        }
        if query.queries.len() > self.key.params.max_queries {
            return Err(OprfError::TooManyQueries);
        }
        if self.key.layers.len() != self.key.params.layers {
            return Err(OprfError::QueryShapeMismatch);
        }

        let mut responses = Vec::with_capacity(query.queries.len());
        for keyword_query in &query.queries {
            if keyword_query.layers.len() != self.key.params.layers {
                return Err(OprfError::QueryShapeMismatch);
            }

            let mut layer_responses = Vec::with_capacity(keyword_query.layers.len());
            for (layer_query, layer_key) in keyword_query.layers.iter().zip(&self.key.layers) {
                if layer_key.row1.len() != layer_key.row2.len() {
                    return Err(OprfError::MismatchedKeyRows);
                }
                if layer_query.left.n != layer_key.row1.len()
                    || layer_query.right.n != layer_key.row2.len()
                {
                    return Err(OprfError::QueryShapeMismatch);
                }

                let left_messages = row_messages(&layer_key.row1);
                let right_messages = row_messages(&layer_key.row2);
                let left = TreeOtSender::respond(&left_messages, &layer_query.left, rng)?;
                let right = TreeOtSender::respond(&right_messages, &layer_query.right, rng)?;
                layer_responses.push(OprfLayerResponse { left, right });
            }

            responses.push(OprfResponse {
                layers: layer_responses,
            });
        }

        self.answered = true;
        Ok(BatchOprfResponse { responses })
    }

    pub fn mask_keyword(&self, keyword: &str, payload_len: usize) -> MaskedKeyword {
        let out_len = DEFAULT_X_HAT_LEN + payload_len;
        let base_input = encode_keyword(keyword, self.key.params.m);
        let mut raw = vec![0u8; out_len];

        for (layer_idx, layer_key) in self.key.layers.iter().enumerate() {
            let input = permute_input(base_input, &self.key.params, layer_idx);
            let layer = eval_layer(
                &layer_key.row1[input.x1],
                &layer_key.row2[input.x2],
                input,
                out_len,
            );
            xor_into(&mut raw, &layer);
        }

        masked_keyword_from_raw(raw)
    }
}

fn row_messages(row: &[GKey]) -> Vec<Vec<u8>> {
    row.iter().map(|key| key.to_vec()).collect()
}

#[cfg(test)]
pub(crate) fn keygen(m: usize, rng: &mut (impl RngCore + CryptoRng)) -> OprfKey {
    let mut params = default_public_params();
    params.m = m;
    keygen_with_params(params, rng)
}

pub(crate) fn keygen_with_params(
    params: OprfPublicParams,
    rng: &mut (impl RngCore + CryptoRng),
) -> OprfKey {
    OprfKey {
        layers: (0..params.layers)
            .map(|_| OprfLayerKey {
                row1: sample_row(params.m, rng),
                row2: sample_row(params.m, rng),
            })
            .collect(),
        params,
    }
}

fn sample_row(m: usize, rng: &mut (impl RngCore + CryptoRng)) -> Vec<GKey> {
    (0..m)
        .map(|_| {
            let mut key = [0u8; 32];
            rng.fill_bytes(&mut key);
            key
        })
        .collect()
}
