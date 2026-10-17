use std::hint::black_box;

use criterion::{BenchmarkId, Criterion, Throughput};
use ratatui::style::Style;
use tui_treelistview::{
    TreeExpansionState, TreeGlyphs, TreeLabelPrefix, TreeMarkState, TreeMatchState, TreeRowContext,
    TreeRowNodeState, TreeRowRenderState, tree_label_line,
};

pub fn labels(c: &mut Criterion) {
    let glyphs = TreeGlyphs::unicode();
    let mut group = c.benchmark_group("render/glyphs");
    group.throughput(Throughput::Elements(1));
    for level in [0, 4, 16, 128] {
        let tails: Vec<_> = (0..level).map(|index| index % 3 == 0).collect();
        let context = TreeRowContext {
            level,
            is_tail_stack: &tails,
            node: TreeRowNodeState {
                expansion: TreeExpansionState::Expanded,
                mark: TreeMarkState::Unmarked,
                match_state: TreeMatchState::Unfiltered,
            },
            render: TreeRowRenderState {
                draw_lines: true,
                is_selected: false,
                selected_column: None,
            },
            line_style: Style::default(),
        };
        group.bench_with_input(BenchmarkId::new("label", level), &context, |b, context| {
            b.iter(|| {
                tree_label_line(
                    black_box(context),
                    TreeLabelPrefix {
                        name: "node".into(),
                        prefix: Some("icon".into()),
                    },
                    black_box(&glyphs),
                )
            });
        });
    }
    group.finish();
}
