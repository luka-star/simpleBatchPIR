use crate::ot::TreeOtSender;
use crate::{
    default_public_params, eval_layer, masked_keyword_from_raw, permute_input, xor_into,
    MaskedKeyword, OprfError, OprfKeywordResponse, OprfLayerResponse, OprfPublicParams, OprfQuery,
    OprfResponse, PrfInput, DEFAULT_X_HAT_LEN,
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
        if self.answered {
            return Err(OprfError::AlreadyAnswered);
        }
        if query.queries.len() > self.key.params.max_queries {
            return Err(OprfError::TooManyQueries);
        }

        let mut responses = Vec::with_capacity(query.queries.len());
        for keyword_query in &query.queries {
            let mut layer_responses = Vec::with_capacity(keyword_query.layers.len());
            for (layer_query, layer_key) in keyword_query.layers.iter().zip(&self.key.layers) {
                let left_messages = row_messages(&layer_key.row1);
                let right_messages = row_messages(&layer_key.row2);
                let left = TreeOtSender::respond(&left_messages, &layer_query.left, rng);
                let right = TreeOtSender::respond(&right_messages, &layer_query.right, rng);
                layer_responses.push(OprfLayerResponse { left, right });
            }

            responses.push(OprfKeywordResponse {
                layers: layer_responses,
            });
        }

        self.answered = true;
        Ok(OprfResponse { responses })
    }

    pub fn mask_input(&self, base_input: PrfInput, payload_len: usize) -> MaskedKeyword {
        let out_len = DEFAULT_X_HAT_LEN + payload_len;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::OprfKeywordQuery;
    use rand::thread_rng;

    #[test]
    fn answer_rejects_more_than_max_queries() {
        let mut rng = thread_rng();
        let mut server = OprfServer::setup(&mut rng);
        let query = OprfQuery {
            queries: vec![
                OprfKeywordQuery { layers: Vec::new() };
                server.public_params().max_queries + 1
            ],
        };

        assert_eq!(
            server.answer(&query, &mut rng),
            Err(OprfError::TooManyQueries)
        );
    }

    #[test]
    fn answer_rejects_reuse_after_success() {
        let mut rng = thread_rng();
        let mut server = OprfServer::setup(&mut rng);
        let query = OprfQuery {
            queries: Vec::new(),
        };

        assert!(server.answer(&query, &mut rng).is_ok());
        assert_eq!(
            server.answer(&query, &mut rng),
            Err(OprfError::AlreadyAnswered)
        );
    }

    #[test]
    fn rejected_query_does_not_consume_server() {
        let mut rng = thread_rng();
        let mut server = OprfServer::setup(&mut rng);
        let too_many = OprfQuery {
            queries: vec![
                OprfKeywordQuery { layers: Vec::new() };
                server.public_params().max_queries + 1
            ],
        };
        let valid = OprfQuery {
            queries: Vec::new(),
        };

        assert_eq!(
            server.answer(&too_many, &mut rng),
            Err(OprfError::TooManyQueries)
        );
        assert!(server.answer(&valid, &mut rng).is_ok());
    }
}
