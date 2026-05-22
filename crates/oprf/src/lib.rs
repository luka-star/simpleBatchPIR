use blake3::Hasher;

pub mod client;
mod ot;
pub mod server;
pub mod types;

pub use client::OprfClient;
pub use server::OprfServer;
pub use types::{MaskedKeyword, OprfClientState, OprfError, OprfQuery, OprfResponse};

pub(crate) use server::GKey;

const DEFAULT_X_HAT_LEN: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PrfInput {
    pub x1: usize,
    pub x2: usize,
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

pub(crate) fn eval_keyword(
    left_key: &GKey,
    right_key: &GKey,
    input: PrfInput,
    payload_len: usize,
) -> MaskedKeyword {
    let raw = eval(left_key, right_key, input, DEFAULT_X_HAT_LEN + payload_len);
    let (x_hat_bytes, p_hat) = raw.split_at(DEFAULT_X_HAT_LEN);

    MaskedKeyword {
        x_hat: hex::encode(&x_hat_bytes),
        p_hat: p_hat.to_vec(),
    }
}

fn eval(left_key: &GKey, right_key: &GKey, input: PrfInput, out_len: usize) -> Vec<u8> {
    let left = eval_g(left_key, input, out_len);
    let right = eval_g(right_key, input, out_len);

    xor_bytes(&left, &right)
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
        let input = encode_keyword(keyword, client::DEFAULT_M);
        let (left_key, right_key) = (&self.key.row1[input.x1], &self.key.row2[input.x2]);
        eval_keyword(left_key, right_key, input, payload_len)
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
    fn oprf_server_rejects_wrong_query_shape() {
        let mut rng = thread_rng();
        let key = keygen(client::DEFAULT_M, &mut rng);
        let (_state, mut query) = OprfClient::init_oprf("metallica", 64, &mut rng).unwrap();
        query.left.n += 1;
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
