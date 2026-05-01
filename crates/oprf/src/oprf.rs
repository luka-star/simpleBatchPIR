use crate::{eval, GKey, OprfKey, PrfInput};

pub const DEFAULT_M: usize = 256;
pub const DEFAULT_X_HAT_LEN: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskedKeyword {
    pub x_hat: String,
    pub p_hat: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct MockOprf {
    key: OprfKey,
}

impl MockOprf {
    pub fn new(key: OprfKey) -> Self {
        Self { key }
    }

    pub fn eval_keyword(&self, keyword: &str, payload_len: usize) -> MaskedKeyword {
        let input = encode_keyword(keyword, DEFAULT_M);
        let (left_key, right_key) = (&self.key.row1[input.x1], &self.key.row2[input.x2]);
        eval_keyword(left_key, right_key, input, payload_len)
    }
}

pub fn encode_keyword(keyword: &str, m: usize) -> PrfInput {
    let bytes = keyword.as_bytes();
    let mid = bytes.len().div_ceil(2);

    let left = &bytes[..mid];
    let right = &bytes[mid..];

    let x1 = bytes_to_index(left, m);
    let x2 = bytes_to_index(right, m);

    PrfInput { x1, x2 }
}

pub fn eval_keyword(left_key: &GKey, right_key: &GKey, input: PrfInput, payload_len: usize) -> MaskedKeyword {
    let raw = eval(left_key, right_key, input, DEFAULT_X_HAT_LEN + payload_len);
    let (x_hat_bytes, p_hat) = raw.split_at(DEFAULT_X_HAT_LEN);

    MaskedKeyword {
        x_hat: hex::encode(&x_hat_bytes),
        p_hat: p_hat.to_vec(),
    }
}

fn bytes_to_index(bytes: &[u8], m: usize) -> usize {
    bytes.iter().fold(0usize, |acc, &byte| {
        acc.wrapping_mul(257).wrapping_add(byte as usize)
    }) % m
}
