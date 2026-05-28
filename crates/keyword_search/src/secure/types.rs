pub use oprf::{BatchOprfQuery, BatchOprfResponse, OprfQuery, OprfResponse};
pub use shared::keyword::{RecordIdxList, SecureKeywordClientContext, SecureKeywordDatabase};
use simplepir::types::{
    SimplePIRClientState, SimplePIRHint, SimplePIRRecordAnswer, SimplePIRRecordQuery,
};

#[derive(Debug, Clone)]
pub struct SecureKeywordQueryState {
    pub keyword_state: SimplePIRClientState,
    pub p_hat: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct SecureKeywordOprfState {
    pub(crate) oprf_state: oprf::OprfClientState,
}

#[derive(Debug, Clone)]
pub struct SecureKeywordBatchOprfState {
    pub(crate) oprf_state: oprf::BatchOprfClientState,
}

pub type SecureKeywordQuery = SimplePIRRecordQuery;
pub type SecureKeywordAnswer = SimplePIRRecordAnswer;
pub type SecureKeywordHint = SimplePIRHint;
