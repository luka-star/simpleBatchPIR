use crate::PrfInput;

pub fn choose_from_row_mock<T: Copy>(row: &[T], index: usize) -> T {
    row[index]
}

pub fn choose_pair_mock<T: Copy>(row1: &[T], row2: &[T], input: PrfInput) -> (T, T) {
    (
        choose_from_row_mock(row1, input.x1),
        choose_from_row_mock(row2, input.x2),
    )
}
