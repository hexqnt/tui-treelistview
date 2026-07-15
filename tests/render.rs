use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::widgets::{Cell, StatefulWidget};
use tui_treelistview::{
    ColumnDef, ColumnWidth, TreeChildren, TreeColumnSet, TreeHit, TreeHorizontalScroll,
    TreeLabelPrefix, TreeLabelProvider, TreeListView, TreeListViewState, TreeListViewStyle,
    TreeModel, TreeQuery, TreeRevision, TreeRowContext, TreeRowRendering,
};

struct Model {
    children: Vec<Vec<usize>>,
    names: Vec<String>,
}

impl Model {
    fn sample() -> Self {
        Self {
            children: vec![vec![1, 2, 3, 4, 5], vec![], vec![], vec![], vec![], vec![]],
            names: ["root", "alpha", "beta", "gamma", "delta", "epsilon"]
                .map(str::to_owned)
                .into(),
        }
    }
}

impl TreeModel for Model {
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

impl TreeLabelProvider<Model> for Label {
    fn label_parts<'a>(&'a self, model: &'a Model, id: usize) -> TreeLabelPrefix<'a> {
        TreeLabelPrefix::borrowed(&model.names[id])
    }
}

fn columns(show_header: bool) -> TreeColumnSet<'static, Model> {
    let separator = String::from(":");
    let columns = TreeColumnSet::new([
        ColumnDef::data_owned(
            "Detail",
            ColumnWidth::fixed(12),
            move |model: &Model, id: usize, context: &TreeRowContext<'_>| {
                Cell::from(format!("{}{separator}{}", context.level, model.names[id]))
            },
        ),
        ColumnDef::tree("Name", ColumnWidth::fixed(12)),
    ])
    .expect("one tree column");
    if show_header {
        columns
    } else {
        columns.without_header()
    }
}

fn rendered(rendering: TreeRowRendering, offset: usize) -> Buffer {
    let model = Model::sample();
    let query = TreeQuery::new();
    let columns = columns(false);
    let label = Label;
    let mut state = TreeListViewState::new();
    let _ = state.expand_all(&model);
    let _ = state.ensure_projection(&model, &query);
    let _ = state.set_offset(offset);
    let area = Rect::new(0, 0, 20, 4);
    let mut buffer = Buffer::empty(area);
    TreeListView::new(
        &model,
        &query,
        &label,
        &columns,
        TreeListViewStyle {
            row_rendering: rendering,
            ..TreeListViewStyle::borderless()
        },
    )
    .render(area, &mut buffer, &mut state);
    buffer
}

#[test]
fn virtualized_and_full_rendering_are_identical_at_every_viewport_position() {
    for offset in [0, 2, 5] {
        assert_eq!(
            rendered(TreeRowRendering::Virtualized, offset),
            rendered(TreeRowRendering::Full, offset),
            "rendering differs at offset {offset}"
        );
    }
}

#[test]
fn hit_testing_reports_headers_rows_columns_and_scrollbars() {
    let model = Model::sample();
    let query = TreeQuery::new();
    let columns = columns(true);
    let label = Label;
    let mut state = TreeListViewState::new();
    let _ = state.expand_all(&model);
    let area = Rect::new(4, 2, 22, 4);
    let mut buffer = Buffer::empty(area);
    TreeListView::new(
        &model,
        &query,
        &label,
        &columns,
        TreeListViewStyle::borderless(),
    )
    .render(area, &mut buffer, &mut state);

    assert_eq!(
        state.hit_test(Position::new(7, 2)),
        Some(TreeHit::Header { column: Some(0) })
    );
    assert_eq!(
        state.hit_test(Position::new(20, 3)),
        Some(TreeHit::Row {
            id: 0,
            index: 0,
            column: Some(1),
        })
    );
    assert_eq!(
        state.hit_test(Position::new(25, 2)),
        Some(TreeHit::VerticalScrollbar)
    );
    assert_eq!(
        state.hit_test(Position::new(4, 5)),
        Some(TreeHit::HorizontalScrollbar)
    );
    assert_eq!(state.hit_test(Position::new(3, 2)), None);
}

#[test]
fn rendering_clamps_the_offset_to_the_last_full_viewport() {
    let model = Model::sample();
    let query = TreeQuery::new();
    let columns = columns(false);
    let label = Label;
    let mut state = TreeListViewState::new();
    let _ = state.expand_all(&model);
    let _ = state.ensure_projection(&model, &query);
    let _ = state.set_offset(usize::MAX);
    let area = Rect::new(0, 0, 20, 4);
    let mut buffer = Buffer::empty(area);
    TreeListView::new(
        &model,
        &query,
        &label,
        &columns,
        TreeListViewStyle::borderless(),
    )
    .render(area, &mut buffer, &mut state);

    assert_eq!(state.offset(), 3);
    assert_eq!(
        buffer.cell((3, 0)).map(ratatui::buffer::Cell::symbol),
        Some("1")
    );
    assert_eq!(
        buffer.cell((16, 0)).map(ratatui::buffer::Cell::symbol),
        Some("├")
    );
}

#[test]
fn vertical_scrollbar_reaches_the_end_at_the_last_viewport() {
    let model = Model::sample();
    let query = TreeQuery::new();
    let columns = columns(false);
    let label = Label;
    let mut state = TreeListViewState::new();
    let _ = state.expand_all(&model);
    let _ = state.ensure_projection(&model, &query);
    let _ = state.set_offset(usize::MAX);
    let area = Rect::new(0, 0, 20, 4);
    let mut buffer = Buffer::empty(area);
    TreeListView::new(
        &model,
        &query,
        &label,
        &columns,
        TreeListViewStyle {
            horizontal_scroll: TreeHorizontalScroll::Disabled,
            ..TreeListViewStyle::borderless()
        },
    )
    .render(area, &mut buffer, &mut state);

    assert_eq!(state.offset(), 2);
    assert_eq!(
        buffer.cell((19, 1)).map(ratatui::buffer::Cell::symbol),
        Some("║")
    );
    assert_eq!(
        buffer.cell((19, 2)).map(ratatui::buffer::Cell::symbol),
        Some("█")
    );
}

#[test]
fn horizontal_scrollbar_reaches_the_end_at_the_maximum_offset() {
    let model = Model::sample();
    let query = TreeQuery::new();
    let columns = columns(false);
    let label = Label;
    let mut state = TreeListViewState::new();
    let _ = state.set_horizontal_offset(u16::MAX);
    let area = Rect::new(0, 0, 12, 8);
    let mut buffer = Buffer::empty(area);
    TreeListView::new(
        &model,
        &query,
        &label,
        &columns,
        TreeListViewStyle::borderless(),
    )
    .render(area, &mut buffer, &mut state);

    assert_eq!(state.horizontal_offset(), 16);
    assert_eq!(
        buffer.cell((10, 7)).map(ratatui::buffer::Cell::symbol),
        Some("█")
    );
    assert_eq!(
        buffer.cell((11, 7)).map(ratatui::buffer::Cell::symbol),
        Some("►")
    );
}
