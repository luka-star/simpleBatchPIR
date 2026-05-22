use ndarray::{Array1, Array2};

pub use shared::keyword::{KeywordClosure, KeywordIndex, RecordFetchRequest};
use shared::rings::Zq;
use simplepir::types::SimplePIRClientState;

#[derive(Debug, Clone)]
pub struct PlainKeywordClientState {
    pub pir_state: SimplePIRClientState,
}

pub type PlainKeywordQuery = Vec<Array1<Zq>>;
pub type PlainKeywordAnswer = Vec<Array1<Zq>>;
pub type PlainKeywordHint = Array2<Zq>;
