use ndarray::{Array1, Array2};

pub use oprf::{OprfQuery, OprfResponse};
pub use shared::keyword::{RecordFetchRequest, SecureKeywordClosure, SecureKeywordIndex};
use shared::rings::Zq;

use crate::plain::types::PlainKeywordClientState;

#[derive(Debug, Clone)]
pub struct SecureKeywordQueryState {
    pub keyword_state: PlainKeywordClientState,
    pub p_hat: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct SecureKeywordOprfState {
    pub(crate) oprf_state: oprf::OprfClientState,
}

pub type SecureKeywordQuery = Vec<Array1<Zq>>;
pub type SecureKeywordAnswer = Vec<Array1<Zq>>;
pub type SecureKeywordHint = Array2<Zq>;
