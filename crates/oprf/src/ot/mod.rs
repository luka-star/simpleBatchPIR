pub(crate) mod endemic_kyber;
#[path = "treeOT.rs"]
pub mod tree_ot;

pub type OtKey = [u8; 32];

pub use tree_ot::{
    TreeOtError, TreeOtReceiver, TreeOtReceiverMessage, TreeOtReceiverOutput, TreeOtSender,
    TreeOtSenderMessage,
};
