use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasher, Hash, Hasher};

use ahash::RandomState;

#[derive(Clone)]
pub struct PBCConfig {
    pub buckets: usize,
    pub seeds: Vec<u64>,
}
impl PBCConfig {
    pub fn new(buckets: usize, w: usize) -> Self {
        let seeds = (0..w)
            .map(|i| 0x9e3779b97f4a7c15u64.wrapping_mul(i as u64 + 1))
            .collect();

        Self { buckets, seeds }
    }

    pub fn w(&self) -> usize {
        self.seeds.len()
    }

    fn hash_single<T: Hash>(&self, value: &T, seed: u64) -> usize {
        let state = RandomState::with_seeds(seed, seed ^ 0xdeadbeef, 0, 0);
        let mut hasher = state.build_hasher();
        value.hash(&mut hasher);
        (hasher.finish() as usize) % self.buckets
    }

    pub fn bucket_positions<T: Hash>(&self, value: &T) -> Vec<usize> {
        self.seeds
            .iter()
            .map(|seed| self.hash_single(value, *seed))
            .collect()
    }
}

pub fn gen_schedule<T: Hash + Copy + Eq>(
    config: &PBCConfig,
    indices: &[T],
) -> Result<HashMap<T, usize>, String> {
    let mut used_buckets = HashSet::new();
    let mut schedule = HashMap::new();

    for &index in indices {
        let mut placed = false;

        for bucket in config.bucket_positions(&index) {
            if !used_buckets.contains(&bucket) {
                schedule.insert(index, bucket);
                used_buckets.insert(bucket);
                placed = true;
                break;
            }
        }

        if !placed {
            return Err("Could not generate a schedule :(".to_string());
        }
    }

    Ok(schedule)
}
