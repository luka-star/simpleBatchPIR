use rand::RngCore;

pub type GKey = [u8; 32];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OprfKey {
    pub row1: Vec<GKey>,
    pub row2: Vec<GKey>,
}

pub fn keygen(m: usize, rng: &mut impl RngCore) -> OprfKey {
    OprfKey {
        row1: sample_row(m, rng),
        row2: sample_row(m, rng),
    }
}

fn sample_row(m: usize, rng: &mut impl RngCore) -> Vec<GKey> {
    (0..m)
        .map(|_| {
            let mut key = [0u8; 32];
            rng.fill_bytes(&mut key);
            key
        })
        .collect()
}
