mod support;

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use shared::models::Band;
use std::path::Path;
use support::make_bands;

criterion_group! {
    name = benches;
    config = Criterion::default()
        .output_directory(Path::new("benchmark-results"))
        .measurement_time(std::time::Duration::from_secs(20));
    targets =
        bench_pack_one_entry,
        bench_unpack_one_entry,
        bench_matrix_conversion,
}

fn bench_pack_one_entry(c: &mut Criterion) {
    let bands = make_bands(1).expect("Failed to fetch bands");
    let band = &bands[0];

    c.bench_function("pack_one", |b| {
        b.iter(|| Band::pack_band_to_zp(black_box(band)))
    });
}

fn bench_unpack_one_entry(c: &mut Criterion) {
    let bands = make_bands(1).expect("Failed to fetch bands");
    let band = &bands[0];
    let packed = Band::pack_band_to_zp(band);

    c.bench_function("unpack_one", |b| {
        b.iter(|| Band::unpack_band_from_zp(black_box(&packed)))
    });
}

fn bench_matrix_conversion(c: &mut Criterion) {
    let bands = make_bands(1024).expect("Failed to fetch bands");

    c.bench_function("bands_to_matrix_1024", |b| {
        b.iter(|| Band::bands_to_matrix(black_box(&bands)))
    });
}

criterion_main!(benches);
