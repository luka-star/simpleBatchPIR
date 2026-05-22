use crate::ot::{TreeOtReceiver, TreeOtSenderMessage};
use crate::{encode_keyword, eval_keyword, GKey, OprfClientState, OprfError, OprfQuery};
use rand::{CryptoRng, RngCore};

pub const DEFAULT_M: usize = 256;

#[derive(Debug, Clone, Copy, Default)]
pub struct OprfClient;

impl OprfClient {
    pub fn init_oprf(
        keyword: &str,
        payload_len: usize,
        rng: &mut (impl RngCore + CryptoRng),
    ) -> Result<(OprfClientState, OprfQuery), OprfError> {
        let input = encode_keyword(keyword, DEFAULT_M);
        let (left_receiver, left) =
            TreeOtReceiver::choose_leaf(input.x1, DEFAULT_M, std::mem::size_of::<GKey>(), rng)?;
        let (right_receiver, right) =
            TreeOtReceiver::choose_leaf(input.x2, DEFAULT_M, std::mem::size_of::<GKey>(), rng)?;

        Ok((
            OprfClientState {
                input,
                payload_len,
                left_receiver,
                right_receiver,
            },
            OprfQuery { left, right },
        ))
    }

    pub fn recover(
        state: OprfClientState,
        response: &crate::OprfResponse,
    ) -> Result<crate::MaskedKeyword, OprfError> {
        let left_key = recover_key(state.left_receiver, &response.left)?;
        let right_key = recover_key(state.right_receiver, &response.right)?;

        Ok(eval_keyword(
            &left_key,
            &right_key,
            state.input,
            state.payload_len,
        ))
    }
}

fn recover_key(
    receiver: TreeOtReceiver,
    sender_msg: &TreeOtSenderMessage,
) -> Result<GKey, OprfError> {
    let output = receiver.recover_leaf(sender_msg)?;
    output
        .message
        .try_into()
        .map_err(|_| OprfError::WrongRecoveredKeyLength)
}
