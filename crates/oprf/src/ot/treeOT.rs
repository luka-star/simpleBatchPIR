use super::{
    KyberEndemicOtError, KyberEndemicOtReceiver, KyberEndemicOtReceiverMessage,
    KyberEndemicOtSender, KyberEndemicOtSenderMessage, OtKey,
};
use rand::CryptoRng;
use rand::RngCore;

const PAD_DOMAIN: &[u8] = b"naor-pinkas-tree";

#[derive(Debug)]
pub enum TreeOtError {
    EmptyMessages,
    ChoiceOutOfRange,
    InconsistentMessageLengths,
    WrongNumberOfBaseMessages,
    KyberEndemicOt(KyberEndemicOtError),
}

impl From<KyberEndemicOtError> for TreeOtError {
    fn from(error: KyberEndemicOtError) -> Self {
        Self::KyberEndemicOt(error)
    }
}

#[derive(Clone, Debug)]
pub struct TreeOtReceiver {
    choice: usize,
    n: usize,
    message_len: usize,
    base_receivers: Vec<KyberEndemicOtReceiver>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeOtReceiverMessage {
    pub n: usize,
    pub base_messages: Vec<KyberEndemicOtReceiverMessage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeOtSenderMessage {
    pub n: usize,
    pub message_len: usize,
    pub base_messages: Vec<KyberEndemicOtSenderMessage>,
    pub ciphertexts: Vec<Vec<u8>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeOtReceiverOutput {
    pub message: Vec<u8>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TreeOtSender;

impl TreeOtReceiver {
    pub fn choose_leaf(
        choice: usize,
        n: usize,
        message_len: usize,
        rng: &mut (impl RngCore + CryptoRng),
    ) -> Result<(Self, TreeOtReceiverMessage), TreeOtError> {
        if n == 0 {
            return Err(TreeOtError::EmptyMessages);
        }
        if choice >= n {
            return Err(TreeOtError::ChoiceOutOfRange);
        }

        let depth = tree_depth(n);
        let mut base_receivers = Vec::with_capacity(depth);
        let mut base_messages = Vec::with_capacity(depth);

        for level in 0..depth {
            let bit = index_bit(choice, level, depth);
            let (receiver, message) = KyberEndemicOtReceiver::gen_strings(bit, rng)?;
            base_receivers.push(receiver);
            base_messages.push(message);
        }

        Ok((
            Self {
                choice,
                n,
                message_len,
                base_receivers,
            },
            TreeOtReceiverMessage { n, base_messages },
        ))
    }

    pub fn recover_leaf(
        self,
        sender_msg: &TreeOtSenderMessage,
    ) -> Result<TreeOtReceiverOutput, TreeOtError> {
        assert_eq!(sender_msg.n, self.n, "sender response has wrong n");
        assert_eq!(
            sender_msg.message_len, self.message_len,
            "sender response has wrong message length"
        );
        assert_eq!(
            sender_msg.ciphertexts.len(),
            self.n,
            "sender response has wrong ciphertext count"
        );
        assert_eq!(
            sender_msg.base_messages.len(),
            self.base_receivers.len(),
            "sender response has wrong number of base OT messages"
        );

        let mut keys = Vec::with_capacity(self.base_receivers.len());
        for (receiver, base_msg) in self
            .base_receivers
            .into_iter()
            .zip(&sender_msg.base_messages)
        {
            keys.push(receiver.recover_message(base_msg)?.k_b);
        }

        let mask = receiver_mask(&keys, self.choice, self.n, self.message_len);
        let ciphertext = &sender_msg.ciphertexts[self.choice];
        assert_eq!(
            ciphertext.len(),
            self.message_len,
            "selected ciphertext has wrong message length"
        );

        Ok(TreeOtReceiverOutput {
            message: xor_bytes(ciphertext, &mask),
        })
    }
}

impl TreeOtSender {
    pub fn respond(
        messages: &[Vec<u8>],
        receiver_msg: &TreeOtReceiverMessage,
        rng: &mut (impl RngCore + CryptoRng),
    ) -> Result<TreeOtSenderMessage, TreeOtError> {
        let n = messages.len();
        if messages.is_empty() {
            return Err(TreeOtError::EmptyMessages);
        }
        assert_eq!(
            receiver_msg.n,
            messages.len(),
            "receiver and actual messages do not match"
        );

        let message_len = messages[0].len();
        if messages.iter().any(|message| message.len() != message_len) {
            return Err(TreeOtError::InconsistentMessageLengths);
        }

        let depth = tree_depth(n);
        if receiver_msg.base_messages.len() != depth {
            return Err(TreeOtError::WrongNumberOfBaseMessages);
        }

        let mut sender_keys: Vec<(OtKey, OtKey)> = Vec::with_capacity(depth);
        let mut base_messages = Vec::with_capacity(depth);

        for base_receiver_msg in &receiver_msg.base_messages {
            let (base_sender_msg, sender_output) =
                KyberEndemicOtSender::respond(base_receiver_msg, rng)?;
            base_messages.push(base_sender_msg);
            sender_keys.push((sender_output.k_0, sender_output.k_1));
        }

        let ciphertexts = messages
            .iter()
            .enumerate()
            .map(|(index, message)| {
                let mask = sender_mask(&sender_keys, index, n, message_len);
                xor_bytes(message, &mask)
            })
            .collect();

        Ok(TreeOtSenderMessage {
            n,
            message_len,
            base_messages,
            ciphertexts,
        })
    }
}

fn tree_depth(n: usize) -> usize {
    if n <= 1 {
        0
    } else {
        usize::BITS as usize - (n - 1).leading_zeros() as usize
    }
}

fn index_bit(index: usize, level: usize, depth: usize) -> bool {
    let shift = depth - 1 - level;
    ((index >> shift) & 1) == 1
}

fn sender_mask(keys: &[(OtKey, OtKey)], index: usize, n: usize, message_len: usize) -> Vec<u8> {
    let depth = keys.len();
    let mut mask = vec![0u8; message_len];

    for (level, (k_0, k_1)) in keys.iter().enumerate() {
        let key = if index_bit(index, level, depth) {
            k_1
        } else {
            k_0
        };
        xor_pad_into(&mut mask, key, index, n, level);
    }

    mask
}

fn receiver_mask(keys: &[OtKey], index: usize, n: usize, message_len: usize) -> Vec<u8> {
    let mut mask = vec![0u8; message_len];

    for (level, key) in keys.iter().enumerate() {
        xor_pad_into(&mut mask, key, index, n, level);
    }

    mask
}

fn xor_pad_into(mask: &mut [u8], key: &OtKey, index: usize, n: usize, level: usize) {
    let mut pad = vec![0u8; mask.len()];
    blake3::Hasher::new_keyed(key)
        .update(PAD_DOMAIN)
        .update(&(n as u64).to_le_bytes())
        .update(&(index as u64).to_le_bytes())
        .update(&(level as u64).to_le_bytes())
        .update(&(mask.len() as u64).to_le_bytes())
        .finalize_xof()
        .fill(&mut pad);

    for (mask_byte, pad_byte) in mask.iter_mut().zip(pad) {
        *mask_byte ^= pad_byte;
    }
}

fn xor_bytes(left: &[u8], right: &[u8]) -> Vec<u8> {
    left.iter().zip(right).map(|(a, b)| a ^ b).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::thread_rng;

    fn messages(n: usize, message_len: usize) -> Vec<Vec<u8>> {
        (0..n)
            .map(|index| {
                (0..message_len)
                    .map(|offset| ((index * 31 + offset * 17) % 251) as u8)
                    .collect()
            })
            .collect()
    }

    fn roundtrip(n: usize, choice: usize, message_len: usize) {
        let messages = messages(n, message_len);
        let mut rng = thread_rng();
        let (receiver, receiver_msg) =
            TreeOtReceiver::choose_leaf(choice, messages.len(), message_len, &mut rng).unwrap();
        let sender_msg = TreeOtSender::respond(&messages, &receiver_msg, &mut rng).unwrap();
        let output = receiver.recover_leaf(&sender_msg).unwrap();

        assert_eq!(output.message, messages[choice]);
    }

    #[test]
    fn tree_ot_single_message_uses_no_base_ots() {
        let messages = messages(1, 16);
        let mut rng = thread_rng();
        let (receiver, receiver_msg) =
            TreeOtReceiver::choose_leaf(0, messages.len(), 16, &mut rng).unwrap();

        assert!(receiver_msg.base_messages.is_empty());

        let sender_msg = TreeOtSender::respond(&messages, &receiver_msg, &mut rng).unwrap();

        assert!(sender_msg.base_messages.is_empty());

        let output = receiver.recover_leaf(&sender_msg).unwrap();
        assert_eq!(output.message, messages[0]);
    }

    #[test]
    fn tree_ot_power_of_two_choices_work() {
        for choice in 0..8 {
            roundtrip(8, choice, 32);
        }
    }

    #[test]
    fn tree_ot_non_power_of_two_choices_work() {
        for choice in 0..13 {
            roundtrip(13, choice, 32);
        }
    }

    #[test]
    fn tree_ot_largest_keyword_shape_depth_is_13() {
        let mut rng = thread_rng();
        let (_receiver, receiver_msg) =
            TreeOtReceiver::choose_leaf(5637, 5638, 32, &mut rng).unwrap();

        assert_eq!(receiver_msg.base_messages.len(), 13);
    }

    #[test]
    fn tree_ot_rejects_empty_messages() {
        let mut rng = thread_rng();
        let err = match TreeOtReceiver::choose_leaf(0, 0, 32, &mut rng) {
            Ok(_) => panic!("empty message set should be rejected"),
            Err(err) => err,
        };

        assert!(matches!(err, TreeOtError::EmptyMessages));
    }

    #[test]
    fn tree_ot_rejects_choice_out_of_range() {
        let mut rng = thread_rng();
        let err = match TreeOtReceiver::choose_leaf(4, 4, 32, &mut rng) {
            Ok(_) => panic!("out-of-range choice should be rejected"),
            Err(err) => err,
        };

        assert!(matches!(err, TreeOtError::ChoiceOutOfRange));
    }

    #[test]
    fn tree_ot_rejects_inconsistent_message_lengths() {
        let messages = vec![vec![1u8; 8], vec![2u8; 9]];
        let mut rng = thread_rng();
        let (_receiver, receiver_msg) =
            TreeOtReceiver::choose_leaf(0, messages.len(), 8, &mut rng).unwrap();
        let err = TreeOtSender::respond(&messages, &receiver_msg, &mut rng).unwrap_err();

        assert!(matches!(err, TreeOtError::InconsistentMessageLengths));
    }
}
