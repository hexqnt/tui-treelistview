use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use ratatui::layout::Rect;
use ratatui::prelude::Buffer;
use ratatui::widgets::{Cell, StatefulWidget};

use tui_treelistview::{
    ColumnDef, ColumnWidth, NoFilter, NoSort, TreeChildren, TreeColumnSet, TreeFilter,
    TreeFilterConfig, TreeGlyphs, TreeLabelRenderer, TreeListView, TreeListViewState,
    TreeListViewStyle, TreeModel, TreeQuery, TreeRevision, TreeRowContext, TreeRowRendering,
    TreeSort,
};

struct BenchTree {
    children: Vec<Vec<usize>>,
}

impl BenchTree {
    fn generate(node_count: usize, fanout: usize) -> Self {
        let mut children = vec![Vec::new(); node_count.max(1)];
        let mut next_id = 1usize;

        for parent in 0..children.len() {
            for _ in 0..fanout {
                if next_id >= children.len() {
                    break;
                }
                children[parent].push(next_id);
                next_id += 1;
            }
            if next_id >= children.len() {
                break;
            }
        }

        Self { children }
    }
}

impl TreeModel for BenchTree {
    type Id = usize;

    fn roots(&self) -> impl Iterator<Item = Self::Id> + '_ {
        std::iter::once(0)
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

fn columns() -> TreeColumnSet<'static, BenchTree> {
    TreeColumnSet::new([ColumnDef::tree(
        "Name",
        ColumnWidth::flexible(12, 48).expect("valid static column width"),
    )])
    .expect("one tree column")
}

const fn sparse_filter(_: &BenchTree, id: usize) -> bool {
    id.is_multiple_of(17)
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

fn bench_projection(c: &mut Criterion) {
    let mut group = c.benchmark_group("projection/rebuild");

    for size in [5_000usize, 10_000, 20_000] {
        let model = BenchTree::generate(size, 4);
        let mut query = TreeQuery::<NoFilter, NoSort>::new();
        let mut state = expanded_state(&model, &query);

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                query.touch_sort();
                let _ = state.ensure_projection(black_box(&model), black_box(&query));
                black_box(state.visible_len());
            });
        });
    }

    group.finish();
}

fn bench_filtered_projection(c: &mut Criterion) {
    let mut group = c.benchmark_group("projection/filter");

    for size in [5_000usize, 10_000, 20_000] {
        let model = BenchTree::generate(size, 4);

        for (name, config) in [
            ("auto_expand", TreeFilterConfig::enabled()),
            ("manual_expand", TreeFilterConfig::enabled_manual_expand()),
        ] {
            let mut query =
                TreeQuery::new().with_filter(sparse_filter, config, TreeRevision::INITIAL);
            let mut state = TreeListViewState::with_capacity(model.size_hint());

            group.bench_with_input(BenchmarkId::new(name, size), &size, |b, _| {
                b.iter(|| {
                    query.touch_filter();
                    let _ = state.ensure_projection(black_box(&model), black_box(&query));
                    black_box(state.visible_len());
                });
            });
        }
    }

    group.finish();
}

fn bench_mark_states(c: &mut Criterion) {
    let mut group = c.benchmark_group("marks/rebuild_after_toggle");

    for size in [5_000usize, 10_000, 20_000] {
        let model = BenchTree::generate(size, 4);
        let mut state = TreeListViewState::with_capacity(model.size_hint());

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                let _ = state.toggle_marked(size - 1);
                state.ensure_mark_states(black_box(&model));
                black_box(state.mark_state(0));
            });
        });
    }

    group.finish();
}

fn bench_render(c: &mut Criterion) {
    let mut group = c.benchmark_group("render/rows");

    for size in [5_000usize, 10_000, 20_000] {
        let model = BenchTree::generate(size, 4);
        let query = TreeQuery::new();
        let label = Label;
        let columns = columns();
        let area = Rect::new(0, 0, 100, 25);

        for rendering in [TreeRowRendering::Full, TreeRowRendering::Virtualized] {
            let rendering_name = match rendering {
                TreeRowRendering::Full => "full",
                TreeRowRendering::Virtualized => "virtualized",
            };
            for (offset_name, offset) in [
                ("start", 0),
                ("middle", size / 2),
                ("end", size.saturating_sub(1)),
            ] {
                let mut state = expanded_state(&model, &query);
                let _ = state.set_offset(offset);
                let mut buffer = Buffer::empty(area);
                let style = TreeListViewStyle {
                    row_rendering: rendering,
                    ..TreeListViewStyle::default()
                };

                group.bench_with_input(
                    BenchmarkId::new(format!("{rendering_name}/{offset_name}"), size),
                    &size,
                    |b, _| {
                        b.iter(|| {
                            buffer.reset();
                            TreeListView::new(
                                black_box(&model),
                                black_box(&query),
                                &label,
                                &columns,
                                style.clone(),
                            )
                            .render(area, &mut buffer, &mut state);
                            black_box(state.visible_len());
                        });
                    },
                );
            }
        }
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_projection,
    bench_filtered_projection,
    bench_mark_states,
    bench_render,
);
criterion_main!(benches);
