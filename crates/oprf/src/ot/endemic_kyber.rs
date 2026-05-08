use super::OtKey;
use pqc_kyber::{
    indcpa::{indcpa_dec, indcpa_enc, indcpa_keypair},
    KyberError, KYBER_CIPHERTEXTBYTES, KYBER_PUBLICKEYBYTES, KYBER_SECRETKEYBYTES, KYBER_SYMBYTES,
};
use rand::CryptoRng;
use rand::RngCore;

const HASH_TO_PK_DOMAIN: &[u8] = b"simpleBatchPIR/kyber-endemic-ot/H-to-pk-mask/v1";

#[derive(Debug)]
pub enum KyberEndemicOtError {
    Kyber(KyberError),
}

impl From<KyberError> for KyberEndemicOtError {
    fn from(error: KyberError) -> Self {
        Self::Kyber(error)
    }
}

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
    pub k_0: OtKey,
    pub k_1: OtKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KyberEndemicOtReceiverOutput {
    pub k_b: OtKey,
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
    ) -> Result<(Self, KyberEndemicOtReceiverMessage), KyberEndemicOtError> {
        let mut pk_real = [0u8; KYBER_PUBLICKEYBYTES];
        let mut sk = [0u8; KYBER_SECRETKEYBYTES];
        let mut r_other = [0u8; KYBER_PUBLICKEYBYTES];

        indcpa_keypair(&mut pk_real, &mut sk, None, rng)?;
        rng.fill_bytes(&mut r_other);

        let r_chosen = xor_public_keys(&pk_real, &hash_to_public_key_mask(&r_other));
        let (r_0, r_1) = if choice {
            (r_other, r_chosen)
        } else {
            (r_chosen, r_other)
        };

        Ok((
            Self { choice, sk },
            KyberEndemicOtReceiverMessage { r_0, r_1 },
        ))
    }

    pub fn recover_message(
        self,
        sender_msg: &KyberEndemicOtSenderMessage,
    ) -> Result<KyberEndemicOtReceiverOutput, KyberEndemicOtError> {
        let ciphertext = if self.choice {
            &sender_msg.ct_1
        } else {
            &sender_msg.ct_0
        };
        let mut k_b = [0u8; KYBER_SYMBYTES];

        indcpa_dec(&mut k_b, ciphertext, &self.sk);

        Ok(KyberEndemicOtReceiverOutput { k_b })
    }
}

impl KyberEndemicOtSender {
    pub fn respond(
        receiver_msg: &KyberEndemicOtReceiverMessage,
        rng: &mut (impl RngCore + CryptoRng),
    ) -> Result<(KyberEndemicOtSenderMessage, KyberEndemicOtSenderOutput), KyberEndemicOtError>
    {
        let mut k_0 = [0u8; KYBER_SYMBYTES];
        let mut k_1 = [0u8; KYBER_SYMBYTES];
        let mut coins_0 = [0u8; KYBER_SYMBYTES];
        let mut coins_1 = [0u8; KYBER_SYMBYTES];
        let mut ct_0 = [0u8; KYBER_CIPHERTEXTBYTES];
        let mut ct_1 = [0u8; KYBER_CIPHERTEXTBYTES];

        rng.fill_bytes(&mut k_0);
        rng.fill_bytes(&mut k_1);
        rng.fill_bytes(&mut coins_0);
        rng.fill_bytes(&mut coins_1);

        let pk_0 = xor_public_keys(
            &receiver_msg.r_0,
            &hash_to_public_key_mask(&receiver_msg.r_1),
        );
        let pk_1 = xor_public_keys(
            &receiver_msg.r_1,
            &hash_to_public_key_mask(&receiver_msg.r_0),
        );

        indcpa_enc(&mut ct_0, &k_0, &pk_0, &coins_0);
        indcpa_enc(&mut ct_1, &k_1, &pk_1, &coins_1);

        Ok((
            KyberEndemicOtSenderMessage { ct_0, ct_1 },
            KyberEndemicOtSenderOutput { k_0, k_1 },
        ))
    }
}

fn hash_to_public_key_mask(input: &[u8; KYBER_PUBLICKEYBYTES]) -> [u8; KYBER_PUBLICKEYBYTES] {
    let mut mask = [0u8; KYBER_PUBLICKEYBYTES];

    blake3::Hasher::new()
        .update(HASH_TO_PK_DOMAIN)
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
        let (receiver, receiver_msg) =
            KyberEndemicOtReceiver::gen_strings(false, &mut rng).unwrap();
        let (sender_msg, sender_output) =
            KyberEndemicOtSender::respond(&receiver_msg, &mut rng).unwrap();
        let receiver_output = receiver.recover_message(&sender_msg).unwrap();

        assert_eq!(receiver_output.k_b, sender_output.k_0);
        assert_ne!(receiver_output.k_b, sender_output.k_1);
    }

    #[test]
    fn kyber_endemic_ot_choice_true_matches_sender_k1() {
        let mut rng = thread_rng();
        let (receiver, receiver_msg) = KyberEndemicOtReceiver::gen_strings(true, &mut rng).unwrap();
        let (sender_msg, sender_output) =
            KyberEndemicOtSender::respond(&receiver_msg, &mut rng).unwrap();
        let receiver_output = receiver.recover_message(&sender_msg).unwrap();

        assert_eq!(receiver_output.k_b, sender_output.k_1);
        assert_ne!(receiver_output.k_b, sender_output.k_0);
    }

    #[test]
    fn kyber_endemic_ot_randomized_correctness() {
        let mut rng = thread_rng();

        for i in 0..32 {
            let choice = i % 2 == 1;
            let (receiver, receiver_msg) =
                KyberEndemicOtReceiver::gen_strings(choice, &mut rng).unwrap();
            let (sender_msg, sender_output) =
                KyberEndemicOtSender::respond(&receiver_msg, &mut rng).unwrap();
            let receiver_output = receiver.recover_message(&sender_msg).unwrap();

            if choice {
                assert_eq!(receiver_output.k_b, sender_output.k_1);
            } else {
                assert_eq!(receiver_output.k_b, sender_output.k_0);
            }
            assert_ne!(sender_output.k_0, sender_output.k_1);
        }
    }
}
