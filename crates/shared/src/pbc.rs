use std::collections::{HashMap};
use std::hash::{BuildHasher, Hash, Hasher};
use rand::Rng;

use ahash::RandomState;

#[derive(Clone)]
pub struct PBCConfig {
    pub buckets: usize,
    pub seeds: Vec<u64>
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
    let max_evict = config.buckets / 3;
    let mut occupied_bucket: HashMap<usize, T> = HashMap::new();
    let mut schedule: HashMap<T, usize> = HashMap::new();

    for &index in indices {
        insert_cuckoo(config, index, max_evict, &mut occupied_bucket, &mut schedule)?;
    }

    Ok(schedule)
}

fn insert_cuckoo<T: Hash + Copy + Eq>(
    config: &PBCConfig,
    mut index: T,
    max_evict: usize,
    occupied_bucket: &mut HashMap<usize, T>,
    schedule: &mut HashMap<T, usize>,
) -> Result<(), String> {
    let mut rng = rand::thread_rng();
    for _ in 0..max_evict {
        let candidates = config.bucket_positions(&index);
        if candidates.is_empty() {
            return Err("No candidate buckets".to_string());
        }
        if let Some(bucket) = candidates
            .iter()
            .find(|bucket| !occupied_bucket.contains_key(bucket))
        {
            occupied_bucket.insert(*bucket, index);
            schedule.insert(index, *bucket);
            return Ok(());
        }
        let bucket = candidates[rng.gen_range(0..candidates.len())];
        let evicted = occupied_bucket.insert(bucket, index).unwrap();
        schedule.insert(index, bucket);
        schedule.remove(&evicted);
        index = evicted;
    }
    Err("Could not generate a cuckoo schedule".to_string())
}