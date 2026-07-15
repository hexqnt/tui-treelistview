use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput};
use tui_treelistview::{TreeListViewState, TreeModel, TreeQuery};

use super::fixture::BenchTree;

pub fn marks_cache(c: &mut Criterion) {
    let model = BenchTree::balanced(20_000, 4);
    let mut state = TreeListViewState::with_capacity(model.size_hint());
    state.ensure_mark_states(&model);
    let mut group = c.benchmark_group("marks/cache_hit");
    group.throughput(Throughput::Elements(1));
    group.bench_function("balanced/20000", |b| {
        b.iter(|| {
            state.ensure_mark_states(black_box(&model));
            black_box(state.mark_state(0));
        });
    });
    group.finish();
}

pub fn marks_rebuild(c: &mut Criterion) {
    let mut group = c.benchmark_group("marks/rebuild");
    for size in [5_000usize, 20_000, 100_000] {
        let model = BenchTree::balanced(size, 4);
        let mut state = TreeListViewState::with_capacity(model.size_hint());
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("single_leaf", size), &size, |b, _| {
            b.iter(|| {
                let _ = state.toggle_marked(size - 1);
                state.ensure_mark_states(black_box(&model));
                black_box(state.mark_state(0));
            });
        });
    }

    let model = BenchTree::balanced(20_000, 4);
    let leaves: Vec<_> = model.leaves().collect();
    let toggled = leaves.last().copied().expect("tree has leaves");
    let mut state = TreeListViewState::with_capacity(model.size_hint());
    for leaf in leaves.iter().step_by(2).copied() {
        let _ = state.set_marked(leaf, true);
    }
    state.ensure_mark_states(&model);
    group.throughput(Throughput::Elements(20_000));
    group.bench_function(BenchmarkId::new("half_leaves", 20_000), |b| {
        b.iter(|| {
            let _ = state.toggle_marked(toggled);
            state.ensure_mark_states(black_box(&model));
            black_box(state.mark_state(0));
        });
    });
    group.finish();
}

pub fn interaction(c: &mut Criterion) {
    const SIZE: usize = 20_000;
    let model = BenchTree::chain(SIZE);
    let query = TreeQuery::new();
    let mut group = c.benchmark_group("interaction/deep_chain");
    group.throughput(Throughput::Elements(SIZE as u64));

    group.bench_function("expand_to/20000", |b| {
        b.iter_batched_ref(
            || TreeListViewState::with_capacity(SIZE),
            |state| black_box(state.expand_to(black_box(&model), SIZE - 1)),
            BatchSize::LargeInput,
        );
    });
    group.bench_function("select_by_id/20000", |b| {
        b.iter_batched_ref(
            || TreeListViewState::with_capacity(SIZE),
            |state| {
                black_box(state.select_by_id(black_box(&model), black_box(&query), SIZE - 1));
            },
            BatchSize::LargeInput,
        );
    });
    group.finish();
}
