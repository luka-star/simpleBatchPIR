use crate::ot::{
    TreeOtError, TreeOtReceiver, TreeOtReceiverMessage, TreeOtSender, TreeOtSenderMessage,
};
use crate::{eval, GKey, OprfKey, PrfInput};
use rand::{CryptoRng, RngCore};

pub const DEFAULT_M: usize = 256;
pub const DEFAULT_X_HAT_LEN: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskedKeyword {
    pub x_hat: String,
    pub p_hat: Vec<u8>,
}

#[derive(Debug)]
pub enum OprfError {
    TreeOt(TreeOtError),
    MismatchedKeyRows,
    QueryShapeMismatch,
    WrongRecoveredKeyLength,
    AlreadyAnswered,
}

impl From<TreeOtError> for OprfError {
    fn from(error: TreeOtError) -> Self {
        Self::TreeOt(error)
    }
}

#[derive(Debug, Clone)]
pub struct OprfClientState {
    input: PrfInput,
    payload_len: usize,
    left_receiver: TreeOtReceiver,
    right_receiver: TreeOtReceiver,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OprfQuery {
    pub left: TreeOtReceiverMessage,
    pub right: TreeOtReceiverMessage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OprfResponse {
    pub left: TreeOtSenderMessage,
    pub right: TreeOtSenderMessage,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct OprfClient;

#[derive(Debug, Clone)]
pub struct OprfServer {
    key: OprfKey
}

impl OprfClient {
    pub fn init_oprf(
        keyword: &str,
        payload_len: usize,
        rng: &mut (impl RngCore + CryptoRng),
    ) -> Result<(OprfClientState, OprfQuery), OprfError> {
        let input = encode_keyword(keyword, DEFAULT_M);
        let (left_receiver, left) =
            TreeOtReceiver::choose_leaf(input.x1, DEFAULT_M, std::mem::size_of::<GKey>(), rng)?;
        let (right_receiver, right) =
            TreeOtReceiver::choose_leaf(input.x2, DEFAULT_M, std::mem::size_of::<GKey>(), rng)?;

        Ok((
            OprfClientState {
                input,
                payload_len,
                left_receiver,
                right_receiver,
            },
            OprfQuery { left, right },
        ))
    }

    pub fn recover(
        state: OprfClientState,
        response: &OprfResponse,
    ) -> Result<MaskedKeyword, OprfError> {
        let left_key = recover_key(state.left_receiver, &response.left)?;
        let right_key = recover_key(state.right_receiver, &response.right)?;

        Ok(eval_keyword(
            &left_key,
            &right_key,
            state.input,
            state.payload_len,
        ))
    }
}

impl OprfServer {
    pub fn new(key: OprfKey) -> Self {
        Self {
            key
        }
    }

    pub fn answer(
        &mut self,
        query: &OprfQuery,
        rng: &mut (impl RngCore + CryptoRng),
    ) -> Result<OprfResponse, OprfError> {
        if self.key.row1.len() != self.key.row2.len() {
            return Err(OprfError::MismatchedKeyRows);
        }
        if query.left.n != self.key.row1.len() || query.right.n != self.key.row2.len() {
            return Err(OprfError::QueryShapeMismatch);
        }

        let left_messages = row_messages(&self.key.row1);
        let right_messages = row_messages(&self.key.row2);
        let left = TreeOtSender::respond(&left_messages, &query.left, rng)?;
        let right = TreeOtSender::respond(&right_messages, &query.right, rng)?;

        Ok(OprfResponse { left, right })
    }
}

fn row_messages(row: &[GKey]) -> Vec<Vec<u8>> {
    row.iter().map(|key| key.to_vec()).collect()
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
        let key = keygen(DEFAULT_M, &mut rng);
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
            let tree_ot_token = eval_keyword_with_protocol(&key, keyword, payload_len, &mut rng);

            assert_eq!(tree_ot_token, mock_token, "keyword: {keyword}");
        }
    }

    #[test]
    fn oprf_server_rejects_wrong_query_shape() {
        let mut rng = thread_rng();
        let key = keygen(DEFAULT_M, &mut rng);
        let (_state, mut query) = OprfClient::init_oprf("metallica", 64, &mut rng).unwrap();
        query.left.n += 1;
        let mut server = OprfServer::new(key);

        let err = server.answer(&query, &mut rng).unwrap_err();

        assert!(matches!(err, OprfError::QueryShapeMismatch));
    }

    #[test]
    fn oprf_server_is_one_time() {
        let mut rng = thread_rng();
        let key = keygen(DEFAULT_M, &mut rng);
        let mut server = OprfServer::new(key);
        let (_state, query) = OprfClient::init_oprf("metallica", 64, &mut rng).unwrap();

        server.answer(&query, &mut rng).unwrap();
        let err = server.answer(&query, &mut rng).unwrap_err();

        assert!(matches!(err, OprfError::AlreadyAnswered));
    }
}
