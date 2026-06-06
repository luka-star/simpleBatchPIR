use blake3::Hasher;

pub mod client;
mod ot;
pub mod server;
pub mod types;

pub use client::OprfClient;
pub use server::OprfServer;
pub use types::{
    MaskedKeyword, OprfClientState, OprfError, OprfPublicParams, OprfQuery, OprfResponse,
};

pub(crate) use types::{
    OprfKeywordClientState, OprfKeywordQuery, OprfKeywordResponse, OprfLayerClientState,
    OprfLayerQuery, OprfLayerResponse,
};

pub(crate) use server::GKey;

pub const FEISTEL_ROUNDS: usize = 8;
pub const DEFAULT_M: usize = 256;
const DEFAULT_X_HAT_LEN: usize = 12;
pub const DEFAULT_T: usize = 8;
pub const DEFAULT_ELL: usize = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrfInput {
    pub x1: usize,
    pub x2: usize,
}

impl Default for OprfPublicParams {
    fn default() -> Self {
        Self {
            max_queries: DEFAULT_T,
            layers: DEFAULT_ELL,
            m: DEFAULT_M,
            permutation_master_seed: *blake3::hash(
                b"non-adaptive-oprf/default-public-permutation-master-seed/v1",
            )
            .as_bytes(),
        }
    }
}

pub fn default_public_params() -> OprfPublicParams {
    OprfPublicParams::default()
}

pub(crate) fn permute_input(
    input: PrfInput,
    params: &OprfPublicParams,
    layer: usize,
) -> PrfInput {
    assert_eq!(
        params.m, 256,
        "current Feistel permutation assumes m = 256"
    );
    assert!(layer < params.layers);
    assert!(input.x1 < 256);
    assert!(input.x2 < 256);

    let seed = derive_permutation_seed(&params.permutation_master_seed, layer);

    let index = pack_input(input);
    let permuted = feistel_permute(index, &seed);

    unpack_input(permuted)
}

fn pack_input(input: PrfInput) -> u16 {
    assert!(input.x1 < 256);
    assert!(input.x2 < 256);

    ((input.x1 as u16) << 8) | input.x2 as u16
}

fn unpack_input(index: u16) -> PrfInput {
    PrfInput {
        x1: (index >> 8) as usize,
        x2: (index & 0xff) as usize,
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
    assert!(input.x1 < 256);
    assert!(input.x2 < 256);

    let mut hasher = Hasher::new_keyed(key);

    hasher.update(b"non-adaptive-oprf/eval-g/v1");
    hasher.update(&(input.x1 as u16).to_le_bytes());
    hasher.update(&(input.x2 as u16).to_le_bytes());

    let mut out = vec![0u8; out_len];
    hasher.finalize_xof().fill(&mut out);
    out
}

fn xor_bytes(left: &[u8], right: &[u8]) -> Vec<u8> {
    left.iter().zip(right).map(|(a, b)| a ^ b).collect()
}

pub(crate) fn xor_into(acc: &mut [u8], value: &[u8]) {
    for (a, b) in acc.iter_mut().zip(value) {
        *a ^= b;
    }
}

pub(crate) fn derive_permutation_seed(
    permutation_master_seed: &[u8; 32],
    layer: usize,
) -> [u8; 32] {
    let mut hasher = Hasher::new_keyed(permutation_master_seed);

    hasher.update(b"non-adaptive-oprf/permutation-seed/v1");
    hasher.update(&(layer as u64).to_le_bytes());

    *hasher.finalize().as_bytes()
}

pub(crate) fn feistel_permute(index: u16, seed: &[u8; 32]) -> u16 {
    let mut left = (index >> 8) as u8;
    let mut right = (index & 0xff) as u8;

    for round in 0..FEISTEL_ROUNDS {
        let f = feistel_round_function(seed, round, right);

        let new_left = right;
        let new_right = left ^ f;

        left = new_left;
        right = new_right;
    }

    ((left as u16) << 8) | right as u16
}

fn feistel_round_function(seed: &[u8; 32], round: usize, right: u8) -> u8 {
    let mut hasher = Hasher::new_keyed(seed);

    hasher.update(b"non-adaptive-oprf/feistel-round/v1");
    hasher.update(&(round as u64).to_le_bytes());
    hasher.update(&[right]);

    let hash = hasher.finalize();

    hash.as_bytes()[0]
}
