use crate::client::DEFAULT_M;
use crate::ot::TreeOtSender;
use crate::{encode_keyword, eval_keyword, MaskedKeyword, OprfError, OprfQuery, OprfResponse};
use rand::{CryptoRng, RngCore};

pub(crate) type GKey = [u8; 32];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OprfKey {
    pub row1: Vec<GKey>,
    pub row2: Vec<GKey>,
}

#[derive(Debug, Clone)]
pub struct OprfServer {
    key: OprfKey,
}

impl OprfServer {
    pub fn setup(rng: &mut (impl RngCore + CryptoRng)) -> Self {
        Self::new(keygen(DEFAULT_M, rng))
    }

    pub(crate) fn new(key: OprfKey) -> Self {
        Self { key }
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

    pub fn mask_keyword(&self, keyword: &str, payload_len: usize) -> MaskedKeyword {
        let input = encode_keyword(keyword, DEFAULT_M);
        let (left_key, right_key) = (&self.key.row1[input.x1], &self.key.row2[input.x2]);
        eval_keyword(left_key, right_key, input, payload_len)
    }
}

fn row_messages(row: &[GKey]) -> Vec<Vec<u8>> {
    row.iter().map(|key| key.to_vec()).collect()
}

pub(crate) fn keygen(m: usize, rng: &mut (impl RngCore + CryptoRng)) -> OprfKey {
    OprfKey {
        row1: sample_row(m, rng),
        row2: sample_row(m, rng),
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
