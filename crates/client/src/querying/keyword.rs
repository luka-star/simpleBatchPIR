use ndarray::{Array1, Array2};
use shared::compute_a;
use shared::keyword::{KeywordClosure, SecureKeywordClosure};
use shared::rings::Zq;
use shared::{tokenize_text, RecordFetchRequest};

use super::plain::{recover_single, single_query};

#[derive(Debug, Clone)]
pub struct KeywordQueryState {
    pub s: Vec<Array1<Zq>>,
    pub block_start_cell: usize,
    pub eof: u16,
    pub square_n: usize,
}

pub fn normalize_keyword(keyword: &str) -> Option<String> {
    tokenize_text(keyword).into_iter().next()
}

pub fn keyword_to_slot(keyword: &str, closure: &KeywordClosure) -> Option<usize> {
    let normalized = normalize_keyword(keyword)?;
    Some(closure.slot_for(&normalized))
}

pub fn slot_to_matrix_pos(slot: usize, square_n: usize) -> (usize, usize) {
    (slot / square_n, slot % square_n)
}

fn get_block_positions(start_cell: usize,block_cell_count: usize,square_n: usize) -> Vec<(usize, usize)> {
    (0..block_cell_count)
        .map(|offset| {
            let cell_index = start_cell + offset;
            slot_to_matrix_pos(cell_index, square_n)
        })
        .collect()
}

pub fn keyword_query(keyword: &str,closure: &KeywordClosure) -> Option<(KeywordQueryState, Vec<Array1<Zq>>)> {
    let slot = keyword_to_slot(keyword, closure)?;
    let block_start_cell = slot * closure.block_cell_count();
    let square_n = closure.square_n;
    let matrix: Array2<Zq> = compute_a(square_n);
    let mut whole_query = Vec::with_capacity(square_n);
    let mut secrets = Vec::with_capacity(square_n);

    for i_col in 0..square_n {
        let (qu, s) = single_query(i_col, &matrix, square_n);
        whole_query.push(qu);
        secrets.push(s);
    }

    let state = KeywordQueryState {
        s: secrets,
        block_start_cell,
        eof: closure.eof,
        square_n,
    };

    Some((state, whole_query))
}

pub fn secure_keyword_query(
    keyword: &str,
    closure: &SecureKeywordClosure,
) -> Option<(KeywordQueryState, Vec<Array1<Zq>>)> {
    let slot = closure.slot_for(keyword);
    let block_start_cell = slot * closure.block_cell_count();
    let square_n = closure.square_n;
    let matrix: Array2<Zq> = compute_a(square_n);
    let mut whole_query = Vec::with_capacity(square_n);
    let mut secrets = Vec::with_capacity(square_n);

    for i_col in 0..square_n {
        let (qu, s) = single_query(i_col, &matrix, square_n);
        whole_query.push(qu);
        secrets.push(s);
    }

    let state = KeywordQueryState {
        s: secrets,
        block_start_cell,
        eof: closure.eof,
        square_n,
    };

    Some((state, whole_query))
}

pub fn recover_keyword_block(state: &KeywordQueryState,block_cell_count: usize,hint_c: &Array2<Zq>,answers: &[Array1<Zq>]) -> RecordFetchRequest {
    let bytes = recover_keyword_block_bytes(state, block_cell_count, hint_c, answers);
    decode_record_fetch_request(&bytes, state.eof)
}

pub fn recover_keyword_block_bytes(state: &KeywordQueryState,block_cell_count: usize,hint_c: &Array2<Zq>,answers: &[Array1<Zq>],) -> Vec<u8> {
    let positions = get_block_positions(state.block_start_cell, block_cell_count, state.square_n);
    let mut recovered = Array1::zeros(block_cell_count);

    for (offset, (row_idx, col_idx)) in positions.into_iter().enumerate() {
        recovered[offset] = recover_single(&state.s[col_idx], row_idx, hint_c, &answers[col_idx]);
    }

    recovered.iter().map(|z| z.0).collect()
}

pub fn decode_record_fetch_request(bytes: &[u8], eof: u16) -> RecordFetchRequest {
    let mut postings = Vec::new();

    for chunk in bytes.chunks_exact(2) {
        let value = u16::from_le_bytes([chunk[0], chunk[1]]);
        if value == eof {
            break;
        }
        postings.push(value as usize);
    }

    RecordFetchRequest::new(postings)
}
