use crate::models::Band;
use crate::rings::Zp;
use boomphf::Mphf;
use ndarray::Array2;
use oprf::{encode_keyword, eval_keyword, OprfKey, DEFAULT_M};
use std::collections::{HashMap, HashSet};
use std::convert::TryFrom;
use std::num::Wrapping;

pub type RecordId = usize;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordFetchRequest {
    record_ids: Vec<RecordId>,
}

impl RecordFetchRequest {
    pub fn new(record_ids: Vec<RecordId>) -> Self {
        Self { record_ids }
    }

    pub fn record_ids(&self) -> &[RecordId] {
        &self.record_ids
    }

    pub fn is_empty(&self) -> bool {
        self.record_ids.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct PerfectHash {
    pub table_size: usize,
    pub mphf: Mphf<String>,
}

impl PerfectHash {
    pub fn slot(&self, keyword: &str) -> usize {
        self.mphf.hash(&keyword.to_owned()) as usize
    }

    pub fn len(&self) -> usize {
        self.table_size
    }

    pub fn is_empty(&self) -> bool {
        self.table_size == 0
    }
}

#[derive(Debug, Clone)]
pub struct KeywordIndex {
    pub perfect_hash: PerfectHash,
    pub matrix: Array2<Zp>,
    pub record_size: usize,
    pub entry_width_bytes: usize,
    pub eof: u16,
}

#[derive(Debug, Clone)]
pub struct SecureKeywordIndex {
    pub perfect_hash: PerfectHash,
    pub matrix: Array2<Zp>,
    pub record_size: usize,
    pub entry_width_bytes: usize,
    pub eof: u16,
}

impl KeywordIndex {
    pub fn square_n(&self) -> usize {
        self.matrix.nrows()
    }

    pub fn closure(&self) -> KeywordClosure {
        KeywordClosure {
            perfect_hash: self.perfect_hash.clone(),
            square_n: self.square_n(),
            record_size: self.record_size,
            eof: self.eof,
        }
    }
}

impl SecureKeywordIndex {
    pub fn square_n(&self) -> usize {
        self.matrix.nrows()
    }

    pub fn closure(&self) -> SecureKeywordClosure {
        SecureKeywordClosure {
            perfect_hash: self.perfect_hash.clone(),
            square_n: self.square_n(),
            record_size: self.record_size,
            eof: self.eof,
        }
    }
}

#[derive(Debug, Clone)]
pub struct KeywordClosure {
    pub perfect_hash: PerfectHash,
    pub square_n: usize,
    pub record_size: usize,
    pub eof: u16,
}

#[derive(Debug, Clone)]
pub struct SecureKeywordClosure {
    pub perfect_hash: PerfectHash,
    pub square_n: usize,
    pub record_size: usize,
    pub eof: u16,
}

impl KeywordClosure {
    pub fn slot_for(&self, keyword: &str) -> usize {
        self.perfect_hash.slot(keyword)
    }

    pub fn block_cell_count(&self) -> usize {
        self.record_size * 2
    }

    pub fn block_start_cell_for(&self, keyword: &str) -> usize {
        self.slot_for(keyword) * self.block_cell_count()
    }
}

impl SecureKeywordClosure {
    pub fn slot_for(&self, keyword: &str) -> usize {
        self.perfect_hash.slot(keyword)
    }

    pub fn block_cell_count(&self) -> usize {
        self.record_size * 2
    }

    pub fn block_start_cell_for(&self, keyword: &str) -> usize {
        self.slot_for(keyword) * self.block_cell_count()
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

fn push_tokens_from_text(text: &str, tokens: &mut Vec<String>, seen: &mut HashSet<String>) {
    for token in tokenize_text(text) {
        if seen.insert(token.clone()) {
            tokens.push(token);
        }
    }
}

pub fn tokenize_band(band: &Band) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut seen = HashSet::new();

    if let Some(name) = band.name.as_deref() {
        push_tokens_from_text(name, &mut tokens, &mut seen);
    }
    if let Some(origin) = band.origin.as_deref() {
        push_tokens_from_text(origin, &mut tokens, &mut seen);
    }
    if let Some(style) = band.style.as_deref() {
        for style_part in style.split(',') {
            push_tokens_from_text(style_part, &mut tokens, &mut seen);
        }
    }
    if band.formed > 0 {
        let token = band.formed.to_string();
        if seen.insert(token.clone()) {
            tokens.push(token);
        }
    }
    if band.split > 0 {
        let token = band.split.to_string();
        if seen.insert(token.clone()) {
            tokens.push(token);
        }
    }

    tokens
}

pub fn construct_keyword_mapping(db: &[Band]) -> HashMap<String, Vec<RecordId>> {
    let mut mapping: HashMap<String, Vec<RecordId>> = HashMap::new();

    for band in db {
        for token in tokenize_band(band) {
            mapping.entry(token).or_default().push(band.id as usize);
        }
    }

    mapping
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
            mphf: Mphf::new(1.7, &unique_keywords),
        };
    }

    let mphf = Mphf::new(1.7, &unique_keywords);
    PerfectHash {
        table_size: unique_keywords.len(),
        mphf,
    }
}

fn pack_posting_block(postings: &[RecordId], record_size: usize, eof: u16) -> Vec<Zp> {
    assert!(
        postings.len() < record_size,
        "posting list does not fit into the fixed-size block"
    );

    let mut entries: Vec<u16> = postings
        .iter()
        .map(|posting| u16::try_from(*posting).expect("posting index exceeds u16"))
        .collect();
    entries.push(eof);
    entries.resize(record_size, eof);

    let mut block = Vec::with_capacity(record_size * 2);
    for entry in entries {
        let bytes = entry.to_le_bytes();
        block.push(Wrapping(bytes[0]));
        block.push(Wrapping(bytes[1]));
    }
    block
}

pub fn build_posting_blocks(mapping: &HashMap<String, Vec<RecordId>>,perfect_hash: &PerfectHash,eof: u16) -> (Vec<Vec<Zp>>, usize) {
    let max_posting_len = mapping.values().map(|posts| posts.len()).max().unwrap_or(0);
    let record_size = max_posting_len.saturating_add(1);
    let mut blocks = vec![pack_posting_block(&[], record_size, eof); perfect_hash.table_size];

    for (keyword, postings) in mapping {
        let slot = perfect_hash.slot(keyword);
        blocks[slot] = pack_posting_block(postings, record_size, eof);
    }

    (blocks, record_size)
}

fn build_masked_posting_blocks(mapping: &HashMap<String, Vec<RecordId>>,oprf_key: &OprfKey,eof: u16) -> (HashMap<String, Vec<Zp>>, usize) {
    let max_posting_len = mapping.values().map(|posts| posts.len()).max().unwrap_or(0);
    let record_size = max_posting_len.saturating_add(1);
    let block_cell_count = record_size * 2;
    let mut masked_mapping = HashMap::with_capacity(mapping.len());

    for (keyword, postings) in mapping {
        let input = encode_keyword(keyword, DEFAULT_M);
        let token = eval_keyword(&oprf_key.row1[input.x1], &oprf_key.row2[input.x2], input, block_cell_count);
        let block = pack_posting_block(postings, record_size, eof);
        let masked_block: Vec<Zp> = block
            .into_iter()
            .zip(token.p_hat.into_iter())
            .map(|(cell, mask)| Wrapping(cell.0 ^ mask))
            .collect();

        masked_mapping.insert(token.x_hat, masked_block);
    }

    (masked_mapping, record_size)
}

fn pack_prebuilt_blocks(mapping: &HashMap<String, Vec<Zp>>,perfect_hash: &PerfectHash,block_cell_count: usize) -> Vec<Vec<Zp>> {
    let mut blocks = vec![vec![Wrapping(0); block_cell_count]; perfect_hash.table_size];

    for (keyword, block) in mapping {
        let slot = perfect_hash.slot(keyword);
        blocks[slot] = block.clone();
    }

    blocks
}

pub fn pack_keyword_blocks_into_square_matrix(blocks: &[Vec<Zp>]) -> Array2<Zp> {
    let mut flat: Vec<Zp> = blocks
        .iter()
        .flat_map(|block| block.iter().copied())
        .collect();
    let total_elements = flat.len();
    let dim = (total_elements as f64).sqrt().ceil() as usize;
    flat.resize(dim * dim, Wrapping(0));
    Array2::from_shape_vec((dim, dim), flat).expect("failed to reshape keyword blocks into matrix")
}

pub fn build_keyword_index(db: &[Band]) -> KeywordIndex {
    let mapping = construct_keyword_mapping(db);
    let keywords = collect_keywords(&mapping);
    let perfect_hash = build_perfect_hash(&keywords);
    let eof = u16::MAX;
    let (blocks, record_size) = build_posting_blocks(&mapping, &perfect_hash, eof);
    let matrix = pack_keyword_blocks_into_square_matrix(&blocks);

    KeywordIndex {
        perfect_hash,
        matrix,
        record_size,
        entry_width_bytes: 2,
        eof,
    }
}

pub fn build_secure_keyword_index(db: &[Band], oprf_key: &OprfKey) -> SecureKeywordIndex {
    let mapping = construct_keyword_mapping(db);
    let eof = u16::MAX;
    let (masked_mapping, record_size) = build_masked_posting_blocks(&mapping, oprf_key, eof);
    let secure_keywords = collect_keywords(&masked_mapping);
    let perfect_hash = build_perfect_hash(&secure_keywords);
    let blocks = pack_prebuilt_blocks(&masked_mapping, &perfect_hash, record_size * 2);
    let matrix = pack_keyword_blocks_into_square_matrix(&blocks);

    SecureKeywordIndex {
        perfect_hash,
        matrix,
        record_size,
        entry_width_bytes: 2,
        eof,
    }
}
