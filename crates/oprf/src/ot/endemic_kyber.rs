use super::OtKey;
use pqc_kyber::*;
use rand::CryptoRng;
use rand::RngCore;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KyberEndemicOtReceiverMessage {
    pub r_0: [u8; KYBER_PUBLICKEYBYTES],
    pub r_1: [u8; KYBER_PUBLICKEYBYTES],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KyberEndemicOtSenderMessage {
    pub ct_0: [u8; KYBER_CIPHERTEXTBYTES],
    pub ct_1: [u8; KYBER_CIPHERTEXTBYTES],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KyberEndemicOtSenderOutput {
    pub o_0: OtKey,
    pub o_1: OtKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KyberEndemicOtReceiverOutput {
    pub o_b: OtKey,
}

#[derive(Debug, Clone)]
pub struct KyberEndemicOtReceiver {
    choice: bool,
    sk: [u8; KYBER_SECRETKEYBYTES],
}

#[derive(Debug, Clone, Copy, Default)]
pub struct KyberEndemicOtSender;

impl KyberEndemicOtReceiver {
    pub fn gen_strings(
        choice: bool,
        rng: &mut (impl RngCore + CryptoRng),
    ) -> (Self, KyberEndemicOtReceiverMessage) {
        let mut r_other = [0u8; KYBER_PUBLICKEYBYTES];
        rng.fill_bytes(&mut r_other);
        let kyber_keys = keypair(rng).expect("Kyber key generation failed");

        let r_chosen = xor_public_keys(&kyber_keys.public, &hash_to_public_key_mask(&r_other));
        let (r_0, r_1) = if choice {
            (r_other, r_chosen)
        } else {
            (r_chosen, r_other)
        };

        (
            Self {
                choice,
                sk: kyber_keys.secret,
            },
            KyberEndemicOtReceiverMessage { r_0, r_1 },
        )
    }

    pub fn recover_message(
        self,
        sender_msg: &KyberEndemicOtSenderMessage,
    ) -> KyberEndemicOtReceiverOutput {
        let ciphertext = if self.choice {
            &sender_msg.ct_1
        } else {
            &sender_msg.ct_0
        };

        let o_b = decapsulate(ciphertext, &self.sk).expect("Kyber decapsulation failed");

        KyberEndemicOtReceiverOutput { o_b }
    }
}

impl KyberEndemicOtSender {
    pub fn respond(
        receiver_msg: &KyberEndemicOtReceiverMessage,
        rng: &mut (impl RngCore + CryptoRng),
    ) -> (KyberEndemicOtSenderMessage, KyberEndemicOtSenderOutput) {
        let pk_0 = xor_public_keys(
            &receiver_msg.r_0,
            &hash_to_public_key_mask(&receiver_msg.r_1),
        );
        let pk_1 = xor_public_keys(
            &receiver_msg.r_1,
            &hash_to_public_key_mask(&receiver_msg.r_0),
        );

        let (ct_0, o_0) = encapsulate(&pk_0, rng).expect("Kyber encapsulation failed");
        let (ct_1, o_1) = encapsulate(&pk_1, rng).expect("Kyber encapsulation failed");

        (
            KyberEndemicOtSenderMessage { ct_0, ct_1 },
            KyberEndemicOtSenderOutput { o_0, o_1 },
        )
    }
}

fn hash_to_public_key_mask(input: &[u8; KYBER_PUBLICKEYBYTES]) -> [u8; KYBER_PUBLICKEYBYTES] {
    let mut mask = [0u8; KYBER_PUBLICKEYBYTES];

    blake3::Hasher::new()
        .update(input)
        .finalize_xof()
        .fill(&mut mask);
    mask
}

fn xor_public_keys(
    left: &[u8; KYBER_PUBLICKEYBYTES],
    right: &[u8; KYBER_PUBLICKEYBYTES],
) -> [u8; KYBER_PUBLICKEYBYTES] {
    let mut out = [0u8; KYBER_PUBLICKEYBYTES];

    for (out_byte, (left_byte, right_byte)) in out.iter_mut().zip(left.iter().zip(right)) {
        *out_byte = left_byte ^ right_byte;
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::thread_rng;

    #[test]
    fn kyber_endemic_ot_choice_false_matches_sender_k0() {
        let mut rng = thread_rng();
        let (receiver, receiver_msg) = KyberEndemicOtReceiver::gen_strings(false, &mut rng);
        let (sender_msg, sender_output) = KyberEndemicOtSender::respond(&receiver_msg, &mut rng);
        let receiver_output = receiver.recover_message(&sender_msg);

        assert_eq!(receiver_output.o_b, sender_output.o_0);
        assert_ne!(receiver_output.o_b, sender_output.o_1);
    }

    #[test]
    fn kyber_endemic_ot_choice_true_matches_sender_k1() {
        let mut rng = thread_rng();
        let (receiver, receiver_msg) = KyberEndemicOtReceiver::gen_strings(true, &mut rng);
        let (sender_msg, sender_output) = KyberEndemicOtSender::respond(&receiver_msg, &mut rng);
        let receiver_output = receiver.recover_message(&sender_msg);

        assert_eq!(receiver_output.o_b, sender_output.o_1);
        assert_ne!(receiver_output.o_b, sender_output.o_0);
    }

    #[test]
    fn kyber_endemic_ot_randomized_correctness() {
        let mut rng = thread_rng();

        for i in 0..32 {
            let choice = i % 2 == 1;
            let (receiver, receiver_msg) = KyberEndemicOtReceiver::gen_strings(choice, &mut rng);
            let (sender_msg, sender_output) =
                KyberEndemicOtSender::respond(&receiver_msg, &mut rng);
            let receiver_output = receiver.recover_message(&sender_msg);

            if choice {
                assert_eq!(receiver_output.o_b, sender_output.o_1);
            } else {
                assert_eq!(receiver_output.o_b, sender_output.o_0);
            }
            assert_ne!(sender_output.o_0, sender_output.o_1);
        }
    }
}
