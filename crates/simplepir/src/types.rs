use ndarray::{Array1, Array2};
use std::collections::HashMap;

pub use shared::pbc::PBCConfig;
use shared::rings::{Zp, Zq};

#[derive(Debug, Clone)]
pub struct SimplePIRClientState {
    pub s: Vec<Array1<Zq>>,
    pub start_cell: usize,
    pub cell_count: usize,
    pub square_n: usize,
}

pub struct SimplePIRServerSetup {
    pub hint: Array2<Zq>,
}

pub type SimplePIRDatabase = Array2<Zp>;
pub type SimplePIRRecord = Array1<Zp>;
pub type SimplePIRHint = Array2<Zq>;

pub type SimplePIRQuery = Array1<Zq>;
pub type SimplePIRAnswer = Array1<Zq>;

pub type SimplePIRRecordQuery = Vec<SimplePIRQuery>;
pub type SimplePIRRecordAnswer = Vec<SimplePIRAnswer>;

pub type BatchSimplePIRQuery = Vec<SimplePIRRecordQuery>;
pub type BatchSimplePIRAnswer = Vec<SimplePIRRecordAnswer>;

pub type BatchSimplePIRSchedule = HashMap<usize, usize>;
pub type BatchSimplePIRBucketOracle = HashMap<(usize, usize), usize>;
