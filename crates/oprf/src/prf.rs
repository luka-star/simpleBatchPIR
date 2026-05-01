use blake3::Hasher;
pub mod keygen;
pub mod oprf;
pub mod ot;

pub use keygen::{keygen, GKey, OprfKey};
pub use oprf::{
    encode_keyword, eval_keyword, MaskedKeyword, MockOprf, DEFAULT_M, DEFAULT_X_HAT_LEN,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrfInput {
    pub x1: usize,
    pub x2: usize,
}

pub fn eval(left_key: &GKey,right_key: &GKey,input: PrfInput,out_len: usize) -> Vec<u8> {
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

fn xor_bytes(left: &[u8], right: &[u8]) -> Vec<u8> {
    left.iter().zip(right).map(|(a, b)| a ^ b).collect()
}
