use ndarray::{Array1, Array2};
use shared::compute_a;
use shared::keyword::{KeywordClosure, RecordFetchRequest};
use shared::rings::Zq;
use shared::tokenize_text;

use super::plain::{recover_single, single_query};

#[derive(Debug, Clone)]
pub struct KeywordQueryState {
    pub s: Vec<Array1<Zq>>,
    pub block_start_cell: usize,
    pub eof: u16,
    pub side_len: usize,
}

pub fn normalize_keyword(keyword: &str) -> Option<String> {
    tokenize_text(keyword).into_iter().next()
}

pub fn keyword_to_slot(keyword: &str, closure: &KeywordClosure) -> Option<usize> {
    let normalized = normalize_keyword(keyword)?;
    Some(closure.slot_for(&normalized))
}

pub fn slot_to_matrix_pos(slot: usize, side_len: usize) -> (usize, usize) {
    (slot / side_len, slot % side_len)
}

fn get_block_positions(
    start_cell: usize,
    block_cell_count: usize,
    side_len: usize,
) -> Vec<(usize, usize)> {
    (0..block_cell_count)
        .map(|offset| {
            let cell_index = start_cell + offset;
            slot_to_matrix_pos(cell_index, side_len)
        })
        .collect()
}

pub fn keyword_query(keyword: &str,closure: &KeywordClosure) -> Option<(KeywordQueryState, Vec<Array1<Zq>>)> {
    let slot = keyword_to_slot(keyword, closure)?;
    let block_start_cell = slot * closure.block_cell_count();
    let side_len = closure.side_len;
    let matrix: Array2<Zq> = compute_a(side_len);
    let mut whole_query = Vec::with_capacity(side_len);
    let mut secrets = Vec::with_capacity(side_len);

    for i_col in 0..side_len {
        let (qu, s) = single_query(i_col, &matrix, side_len);
        whole_query.push(qu);
        secrets.push(s);
    }

    let state = KeywordQueryState {
        s: secrets,
        block_start_cell,
        eof: closure.eof,
        side_len,
    };

    Some((state, whole_query))
}

pub fn recover_keyword_block(state: &KeywordQueryState,block_cell_count: usize,hint_c: &Array2<Zq>,answers: &[Array1<Zq>]) -> RecordFetchRequest {
    let positions = get_block_positions(state.block_start_cell, block_cell_count, state.side_len);
    let mut recovered = Array1::zeros(block_cell_count);

    for (offset, (row_idx, col_idx)) in positions.into_iter().enumerate() {
        recovered[offset] = recover_single(&state.s[col_idx], row_idx, hint_c, &answers[col_idx]);
    }

    let bytes: Vec<u8> = recovered.iter().map(|z| z.0).collect();
    let mut postings = Vec::new();

    for chunk in bytes.chunks_exact(2) {
        let value = u16::from_le_bytes([chunk[0], chunk[1]]);
        if value == state.eof {
            break;
        }
        postings.push(value as usize);
    }
    RecordFetchRequest::new(postings)
}
