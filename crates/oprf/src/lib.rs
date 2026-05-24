use blake3::Hasher;

pub mod client;
mod ot;
pub mod server;
pub mod types;

pub use client::OprfClient;
pub use server::OprfServer;
pub use types::{
    BatchOprfClientState, BatchOprfQuery, BatchOprfResponse, MaskedKeyword, OprfClientState,
    OprfError, OprfPublicParams, OprfQuery, OprfResponse,
};
pub(crate) use types::{OprfLayerClientState, OprfLayerQuery, OprfLayerResponse};

pub(crate) use server::GKey;

const DEFAULT_X_HAT_LEN: usize = 16;
pub const DEFAULT_T: usize = 16;
pub const DEFAULT_ELL: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PrfInput {
    pub x1: usize,
    pub x2: usize,
}

impl Default for OprfPublicParams {
    fn default() -> Self {
        Self {
            max_queries: DEFAULT_T,
            layers: DEFAULT_ELL,
            m: client::DEFAULT_M,
            permutation_seeds: (0..DEFAULT_ELL).map(permutation_seed).collect(),
        }
    }
}

pub fn default_public_params() -> OprfPublicParams {
    OprfPublicParams::default()
}

pub(crate) fn encode_keyword(keyword: &str, m: usize) -> PrfInput {
    let bytes = keyword.as_bytes();
    let mid = bytes.len().div_ceil(2);

    let left = &bytes[..mid];
    let right = &bytes[mid..];

    let x1 = bytes_to_index(left, m);
    let x2 = bytes_to_index(right, m);

    PrfInput { x1, x2 }
}

pub(crate) fn permute_input(input: PrfInput, params: &OprfPublicParams, layer: usize) -> PrfInput {
    assert_eq!(
        params.m, 256,
        "the current compact permutation assumes M = 256"
    );
    let index = ((input.x1 as u16) << 8) | input.x2 as u16;
    let permuted = feistel_permute(index, &params.permutation_seeds[layer]);
    PrfInput {
        x1: (permuted >> 8) as usize,
        x2: (permuted & 0xff) as usize,
    }
}

pub(crate) fn masked_keyword_from_raw(raw: Vec<u8>) -> MaskedKeyword {
    let (x_hat_bytes, p_hat) = raw.split_at(DEFAULT_X_HAT_LEN);

    MaskedKeyword {
        x_hat: hex::encode(x_hat_bytes),
        p_hat: p_hat.to_vec(),
    }
}

fn eval(left_key: &GKey, right_key: &GKey, input: PrfInput, out_len: usize) -> Vec<u8> {
    let left = eval_g(left_key, input, out_len);
    let right = eval_g(right_key, input, out_len);

    xor_bytes(&left, &right)
}

pub(crate) fn eval_layer(
    left_key: &GKey,
    right_key: &GKey,
    input: PrfInput,
    out_len: usize,
) -> Vec<u8> {
    eval(left_key, right_key, input, out_len)
}

fn eval_g(key: &GKey, input: PrfInput, out_len: usize) -> Vec<u8> {
    let mut hasher = Hasher::new_keyed(key);
    hasher.update(&(input.x1 as u64).to_le_bytes());
    hasher.update(&(input.x2 as u64).to_le_bytes());

    let mut out = vec![0u8; out_len];
    hasher.finalize_xof().fill(&mut out);
    out
}

fn bytes_to_index(bytes: &[u8], m: usize) -> usize {
    bytes.iter().fold(0usize, |acc, &byte| {
        acc.wrapping_mul(257).wrapping_add(byte as usize)
    }) % m
}

fn xor_bytes(left: &[u8], right: &[u8]) -> Vec<u8> {
    left.iter().zip(right).map(|(a, b)| a ^ b).collect()
}

pub(crate) fn xor_into(acc: &mut [u8], value: &[u8]) {
    for (a, b) in acc.iter_mut().zip(value) {
        *a ^= b;
    }
}

fn permutation_seed(layer: usize) -> [u8; 32] {
    *blake3::hash(format!("simpleBatchPIR/non-adaptive-oprf/sigma/{layer}").as_bytes()).as_bytes()
}

fn feistel_permute(index: u16, seed: &[u8; 32]) -> u16 {
    let mut left = (index >> 8) as u8;
    let mut right = (index & 0xff) as u8;

    for round in 0..4u8 {
        let f = feistel_round(seed, round, right);
        let next_left = right;
        let next_right = left ^ f;
        left = next_left;
        right = next_right;
    }

    ((left as u16) << 8) | right as u16
}

fn feistel_round(seed: &[u8; 32], round: u8, half: u8) -> u8 {
    let mut hasher = Hasher::new_keyed(seed);
    hasher.update(&[round, half]);
    hasher.finalize().as_bytes()[0]
}

#[cfg(test)]
#[derive(Debug, Clone)]
struct MockOprf {
    key: server::OprfKey,
}

#[cfg(test)]
impl MockOprf {
    pub fn new(key: server::OprfKey) -> Self {
        Self { key }
    }

    pub fn eval_keyword(&self, keyword: &str, payload_len: usize) -> MaskedKeyword {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::{keygen, OprfKey};
    use rand::thread_rng;

    fn eval_keyword_with_protocol(
        key: &OprfKey,
        keyword: &str,
        payload_len: usize,
        rng: &mut (impl rand::RngCore + rand::CryptoRng),
    ) -> MaskedKeyword {
        let (state, query) = OprfClient::init_oprf(keyword, payload_len, rng)
            .expect("OPRF client should initialize query");
        let mut server = OprfServer::new(key.clone());
        let response = server
            .answer(&query, rng)
            .expect("OPRF server should answer query");
        OprfClient::recover(state, &response).expect("OPRF client should finalize response")
    }

    #[test]
    fn tree_ot_oprf_matches_mock_oprf_for_single_keyword() {
        let mut rng = thread_rng();
        let key = keygen(client::DEFAULT_M, &mut rng);
        let mock = MockOprf::new(key.clone());
        let keyword = "metallica";
        let payload_len = 64;

        let mock_token = mock.eval_keyword(keyword, payload_len);
        let tree_ot_token = eval_keyword_with_protocol(&key, keyword, payload_len, &mut rng);

        assert_eq!(tree_ot_token, mock_token);
    }

    #[test]
    fn tree_ot_oprf_matches_mock_oprf_for_many_keywords() {
        let mut rng = thread_rng();
        let key = keygen(client::DEFAULT_M, &mut rng);
        let mock = MockOprf::new(key.clone());
        let payload_len = 113;
        let keywords = [
            "heavy",
            "black metal",
            "progressive",
            "Iron Maiden",
            "Copenhagen",
            "1990",
            "doom",
            "melodic death",
            "bay area thrash",
            "opeth",
        ];

        for keyword in keywords {
            let mock_token = mock.eval_keyword(keyword, payload_len);
            let tree_ot_token = eval_keyword_with_protocol(&key, keyword, payload_len, &mut rng);

            assert_eq!(tree_ot_token, mock_token, "keyword: {keyword}");
        }
    }

    #[test]
    fn batch_oprf_matches_mock_oprf_for_many_keywords() {
        let mut rng = thread_rng();
        let key = keygen(client::DEFAULT_M, &mut rng);
        let params = key.params.clone();
        let mock = MockOprf::new(key.clone());
        let payload_len = 64;
        let keywords = vec![
            "heavy".to_string(),
            "black metal".to_string(),
            "progressive".to_string(),
            "Iron Maiden".to_string(),
        ];

        let (state, query) = OprfClient::init_batch_oprf(&keywords, payload_len, &params, &mut rng)
            .expect("batch OPRF client should initialize query");
        let mut server = OprfServer::new(key);
        let response = server
            .answer_batch(&query, &mut rng)
            .expect("OPRF server should answer batch query");
        let tokens =
            OprfClient::recover_batch(state, &response).expect("client should recover tokens");

        for (keyword, token) in keywords.iter().zip(tokens) {
            assert_eq!(token, mock.eval_keyword(keyword, payload_len));
        }
    }

    #[test]
    fn batch_oprf_rejects_too_many_queries() {
        let mut rng = thread_rng();
        let mut params = default_public_params();
        params.max_queries = 2;
        let keywords = vec![
            "heavy".to_string(),
            "doom".to_string(),
            "thrash".to_string(),
        ];

        let err = OprfClient::init_batch_oprf(&keywords, 64, &params, &mut rng).unwrap_err();

        assert!(matches!(err, OprfError::TooManyQueries));
    }

    #[test]
    fn oprf_server_rejects_wrong_query_shape() {
        let mut rng = thread_rng();
        let key = keygen(client::DEFAULT_M, &mut rng);
        let (_state, mut query) = OprfClient::init_oprf("metallica", 64, &mut rng).unwrap();
        query.layers[0].left.n += 1;
        let mut server = OprfServer::new(key);

        let err = server.answer(&query, &mut rng).unwrap_err();

        assert!(matches!(err, OprfError::QueryShapeMismatch));
    }

    #[test]
    fn oprf_server_is_one_time() {
        let mut rng = thread_rng();
        let key = keygen(client::DEFAULT_M, &mut rng);
        let mut server = OprfServer::new(key);
        let (_state, query) = OprfClient::init_oprf("metallica", 64, &mut rng).unwrap();

        server.answer(&query, &mut rng).unwrap();
        let err = server.answer(&query, &mut rng).unwrap_err();

        assert!(matches!(err, OprfError::AlreadyAnswered));
    }
}
