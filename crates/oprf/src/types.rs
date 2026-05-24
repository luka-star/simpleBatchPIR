use crate::ot::{TreeOtError, TreeOtReceiver, TreeOtReceiverMessage, TreeOtSenderMessage};
use crate::PrfInput;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OprfPublicParams {
    pub max_queries: usize,
    pub layers: usize,
    pub m: usize,
    pub permutation_seeds: Vec<[u8; 32]>,
}

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
    TooManyQueries,
}

impl From<TreeOtError> for OprfError {
    fn from(error: TreeOtError) -> Self {
        Self::TreeOt(error)
    }
}

#[derive(Debug, Clone)]
pub struct OprfClientState {
    pub(crate) payload_len: usize,
    pub(crate) layers: Vec<OprfLayerClientState>,
}

#[derive(Debug, Clone)]
pub struct BatchOprfClientState {
    pub(crate) keywords: Vec<OprfClientState>,
}

#[derive(Debug, Clone)]
pub(crate) struct OprfLayerClientState {
    pub(crate) input: PrfInput,
    pub(crate) left_receiver: TreeOtReceiver,
    pub(crate) right_receiver: TreeOtReceiver,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OprfLayerQuery {
    pub left: TreeOtReceiverMessage,
    pub right: TreeOtReceiverMessage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OprfQuery {
    pub layers: Vec<OprfLayerQuery>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchOprfQuery {
    pub queries: Vec<OprfQuery>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OprfLayerResponse {
    pub left: TreeOtSenderMessage,
    pub right: TreeOtSenderMessage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OprfResponse {
    pub layers: Vec<OprfLayerResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BatchOprfResponse {
    pub responses: Vec<OprfResponse>,
}
