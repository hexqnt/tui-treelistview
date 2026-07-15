use std::hint::black_box;

use criterion::{BatchSize, BenchmarkId, Criterion, Throughput};
use ratatui::layout::Rect;
use ratatui::prelude::Buffer;
use ratatui::widgets::StatefulWidget;
use tui_treelistview::{
    TreeColumnSet, TreeListView, TreeListViewState, TreeListViewStyle, TreeQuery, TreeRowRendering,
};

use super::fixture::{BenchTree, Label, columns, expanded_state};

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

pub fn balanced(c: &mut Criterion) {
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

pub fn deep(c: &mut Criterion) {
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

pub fn horizontal(c: &mut Criterion) {
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

pub fn end_to_end(c: &mut Criterion) {
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
    let mut group = c.benchmark_group("render/end_to_end");
    group.throughput(Throughput::Elements(u64::from(area.height)));
    group.bench_function("virtualized/balanced/20000", |b| {
        b.iter(|| {
            buffer.reset();
            fixture.render(&mut buffer, &mut state);
        });
    });
    group.finish();
}
