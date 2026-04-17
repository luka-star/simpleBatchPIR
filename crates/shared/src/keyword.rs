use crate::models::Band;
use crate::rings::Zp;
use boomphf::Mphf;
use ndarray::Array2;
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
    pub block_entry_count: usize,
    pub entry_width_bytes: usize,
    pub eof: u16,
}

impl KeywordIndex {
    pub fn side_len(&self) -> usize {
        self.matrix.nrows()
    }

    pub fn closure(&self) -> KeywordClosure {
        KeywordClosure {
            perfect_hash: self.perfect_hash.clone(),
            side_len: self.side_len(),
            block_entry_count: self.block_entry_count,
            eof: self.eof,
        }
    }
}

#[derive(Debug, Clone)]
pub struct KeywordClosure {
    pub perfect_hash: PerfectHash,
    pub side_len: usize,
    pub block_entry_count: usize,
    pub eof: u16,
}

impl KeywordClosure {
    pub fn slot_for(&self, keyword: &str) -> usize {
        self.perfect_hash.slot(keyword)
    }

    pub fn block_cell_count(&self) -> usize {
        self.block_entry_count * 2
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

pub fn collect_keywords(mapping: &HashMap<String, Vec<usize>>) -> Vec<String> {
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

fn pack_posting_block(postings: &[RecordId], block_entry_count: usize, eof: u16) -> Vec<Zp> {
    assert!(
        postings.len() < block_entry_count,
        "posting list does not fit into the fixed-size block"
    );

    let mut entries: Vec<u16> = postings
        .iter()
        .map(|posting| u16::try_from(*posting).expect("posting index exceeds u16"))
        .collect();
    entries.push(eof);
    entries.resize(block_entry_count, eof);

    let mut block = Vec::with_capacity(block_entry_count * 2);
    for entry in entries {
        let bytes = entry.to_le_bytes();
        block.push(Wrapping(bytes[0]));
        block.push(Wrapping(bytes[1]));
    }
    block
}

pub fn build_posting_blocks(mapping: &HashMap<String, Vec<RecordId>>,perfect_hash: &PerfectHash,eof: u16) -> (Vec<Vec<Zp>>, usize) {
    let max_posting_len = mapping.values().map(|posts| posts.len()).max().unwrap_or(0);
    let block_entry_count = max_posting_len.saturating_add(1);
    let mut blocks = vec![pack_posting_block(&[], block_entry_count, eof); perfect_hash.table_size];

    for (keyword, postings) in mapping {
        let slot = perfect_hash.slot(keyword);
        blocks[slot] = pack_posting_block(postings, block_entry_count, eof);
    }

    (blocks, block_entry_count)
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
    let (blocks, block_entry_count) = build_posting_blocks(&mapping, &perfect_hash, eof);
    let matrix = pack_keyword_blocks_into_square_matrix(&blocks);

    KeywordIndex {
        perfect_hash,
        matrix,
        block_entry_count,
        entry_width_bytes: 2,
        eof,
    }
}
