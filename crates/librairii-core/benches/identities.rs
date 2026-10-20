use criterion::{Criterion, criterion_group, criterion_main};
use librairii_core::{Checksum, StoryUuid};
use std::hint::black_box;
fn identities(c: &mut Criterion) {
    let valid = "ab".repeat(32);
    let invalid = "é".repeat(32);
    c.bench_function("checksum/parse-valid-64-ascii", |b| {
        b.iter(|| black_box(valid.as_str()).parse::<Checksum>())
    });
    c.bench_function("checksum/reject-multibyte-64-bytes", |b| {
        b.iter(|| black_box(invalid.as_str()).parse::<Checksum>())
    });
    c.bench_function("uuid/parse-canonical", |b| {
        b.iter(|| black_box("aabbccdd-1234-5678-9012-aabbccddeeff").parse::<StoryUuid>())
    });
}
criterion_group!(benches, identities);
criterion_main!(benches);
