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

pub fn eval_keyword(
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

fn bytes_to_index(bytes: &[u8], m: usize) -> usize {
    bytes.iter().fold(0usize, |acc, &byte| {
        acc.wrapping_mul(257).wrapping_add(byte as usize)
    }) % m
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keygen;
    use crate::ot::{TreeOtReceiver, TreeOtSender};
    use rand::thread_rng;

    fn select_key_with_tree_ot(
        row: &[GKey],
        index: usize,
        rng: &mut (impl rand::RngCore + rand::CryptoRng),
    ) -> GKey {
        let messages: Vec<Vec<u8>> = row.iter().map(|key| key.to_vec()).collect();
        let (receiver, receiver_msg) =
            TreeOtReceiver::choose_leaf(index, messages.len(), std::mem::size_of::<GKey>(), rng)
                .expect("tree OT receiver start should succeed");
        let sender_msg = TreeOtSender::respond(&messages, &receiver_msg, rng)
            .expect("tree OT sender response should succeed");
        let output = receiver
            .recover_leaf(&sender_msg)
            .expect("tree OT receiver recover_leaf should succeed");
        output
            .message
            .try_into()
            .expect("tree OT should recover one 32-byte OPRF row key")
    }

    fn eval_keyword_with_tree_ot(
        key: &OprfKey,
        keyword: &str,
        payload_len: usize,
        rng: &mut (impl rand::RngCore + rand::CryptoRng),
    ) -> MaskedKeyword {
        let input = encode_keyword(keyword, DEFAULT_M);
        let left_key = select_key_with_tree_ot(&key.row1, input.x1, rng);
        let right_key = select_key_with_tree_ot(&key.row2, input.x2, rng);
        eval_keyword(&left_key, &right_key, input, payload_len)
    }

    #[test]
    fn tree_ot_oprf_matches_mock_oprf_for_single_keyword() {
        let mut rng = thread_rng();
        let key = keygen(DEFAULT_M, &mut rng);
        let mock = MockOprf::new(key.clone());
        let keyword = "metallica";
        let payload_len = 64;

        let mock_token = mock.eval_keyword(keyword, payload_len);
        let tree_ot_token = eval_keyword_with_tree_ot(&key, keyword, payload_len, &mut rng);

        assert_eq!(tree_ot_token, mock_token);
    }

    #[test]
    fn tree_ot_oprf_matches_mock_oprf_for_many_keywords() {
        let mut rng = thread_rng();
        let key = keygen(DEFAULT_M, &mut rng);
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
            let tree_ot_token = eval_keyword_with_tree_ot(&key, keyword, payload_len, &mut rng);

            assert_eq!(tree_ot_token, mock_token, "keyword: {keyword}");
        }
    }
}
