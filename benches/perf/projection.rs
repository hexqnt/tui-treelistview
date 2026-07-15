use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput};
use tui_treelistview::{
    NoFilter, NoSort, TreeFilterConfig, TreeListViewState, TreeModel, TreeQuery, TreeRevision,
    TreeRootVisibility,
};

use super::fixture::{
    BenchFilter, BenchTree, WideIdTree, all_matches, descending, expanded_state, no_matches,
    sparse_filter,
};

pub fn rebuild(c: &mut Criterion) {
    let mut group = c.benchmark_group("projection/rebuild");

    for size in [5_000usize, 20_000, 100_000] {
        let model = BenchTree::balanced(size, 4);
        let mut query = TreeQuery::<NoFilter, NoSort>::new();
        let mut state = expanded_state(&model, &query);
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("balanced", size), &size, |b, _| {
            b.iter(|| {
                query.touch_sort();
                let rebuilt = state.ensure_projection(black_box(&model), black_box(&query));
                black_box((rebuilt, state.visible_len()));
            });
        });
    }

    for (name, model) in [
        ("chain", BenchTree::chain(20_000)),
        ("wide", BenchTree::balanced(20_000, 32)),
    ] {
        let mut query = TreeQuery::<NoFilter, NoSort>::new();
        let mut state = expanded_state(&model, &query);
        group.throughput(Throughput::Elements(20_000));
        group.bench_function(BenchmarkId::new(name, 20_000), |b| {
            b.iter(|| {
                query.touch_sort();
                let rebuilt = state.ensure_projection(black_box(&model), black_box(&query));
                black_box((rebuilt, state.visible_len()));
            });
        });
    }

    let model = BenchTree::chain(20_000);
    let mut query = TreeQuery::<NoFilter, NoSort>::new();
    let mut state = expanded_state(&model, &query);
    let _ = state.select_last();
    group.throughput(Throughput::Elements(20_000));
    group.bench_function(BenchmarkId::new("selected_chain", 20_000), |b| {
        b.iter(|| {
            query.touch_sort();
            let rebuilt = state.ensure_projection(black_box(&model), black_box(&query));
            black_box((rebuilt, state.selected_index()));
        });
    });

    let forest = BenchTree::forest(20_000, 4, 64);
    for visibility in [TreeRootVisibility::Visible, TreeRootVisibility::Hidden] {
        let name = match visibility {
            TreeRootVisibility::Visible => "forest_visible",
            TreeRootVisibility::Hidden => "forest_hidden",
        };
        let mut query = TreeQuery::new().with_root_visibility(visibility);
        let mut state = expanded_state(&forest, &query);
        group.throughput(Throughput::Elements(20_000));
        group.bench_function(BenchmarkId::new(name, 20_000), |b| {
            b.iter(|| {
                query.touch_sort();
                let rebuilt = state.ensure_projection(black_box(&forest), black_box(&query));
                black_box((rebuilt, state.visible_len()));
            });
        });
    }

    group.finish();
}

pub fn cache(c: &mut Criterion) {
    let model = BenchTree::balanced(20_000, 4);
    let query = TreeQuery::new();
    let mut state = expanded_state(&model, &query);
    let mut group = c.benchmark_group("projection/cache_hit");
    group.throughput(Throughput::Elements(1));
    group.bench_function("balanced/20000", |b| {
        b.iter(|| {
            black_box(state.ensure_projection(black_box(&model), black_box(&query)));
        });
    });
    group.finish();
}

pub fn id_width(c: &mut Criterion) {
    let mut group = c.benchmark_group("projection/id_width");
    group.throughput(Throughput::Elements(20_000));

    {
        let model = BenchTree::balanced(20_000, 4);
        let mut query = TreeQuery::<NoFilter, NoSort>::new();
        let mut state = expanded_state(&model, &query);
        group.bench_function("usize/20000", |b| {
            b.iter(|| {
                query.touch_sort();
                let rebuilt = state.ensure_projection(black_box(&model), black_box(&query));
                black_box((rebuilt, state.visible_len()));
            });
        });
    }

    {
        let model = WideIdTree::balanced(20_000, 4);
        let mut query = TreeQuery::<NoFilter, NoSort>::new();
        let mut state = TreeListViewState::with_capacity(model.size_hint());
        let _ = state.expand_all(&model);
        let _ = state.ensure_projection(&model, &query);
        group.bench_function("u128/20000", |b| {
            b.iter(|| {
                query.touch_sort();
                let rebuilt = state.ensure_projection(black_box(&model), black_box(&query));
                black_box((rebuilt, state.visible_len()));
            });
        });
    }

    group.finish();
}

pub fn filter(c: &mut Criterion) {
    let model = BenchTree::balanced(20_000, 4);
    let cases: [(&str, BenchFilter, TreeFilterConfig); 5] = [
        ("none", no_matches, TreeFilterConfig::enabled()),
        ("sparse/auto", sparse_filter, TreeFilterConfig::enabled()),
        (
            "sparse/manual",
            sparse_filter,
            TreeFilterConfig::enabled_manual_expand(),
        ),
        ("all/auto", all_matches, TreeFilterConfig::enabled()),
        (
            "all/manual",
            all_matches,
            TreeFilterConfig::enabled_manual_expand(),
        ),
    ];
    let mut group = c.benchmark_group("projection/filter");
    group.throughput(Throughput::Elements(20_000));

    for (name, filter, config) in cases {
        let mut query = TreeQuery::new().with_filter(filter, config, TreeRevision::INITIAL);
        let mut state = TreeListViewState::with_capacity(model.size_hint());
        group.bench_function(BenchmarkId::new(name, 20_000), |b| {
            b.iter(|| {
                query.touch_filter();
                let rebuilt = state.ensure_projection(black_box(&model), black_box(&query));
                black_box((rebuilt, state.visible_len()));
            });
        });
    }
    group.finish();
}

pub fn sort(c: &mut Criterion) {
    let model = BenchTree::balanced(20_000, 32);
    let mut query = TreeQuery::new().with_sort(descending, TreeRevision::INITIAL);
    let mut state = expanded_state(&model, &query);
    let mut group = c.benchmark_group("projection/sort");
    group.throughput(Throughput::Elements(20_000));
    group.bench_function("wide/20000", |b| {
        b.iter(|| {
            query.touch_sort();
            let rebuilt = state.ensure_projection(black_box(&model), black_box(&query));
            black_box((rebuilt, state.visible_len()));
        });
    });
    group.finish();
}
