pub(crate) mod endemic_kyber;
#[path = "treeOT.rs"]
mod tree_ot;

pub(crate) type OtKey = [u8; 32];

pub(crate) use tree_ot::{
    TreeOtReceiver, TreeOtReceiverMessage, TreeOtSender, TreeOtSenderMessage,
};
