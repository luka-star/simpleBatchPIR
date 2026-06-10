use crate::rings::Zp;
use boomphf::Mphf;
use ndarray::Array2;
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::num::Wrapping;

pub type RecordIdx = usize;
pub type RecordIdxList = Vec<RecordIdx>;
pub type KeywordRecord = Vec<Zp>;

#[derive(Debug, Clone)]
pub struct PerfectHash {
    pub table_size: usize,
    pub mphf: Mphf<String>,
}

impl PerfectHash {
    pub fn slot(&self, keyword: &str) -> usize {
        self.mphf.hash(&keyword.to_owned()) as usize
    }
}

#[derive(Debug, Clone)]
pub struct KeywordClientContext {
    pub perfect_hash: PerfectHash,
    pub square_n: usize,
    pub record_size: usize,
}

impl KeywordClientContext {
    pub fn slot_for(&self, keyword: &str) -> usize {
        self.perfect_hash.slot(keyword)
    }

    pub fn keyword_record_cell_count(&self) -> usize {
        self.record_size * 2
    }
}

#[derive(Debug, Clone)]
pub struct KeywordDatabase {
    pub perfect_hash: PerfectHash,
    pub matrix: Array2<Zp>,
    pub record_size: usize,
}

pub type SecureKeywordDatabase = KeywordDatabase;

impl KeywordDatabase {
    pub fn client_context(&self) -> KeywordClientContext {
        KeywordClientContext {
            perfect_hash: self.perfect_hash.clone(),
            square_n: self.matrix.nrows(),
            record_size: self.record_size,
        }
    }
}

pub fn tokenize_text(text: &str) -> Vec<String> {
    const STOP_WORDS: &[&str] = &[
        "a", "an", "and", "as", "at", "by", "for", "from", "in", "into", "of", "on", "or", "the",
        "to", "with",
    ];

    let mut tokens = Vec::new();
    let mut seen = HashSet::new();

    for raw in text
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|part| !part.is_empty())
    {
        let token = raw.to_ascii_lowercase();

        if STOP_WORDS.contains(&token.as_str()) {
            continue;
        }

        if seen.insert(token.clone()) {
            tokens.push(token);
        }
    }

    tokens
}

pub fn collect_keywords<T>(mapping: &HashMap<String, T>) -> Vec<String> {
    let mut keywords: Vec<String> = mapping.keys().cloned().collect();
    keywords.sort_unstable();
    keywords
}

pub fn build_perfect_hash(keywords: &[String]) -> PerfectHash {
    let mut unique_keywords: Vec<String> = keywords.to_vec();
    unique_keywords.sort_unstable();
    unique_keywords.dedup();

    if unique_keywords.is_empty() {
        return PerfectHash {
            table_size: 0,
            mphf: Mphf::new(2.0, &unique_keywords),
        };
    }

    let mphf = Mphf::new(2.0, &unique_keywords);
    PerfectHash {
        table_size: unique_keywords.len(),
        mphf,
    }
}

pub fn encode_keyword_record(record_idxs: &[RecordIdx], record_size: usize) -> KeywordRecord {
    assert!(
        record_idxs.len() < record_size,
        "record index list does not fit into the keyword record"
    );

    let mut entries: Vec<u16> = Vec::with_capacity(record_size);
    entries.push(u16::try_from(record_idxs.len()).expect("record index count exceeds u16"));
    entries.extend(
        record_idxs
            .iter()
            .map(|record_idx| u16::try_from(*record_idx).expect("record index exceeds u16")),
    );
    entries.resize(record_size, 0);

    let mut record = Vec::with_capacity(record_size * 2);
    for entry in entries {
        let bytes = entry.to_le_bytes();
        record.push(Wrapping(bytes[0]));
        record.push(Wrapping(bytes[1]));
    }
    record
}

pub fn decode_keyword_record(bytes: &[u8]) -> RecordIdxList {
    let mut entries = bytes
        .chunks_exact(2)
        .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]) as usize);
    let count = entries.next().unwrap_or(0);

    entries.take(count).collect()
}

pub fn build_keyword_records(
    mapping: &HashMap<String, RecordIdxList>,
    perfect_hash: &PerfectHash,
) -> (Vec<KeywordRecord>, usize) {
    let max_record_idx_count = mapping.values().map(|idxs| idxs.len()).max().unwrap_or(0);
    let record_size = max_record_idx_count.saturating_add(1);
    let mut records = vec![encode_keyword_record(&[], record_size); perfect_hash.table_size];

    for (keyword, record_idxs) in mapping {
        let slot = perfect_hash.slot(keyword);
        records[slot] = encode_keyword_record(record_idxs, record_size);
    }

    (records, record_size)
}

pub fn pack_prebuilt_keyword_records(
    mapping: &HashMap<String, KeywordRecord>,
    perfect_hash: &PerfectHash,
    keyword_record_cell_count: usize,
) -> Vec<KeywordRecord> {
    let mut records = vec![vec![Wrapping(0); keyword_record_cell_count]; perfect_hash.table_size];

    for (keyword, record) in mapping {
        let slot = perfect_hash.slot(keyword);
        records[slot] = record.clone();
    }

    records
}

pub fn pack_keyword_records_into_square_matrix(records: &[KeywordRecord]) -> Array2<Zp> {
    let mut flat: Vec<Zp> = records
        .iter()
        .flat_map(|record| record.iter().copied())
        .collect();
    let total_elements = flat.len();
    let dim = (total_elements as f64).sqrt().ceil() as usize;
    flat.resize(dim * dim, Wrapping(0));
    Array2::from_shape_vec((dim, dim), flat).expect("failed to reshape keyword records into matrix")
}

pub fn build_keyword_database(mapping: &HashMap<String, RecordIdxList>) -> KeywordDatabase {
    let keywords = collect_keywords(mapping);
    let perfect_hash = build_perfect_hash(&keywords);
    let (records, record_size) = build_keyword_records(mapping, &perfect_hash);
    let matrix = pack_keyword_records_into_square_matrix(&records);

    KeywordDatabase {
        perfect_hash,
        matrix,
        record_size,
    }
}