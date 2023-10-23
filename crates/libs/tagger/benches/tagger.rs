use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tagger::TaggerQueryBuilder;

fn criterion_benchmark(c: &mut Criterion) {
    c.bench_function("address", |b| {
        b.iter(|| {
            TaggerQueryBuilder::all()
                .apply_taggers(black_box("156BIS Route de Dijon Brazey-en-Plaine"), false)
        })
    });
    c.bench_function("street", |b| {
        b.iter(|| {
            TaggerQueryBuilder::all()
                .apply_taggers(black_box("Route de Dijon Brazey-en-Plaine"), false)
        })
    });
    c.bench_function("location", |b| {
        b.iter(|| {
            TaggerQueryBuilder::all().apply_taggers(black_box("Franconville-la-garenne"), false)
        })
    });
    c.bench_function("mixed", |b| {
        b.iter(|| TaggerQueryBuilder::all().apply_taggers(black_box("magasin apple"), false))
    });
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
