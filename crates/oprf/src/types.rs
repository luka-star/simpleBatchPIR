use crate::ot::{TreeOtError, TreeOtReceiver, TreeOtReceiverMessage, TreeOtSenderMessage};
use crate::PrfInput;

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
    pub(crate) input: PrfInput,
    pub(crate) payload_len: usize,
    pub(crate) left_receiver: TreeOtReceiver,
    pub(crate) right_receiver: TreeOtReceiver,
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
