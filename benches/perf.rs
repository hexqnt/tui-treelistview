use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};
use ratatui::layout::{Constraint, Rect};
use ratatui::prelude::Buffer;
use ratatui::widgets::{Cell, StatefulWidget};
use smallvec::SmallVec;

use tui_treelistview::{
    TreeAction, TreeColumns, TreeFilterConfig, TreeGlyphs, TreeLabelRenderer, TreeListView,
    TreeListViewState, TreeListViewStyle, TreeModel, TreeRowContext,
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

    fn root(&self) -> Option<Self::Id> {
        Some(0)
    }

    fn children(&self, id: Self::Id) -> &[Self::Id] {
        &self.children[id]
    }

    fn contains(&self, id: Self::Id) -> bool {
        id < self.children.len()
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
        _ctx: &TreeRowContext,
        _glyphs: &TreeGlyphs<'a>,
    ) -> Cell<'a> {
        Cell::from("node")
    }
}

struct Columns;

impl TreeColumns<BenchTree> for Columns {
    fn label_constraint(&self) -> Constraint {
        Constraint::Fill(1)
    }

    fn other_constraints(&self) -> &[Constraint] {
        &[]
    }

    fn cells<'a>(&'a self, _model: &'a BenchTree, _id: usize) -> SmallVec<[Cell<'a>; 8]> {
        SmallVec::new()
    }
}

const fn sparse_filter(_: &BenchTree, id: usize) -> bool {
    id.is_multiple_of(17)
}

fn expanded_state(model: &BenchTree) -> TreeListViewState<usize> {
    let mut state = TreeListViewState::with_capacity(model.size_hint());
    state.expand_all(model);
    state.ensure_visible_nodes(model);
    state.select_first();
    state
}

fn bench_ensure_visible_nodes(c: &mut Criterion) {
    let mut group = c.benchmark_group("ensure_visible_nodes");

    for size in [5_000usize, 10_000, 20_000] {
        let model = BenchTree::generate(size, 4);
        let mut state = expanded_state(&model);

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                state.invalidate();
                state.ensure_visible_nodes(black_box(&model));
                black_box(state.visible_len());
            });
        });
    }

    group.finish();
}

fn bench_ensure_visible_nodes_filtered(c: &mut Criterion) {
    let mut group = c.benchmark_group("ensure_visible_nodes_filtered");

    for size in [5_000usize, 10_000, 20_000] {
        let model = BenchTree::generate(size, 4);
        let mut state = TreeListViewState::with_capacity(model.size_hint());

        for auto_expand in [false, true] {
            let config = TreeFilterConfig::Enabled { auto_expand };
            let bench_name = if auto_expand {
                format!("{size}/auto_expand")
            } else {
                format!("{size}/manual_expand")
            };

            group.bench_with_input(BenchmarkId::from_parameter(bench_name), &size, |b, _| {
                b.iter(|| {
                    state.invalidate();
                    state.ensure_visible_nodes_filtered(black_box(&model), &sparse_filter, config);
                    black_box(state.visible_len());
                });
            });
        }
    }

    group.finish();
}

fn bench_ensure_mark_cache(c: &mut Criterion) {
    let mut group = c.benchmark_group("ensure_mark_cache_after_toggle");

    for size in [5_000usize, 10_000, 20_000] {
        let model = BenchTree::generate(size, 4);
        let mut state = expanded_state(&model);

        group.bench_with_input(BenchmarkId::from_parameter(size), &size, |b, _| {
            b.iter(|| {
                let _ = state.handle_action(black_box(&model), TreeAction::<()>::ToggleMark);
                state.ensure_mark_cache(black_box(&model));
                black_box(state.node_is_marked(0));
            });
        });
    }

    group.finish();
}

fn bench_render_row_build(c: &mut Criterion) {
    let mut group = c.benchmark_group("render_row_build");

    for size in [5_000usize, 10_000, 20_000] {
        let model = BenchTree::generate(size, 4);
        let label = Label;
        let columns = Columns;
        let mut state = expanded_state(&model);
        let area = Rect::new(0, 0, 100, 25);
        let mut buffer = Buffer::empty(area);

        for virtualize_rows in [false, true] {
            let style = TreeListViewStyle {
                virtualize_rows,
                ..TreeListViewStyle::default()
            };
            let bench_name = if virtualize_rows {
                format!("{size}/virtualized")
            } else {
                format!("{size}/full")
            };

            group.bench_with_input(BenchmarkId::from_parameter(bench_name), &size, |b, _| {
                b.iter(|| {
                    let widget = TreeListView::new(&model, &label, &columns, style.clone());
                    widget.render(area, &mut buffer, &mut state);
                    black_box(state.visible_len());
                });
            });
        }
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_ensure_visible_nodes,
    bench_ensure_visible_nodes_filtered,
    bench_ensure_mark_cache,
    bench_render_row_build,
);
criterion_main!(benches);
