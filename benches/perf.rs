use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use ratatui::layout::Rect;
use ratatui::prelude::Buffer;
use ratatui::widgets::{Cell, StatefulWidget};

use tui_treelistview::{
    ColumnDef, ColumnWidth, IndexedTree, NoFilter, NoSort, TreeChildren, TreeColumnSet, TreeFilter,
    TreeFilterConfig, TreeGlyphs, TreeLabelRenderer, TreeListView, TreeListViewState,
    TreeListViewStyle, TreeModel, TreeQuery, TreeRevision, TreeRootVisibility, TreeRowContext,
    TreeRowRendering, TreeSort,
};

type BenchFilter = fn(&BenchTree, usize) -> bool;

struct BenchTree {
    roots: Vec<usize>,
    children: Vec<Vec<usize>>,
}

impl BenchTree {
    fn balanced(node_count: usize, fanout: usize) -> Self {
        Self::forest(node_count, fanout, 1)
    }

    fn forest(node_count: usize, fanout: usize, root_count: usize) -> Self {
        let node_count = node_count.max(1);
        let root_count = root_count.clamp(1, node_count);
        let roots = (0..root_count).collect();
        let mut children = vec![Vec::new(); node_count];
        let mut next_id = root_count;

        for node_children in &mut children {
            for _ in 0..fanout {
                if next_id == node_count {
                    break;
                }
                node_children.push(next_id);
                next_id += 1;
            }
            if next_id == node_count {
                break;
            }
        }

        Self { roots, children }
    }

    fn chain(node_count: usize) -> Self {
        let node_count = node_count.max(1);
        let mut children = vec![Vec::new(); node_count];
        for (id, node_children) in children.iter_mut().enumerate().take(node_count - 1) {
            node_children.push(id + 1);
        }
        Self {
            roots: vec![0],
            children,
        }
    }

    fn leaves(&self) -> impl Iterator<Item = usize> + '_ {
        self.children
            .iter()
            .enumerate()
            .filter_map(|(id, children)| children.is_empty().then_some(id))
    }
}

impl TreeModel for BenchTree {
    type Id = usize;

    fn roots(&self) -> impl Iterator<Item = Self::Id> + '_ {
        self.roots.iter().copied()
    }

    fn children(&self, id: Self::Id) -> TreeChildren<'_, Self::Id> {
        TreeChildren::loaded(&self.children[id])
    }

    fn revision(&self) -> TreeRevision {
        TreeRevision::INITIAL
    }

    fn size_hint(&self) -> usize {
        self.children.len()
    }
}

struct WideIdTree {
    children: Vec<Vec<u128>>,
}

impl WideIdTree {
    fn balanced(node_count: usize, fanout: usize) -> Self {
        let tree = BenchTree::balanced(node_count, fanout);
        Self {
            children: tree
                .children
                .into_iter()
                .map(|children| children.into_iter().map(|id| id as u128).collect())
                .collect(),
        }
    }
}

impl TreeModel for WideIdTree {
    type Id = u128;

    fn roots(&self) -> impl Iterator<Item = Self::Id> + '_ {
        std::iter::once(0)
    }

    fn children(&self, id: Self::Id) -> TreeChildren<'_, Self::Id> {
        TreeChildren::loaded(&self.children[id as usize])
    }

    fn revision(&self) -> TreeRevision {
        TreeRevision::INITIAL
    }

    fn size_hint(&self) -> usize {
        self.children.len()
    }
}

struct Label;

impl TreeLabelRenderer<BenchTree> for Label {
    fn cell<'a>(
        &'a self,
        _model: &'a BenchTree,
        _id: usize,
        _context: &TreeRowContext<'_>,
        _glyphs: &TreeGlyphs<'a>,
    ) -> Cell<'a> {
        Cell::from("node")
    }
}

struct RenderFixture<'a> {
    model: &'a BenchTree,
    query: &'a TreeQuery,
    label: &'a Label,
    columns: &'a TreeColumnSet<'a, BenchTree>,
    style: &'a TreeListViewStyle<'a>,
    area: Rect,
}

impl RenderFixture<'_> {
    fn render(&self, buffer: &mut Buffer, state: &mut TreeListViewState<usize>) {
        TreeListView::new(
            self.model,
            self.query,
            self.label,
            self.columns,
            self.style.clone(),
        )
        .render(self.area, buffer, state);
        black_box(buffer);
    }
}

fn data_cell<'a>(_model: &'a BenchTree, _id: usize, _context: &TreeRowContext<'_>) -> Cell<'a> {
    Cell::from("data")
}

fn columns(count: usize) -> TreeColumnSet<'static, BenchTree> {
    let mut columns = Vec::with_capacity(count);
    let tree_width = if count == 1 {
        ColumnWidth::flexible(12, 48).expect("valid static column width")
    } else {
        ColumnWidth::fixed(24)
    };
    columns.push(ColumnDef::tree("Name", tree_width));
    columns.extend(
        (1..count).map(|index| {
            ColumnDef::data(format!("Data {index}"), ColumnWidth::fixed(24), data_cell)
        }),
    );
    TreeColumnSet::new(columns).expect("one tree column")
}

const fn no_matches(_: &BenchTree, _: usize) -> bool {
    false
}

const fn sparse_filter(_: &BenchTree, id: usize) -> bool {
    id.is_multiple_of(17)
}

const fn all_matches(_: &BenchTree, _: usize) -> bool {
    true
}

fn descending(_: &BenchTree, left: usize, right: usize) -> std::cmp::Ordering {
    right.cmp(&left)
}

fn expanded_state<F, S>(model: &BenchTree, query: &TreeQuery<F, S>) -> TreeListViewState<usize>
where
    F: TreeFilter<BenchTree>,
    S: TreeSort<BenchTree>,
{
    let mut state = TreeListViewState::with_capacity(model.size_hint());
    let _ = state.expand_all(model);
    let _ = state.ensure_projection(model, query);
    let _ = state.select_first();
    state
}

fn bench_projection_rebuild(c: &mut Criterion) {
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

fn bench_projection_cache(c: &mut Criterion) {
    let model = BenchTree::balanced(20_000, 4);
    let query = TreeQuery::new();
    let mut state = expanded_state(&model, &query);
    let mut cache = c.benchmark_group("projection/cache_hit");
    cache.throughput(Throughput::Elements(1));
    cache.bench_function("balanced/20000", |b| {
        b.iter(|| {
            black_box(state.ensure_projection(black_box(&model), black_box(&query)));
        });
    });
    cache.finish();
}

fn bench_projection_id_width(c: &mut Criterion) {
    let mut ids = c.benchmark_group("projection/id_width");
    ids.throughput(Throughput::Elements(20_000));

    {
        let model = BenchTree::balanced(20_000, 4);
        let mut query = TreeQuery::<NoFilter, NoSort>::new();
        let mut state = expanded_state(&model, &query);
        ids.bench_function("usize/20000", |b| {
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
        ids.bench_function("u128/20000", |b| {
            b.iter(|| {
                query.touch_sort();
                let rebuilt = state.ensure_projection(black_box(&model), black_box(&query));
                black_box((rebuilt, state.visible_len()));
            });
        });
    }

    ids.finish();
}

fn bench_projection_filter(c: &mut Criterion) {
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
    let mut filter_group = c.benchmark_group("projection/filter");
    filter_group.throughput(Throughput::Elements(20_000));

    for (name, filter, config) in cases {
        let mut query = TreeQuery::new().with_filter(filter, config, TreeRevision::INITIAL);
        let mut state = TreeListViewState::with_capacity(model.size_hint());
        filter_group.bench_function(BenchmarkId::new(name, 20_000), |b| {
            b.iter(|| {
                query.touch_filter();
                let rebuilt = state.ensure_projection(black_box(&model), black_box(&query));
                black_box((rebuilt, state.visible_len()));
            });
        });
    }
    filter_group.finish();
}

fn bench_projection_sort(c: &mut Criterion) {
    let model = BenchTree::balanced(20_000, 32);
    let mut query = TreeQuery::new().with_sort(descending, TreeRevision::INITIAL);
    let mut state = expanded_state(&model, &query);
    let mut sort_group = c.benchmark_group("projection/sort");
    sort_group.throughput(Throughput::Elements(20_000));
    sort_group.bench_function("wide/20000", |b| {
        b.iter(|| {
            query.touch_sort();
            let rebuilt = state.ensure_projection(black_box(&model), black_box(&query));
            black_box((rebuilt, state.visible_len()));
        });
    });
    sort_group.finish();
}

fn bench_marks_cache(c: &mut Criterion) {
    let model = BenchTree::balanced(20_000, 4);
    let mut state = TreeListViewState::with_capacity(model.size_hint());
    state.ensure_mark_states(&model);
    let mut cache = c.benchmark_group("marks/cache_hit");
    cache.throughput(Throughput::Elements(1));
    cache.bench_function("balanced/20000", |b| {
        b.iter(|| {
            state.ensure_mark_states(black_box(&model));
            black_box(state.mark_state(0));
        });
    });
    cache.finish();
}

fn bench_marks_rebuild(c: &mut Criterion) {
    let mut rebuild = c.benchmark_group("marks/rebuild");
    for size in [5_000usize, 20_000, 100_000] {
        let model = BenchTree::balanced(size, 4);
        let mut state = TreeListViewState::with_capacity(model.size_hint());
        rebuild.throughput(Throughput::Elements(size as u64));
        rebuild.bench_with_input(BenchmarkId::new("single_leaf", size), &size, |b, _| {
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
    rebuild.throughput(Throughput::Elements(20_000));
    rebuild.bench_function(BenchmarkId::new("half_leaves", 20_000), |b| {
        b.iter(|| {
            let _ = state.toggle_marked(toggled);
            state.ensure_mark_states(black_box(&model));
            black_box(state.mark_state(0));
        });
    });
    rebuild.finish();
}

fn bench_interaction(c: &mut Criterion) {
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

fn bench_adapters(c: &mut Criterion) {
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

fn bench_render_balanced(c: &mut Criterion) {
    let area = Rect::new(0, 0, 100, 25);
    let label = Label;
    let one_column = columns(1);
    let mut group = c.benchmark_group("render/widget_only");

    for size in [5_000usize, 20_000] {
        let model = BenchTree::balanced(size, 4);
        let query = TreeQuery::new();
        let mut state = expanded_state(&model, &query);
        let style = TreeListViewStyle {
            row_rendering: TreeRowRendering::Full,
            ..TreeListViewStyle::default()
        };
        let fixture = RenderFixture {
            model: &model,
            query: &query,
            label: &label,
            columns: &one_column,
            style: &style,
            area,
        };
        group.throughput(Throughput::Elements(size as u64));
        group.bench_with_input(BenchmarkId::new("full/balanced", size), &size, |b, _| {
            b.iter_batched_ref(
                || Buffer::empty(area),
                |buffer| fixture.render(buffer, &mut state),
                BatchSize::LargeInput,
            );
        });
    }

    for size in [20_000usize, 100_000] {
        let model = BenchTree::balanced(size, 4);
        let query = TreeQuery::new();
        let mut state = expanded_state(&model, &query);
        let _ = state.set_offset(size / 2);
        let style = TreeListViewStyle::default();
        let fixture = RenderFixture {
            model: &model,
            query: &query,
            label: &label,
            columns: &one_column,
            style: &style,
            area,
        };
        group.throughput(Throughput::Elements(u64::from(area.height)));
        group.bench_with_input(
            BenchmarkId::new("virtualized/balanced", size),
            &size,
            |b, _| {
                b.iter_batched_ref(
                    || Buffer::empty(area),
                    |buffer| fixture.render(buffer, &mut state),
                    BatchSize::LargeInput,
                );
            },
        );
    }
    group.finish();
}

fn bench_render_deep(c: &mut Criterion) {
    let area = Rect::new(0, 0, 100, 25);
    let label = Label;
    let one_column = columns(1);
    let model = BenchTree::chain(20_000);
    let query = TreeQuery::new();
    let mut state = expanded_state(&model, &query);
    let _ = state.set_offset(usize::MAX);
    let style = TreeListViewStyle::default();
    let fixture = RenderFixture {
        model: &model,
        query: &query,
        label: &label,
        columns: &one_column,
        style: &style,
        area,
    };
    let mut group = c.benchmark_group("render/widget_only");
    group.throughput(Throughput::Elements(20_000));
    group.bench_function("virtualized/deep_chain_end/20000", |b| {
        b.iter_batched_ref(
            || Buffer::empty(area),
            |buffer| fixture.render(buffer, &mut state),
            BatchSize::LargeInput,
        );
    });
    group.finish();
}

fn bench_render_horizontal(c: &mut Criterion) {
    let area = Rect::new(0, 0, 80, 25);
    let label = Label;
    let model = BenchTree::balanced(20_000, 4);
    let query = TreeQuery::new();
    let many_columns = columns(8);
    let mut state = expanded_state(&model, &query);
    let _ = state.set_horizontal_offset(u16::MAX);
    let style = TreeListViewStyle::default();
    let fixture = RenderFixture {
        model: &model,
        query: &query,
        label: &label,
        columns: &many_columns,
        style: &style,
        area,
    };
    let mut warm_buffer = Buffer::empty(area);
    fixture.render(&mut warm_buffer, &mut state);

    let mut group = c.benchmark_group("render/widget_only");
    group.throughput(Throughput::Elements(u64::from(area.height) * 8));
    group.bench_function("horizontal/end/20000", |b| {
        b.iter_batched_ref(
            || Buffer::empty(area),
            |buffer| fixture.render(buffer, &mut state),
            BatchSize::LargeInput,
        );
    });
    group.finish();
}

fn bench_render_end_to_end(c: &mut Criterion) {
    let area = Rect::new(0, 0, 100, 25);
    let label = Label;
    let one_column = columns(1);
    let model = BenchTree::balanced(20_000, 4);
    let query = TreeQuery::new();
    let mut state = expanded_state(&model, &query);
    let style = TreeListViewStyle::default();
    let fixture = RenderFixture {
        model: &model,
        query: &query,
        label: &label,
        columns: &one_column,
        style: &style,
        area,
    };
    let mut buffer = Buffer::empty(area);
    let mut end_to_end = c.benchmark_group("render/end_to_end");
    end_to_end.throughput(Throughput::Elements(u64::from(area.height)));
    end_to_end.bench_function("virtualized/balanced/20000", |b| {
        b.iter(|| {
            buffer.reset();
            fixture.render(&mut buffer, &mut state);
        });
    });
    end_to_end.finish();
}

criterion_group!(
    benches,
    bench_projection_rebuild,
    bench_projection_cache,
    bench_projection_id_width,
    bench_projection_filter,
    bench_projection_sort,
    bench_marks_cache,
    bench_marks_rebuild,
    bench_interaction,
    bench_adapters,
    bench_render_balanced,
    bench_render_deep,
    bench_render_horizontal,
    bench_render_end_to_end,
);
criterion_main!(benches);
