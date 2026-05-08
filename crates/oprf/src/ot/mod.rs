pub mod endemic_kyber;
#[path = "mockOT.rs"]
pub mod mock_ot;
#[path = "treeOT.rs"]
pub mod tree_ot;

pub type OtKey = [u8; 32];

pub use endemic_kyber::{
    KyberEndemicOtError, KyberEndemicOtReceiver, KyberEndemicOtReceiverMessage,
    KyberEndemicOtReceiverOutput, KyberEndemicOtSender, KyberEndemicOtSenderMessage,
    KyberEndemicOtSenderOutput,
};
pub use mock_ot::{choose_from_row_mock, choose_pair_mock};
pub use tree_ot::{
    TreeOtError, TreeOtReceiver, TreeOtReceiverMessage, TreeOtReceiverOutput, TreeOtSender,
    TreeOtSenderMessage,
};
