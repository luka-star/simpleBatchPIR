use crate::keygen::GKey;
use crate::PrfInput;

pub fn choose_from_row_mock(row: &[GKey], index: usize) -> GKey {
    row[index]
}

pub fn choose_pair_mock(row1: &[GKey], row2: &[GKey], input: PrfInput) -> (GKey, GKey) {
    (
        choose_from_row_mock(row1, input.x1),
        choose_from_row_mock(row2, input.x2),
    )
}
