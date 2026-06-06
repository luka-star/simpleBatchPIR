use super::endemic_kyber::{
    KyberEndemicOtReceiver, KyberEndemicOtReceiverMessage, KyberEndemicOtSender,
    KyberEndemicOtSenderMessage,
};
use super::OtKey;
use rand::CryptoRng;
use rand::RngCore;

const DOMAIN: &[u8] = b"ot-extension";

#[derive(Clone, Debug)]
pub struct TreeOtReceiver {
    choice: usize,
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
    ) -> (Self, TreeOtReceiverMessage) {
        let depth = tree_depth(n);
        let mut base_receivers = Vec::with_capacity(depth);
        let mut base_messages = Vec::with_capacity(depth);
        for level in 0..depth {
            let bit = index_bit(choice, level, depth);
            let (receiver, message) = KyberEndemicOtReceiver::gen_strings(bit, rng);
            base_receivers.push(receiver);
            base_messages.push(message);
        }
        (
            Self {
                choice,
                message_len,
                base_receivers,
            },
            TreeOtReceiverMessage { n, base_messages },
        )
    }

    pub fn recover_leaf(self, sender_msg: &TreeOtSenderMessage) -> TreeOtReceiverOutput {
        let mut keys = Vec::with_capacity(self.base_receivers.len());
        for (receiver, base_msg) in self
            .base_receivers
            .into_iter()
            .zip(&sender_msg.base_messages)
        {
            keys.push(receiver.recover_message(base_msg).o_b);
        }

        let mask = receiver_mask(&keys, self.choice, self.message_len);
        let ciphertext = &sender_msg.ciphertexts[self.choice];

        TreeOtReceiverOutput {
            message: xor_bytes(ciphertext, &mask),
        }
    }
}

impl TreeOtSender {
    pub fn respond(
        messages: &[Vec<u8>],
        receiver_msg: &TreeOtReceiverMessage,
        rng: &mut (impl RngCore + CryptoRng),
    ) -> TreeOtSenderMessage {
        let n = messages.len();
        let message_len = messages[0].len();
        let depth = tree_depth(n);
        let mut sender_keys: Vec<(OtKey, OtKey)> = Vec::with_capacity(depth);
        let mut base_messages = Vec::with_capacity(depth);

        for base_receiver_msg in &receiver_msg.base_messages {
            let (base_sender_msg, sender_output) =
                KyberEndemicOtSender::respond(base_receiver_msg, rng);
            base_messages.push(base_sender_msg);
            sender_keys.push((sender_output.o_0, sender_output.o_1));
        }

        let ciphertexts = messages
            .iter()
            .enumerate()
            .map(|(index, message)| {
                let mask = sender_mask(&sender_keys, index, message_len);
                xor_bytes(message, &mask)
            })
            .collect();

        TreeOtSenderMessage {
            n,
            message_len,
            base_messages,
            ciphertexts,
        }
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

fn sender_mask(keys: &[(OtKey, OtKey)], index: usize, message_len: usize) -> Vec<u8> {
    let depth = keys.len();
    let mut leaf_key = [0u8; 32];

    for (level, (k_0, k_1)) in keys.iter().enumerate() {
        let key = if index_bit(index, level, depth) {
            k_1
        } else {
            k_0
        };
        xor_key_into(&mut leaf_key, key);
    }

    derive_mask(&leaf_key, index, message_len)
}

fn receiver_mask(keys: &[OtKey], index: usize, message_len: usize) -> Vec<u8> {
    let mut combined_key = [0u8; 32];

    for key in keys {
        xor_key_into(&mut combined_key, key);
    }

    derive_mask(&combined_key, index, message_len)
}

fn xor_key_into(combined_key: &mut OtKey, key: &OtKey) {
    for (combined_byte, key_byte) in combined_key.iter_mut().zip(key) {
        *combined_byte ^= key_byte;
    }
}

fn derive_mask(key: &OtKey, index: usize, message_len: usize) -> Vec<u8> {
    let mut mask = vec![0u8; message_len];
    blake3::Hasher::new_keyed(key)
        .update(DOMAIN)
        .update(&(index as u64).to_le_bytes())
        .update(&(message_len as u64).to_le_bytes())
        .finalize_xof()
        .fill(&mut mask);

    mask
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
            TreeOtReceiver::choose_leaf(choice, messages.len(), message_len, &mut rng);
        let sender_msg = TreeOtSender::respond(&messages, &receiver_msg, &mut rng);
        let output = receiver.recover_leaf(&sender_msg);

        assert_eq!(output.message, messages[choice]);
    }

    #[test]
    fn tree_ot_single_message_uses_no_base_ots() {
        let messages = messages(1, 16);
        let mut rng = thread_rng();
        let (receiver, receiver_msg) = TreeOtReceiver::choose_leaf(0, messages.len(), 16, &mut rng);

        assert!(receiver_msg.base_messages.is_empty());

        let sender_msg = TreeOtSender::respond(&messages, &receiver_msg, &mut rng);

        assert!(sender_msg.base_messages.is_empty());

        let output = receiver.recover_leaf(&sender_msg);
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
        let (_receiver, receiver_msg) = TreeOtReceiver::choose_leaf(5637, 5638, 32, &mut rng);

        assert_eq!(receiver_msg.base_messages.len(), 13);
    }
}
