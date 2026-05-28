use ahash::RandomState;
use rand::Rng;
use std::collections::HashMap;
use std::hash::{BuildHasher, Hash, Hasher};

#[derive(Clone)]
pub struct PBCConfig {
    pub buckets: usize,
    pub seeds: Vec<u64>,
}

impl PBCConfig {
    pub fn fixed_seeds(buckets: usize, w: usize) -> Self {
        let seeds = (0..w)
            .map(|i| 0x9e3779b97f4a7c15u64.wrapping_mul(i as u64 + 1))
            .collect();

        Self { buckets, seeds }
    }

    pub fn random_seeds(buckets: usize, w: usize) -> Self {
        let mut rng = rand::thread_rng();
        let seeds = (0..w).map(|_| rng.gen()).collect();

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
    let max_loop = max_loop(config, indices.len())?;
    let mut occupied_bucket: HashMap<usize, T> = HashMap::new();
    let mut schedule: HashMap<T, usize> = HashMap::new();

    for &index in indices {
        insert_cuckoo(config, index, max_loop, &mut occupied_bucket, &mut schedule)?;
    }

    Ok(schedule)
}

fn max_loop(config: &PBCConfig, item_count: usize) -> Result<usize, String> {
    if item_count == 0 {
        return Ok(0);
    }
    if item_count > config.buckets {
        return Err("Cannot schedule more indices than buckets".to_string());
    }
    if item_count == config.buckets {
        return Ok(config.buckets);
    }

    let base = config.buckets as f64 / item_count as f64;
    let bound = 3.0 * (item_count as f64).log(base);
    Ok((bound.ceil() as usize).max(config.w()).max(1))
}

fn insert_cuckoo<T: Hash + Copy + Eq>(
    config: &PBCConfig,
    mut index: T,
    max_loop: usize,
    occupied_bucket: &mut HashMap<usize, T>,
    schedule: &mut HashMap<T, usize>,
) -> Result<(), String> {
    let mut rng = rand::thread_rng();
    for _ in 0..max_loop {
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
