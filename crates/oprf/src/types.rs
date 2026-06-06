use crate::ot::{TreeOtReceiver, TreeOtReceiverMessage, TreeOtSenderMessage};
use crate::PrfInput;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OprfPublicParams {
    pub max_queries: usize,
    pub layers: usize,
    pub m: usize,
    pub permutation_master_seed: [u8; 32],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskedKeyword {
    pub x_hat: String,
    pub p_hat: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OprfError {
    AlreadyAnswered,
    TooManyQueries,
}

#[derive(Debug, Clone)]
pub(crate) struct OprfKeywordClientState {
    pub(crate) payload_len: usize,
    pub(crate) layers: Vec<OprfLayerClientState>,
}

#[derive(Debug, Clone)]
pub struct OprfClientState {
    pub(crate) keywords: Vec<OprfKeywordClientState>,
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
pub(crate) struct OprfKeywordQuery {
    pub layers: Vec<OprfLayerQuery>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OprfQuery {
    pub(crate) queries: Vec<OprfKeywordQuery>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OprfLayerResponse {
    pub left: TreeOtSenderMessage,
    pub right: TreeOtSenderMessage,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OprfKeywordResponse {
    pub layers: Vec<OprfLayerResponse>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OprfResponse {
    pub(crate) responses: Vec<OprfKeywordResponse>,
}
