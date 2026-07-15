use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput};
use tui_treelistview::{IndexedTree, TreeRevision};

use super::fixture::BenchTree;

pub fn indexed_tree(c: &mut Criterion) {
    const SIZE: usize = 20_000;
    let mut group = c.benchmark_group("adapter/indexed_tree");
    group.throughput(Throughput::Elements(SIZE as u64));

    for (name, model) in [
        ("balanced", BenchTree::balanced(SIZE, 4)),
        ("chain", BenchTree::chain(SIZE)),
    ] {
        group.bench_function(BenchmarkId::new(name, SIZE), |b| {
            b.iter(|| {
                black_box(
                    IndexedTree::new(
                        model.roots.iter().copied(),
                        black_box(&model.children),
                        TreeRevision::INITIAL,
                    )
                    .expect("generated tree is valid"),
                );
            });
        });
    }
    group.finish();
}
