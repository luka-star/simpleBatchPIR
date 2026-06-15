use oprf::OprfPublicParams;
pub use oprf::{OprfQuery, OprfResponse};
use shared::keyword::KeywordClientContext;
use shared::keyword::PerfectHash;
pub use shared::keyword::{RecordIdxList, SecureKeywordDatabase};
use simplepir::types::{
    SimplePIRClientState, SimplePIRHint, SimplePIRRecordAnswer, SimplePIRRecordQuery,
};

#[derive(Debug, Clone)]
pub struct SecureKeywordClientContext {
    pub keyword_database: KeywordClientContext,
    pub oprf_input_hash: PerfectHash,
    pub oprf_params: OprfPublicParams,
}

#[derive(Debug, Clone)]
pub struct SecureKeywordQueryState {
    pub keyword_state: SimplePIRClientState,
    pub p_hat: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct SecureKeywordOprfState {
    pub(crate) oprf_state: oprf::OprfClientState,
}

pub type SecureKeywordQuery = SimplePIRRecordQuery;
pub type SecureKeywordAnswer = SimplePIRRecordAnswer;
pub type SecureKeywordHint = SimplePIRHint;
