use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::widgets::{Cell, StatefulWidget};
use tui_treelistview::{
    ColumnDef, ColumnWidth, TreeChildren, TreeColumnSet, TreeHit, TreeHorizontalScroll,
    TreeLabelPrefix, TreeLabelProvider, TreeListView, TreeListViewState, TreeListViewStyle,
    TreeModel, TreeQuery, TreeRevision, TreeRowContext, TreeRowRendering,
};

#[derive(Clone)]
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

    fn nested() -> Self {
        Self {
            children: vec![
                vec![1, 2],
                vec![3, 4],
                vec![5],
                vec![6],
                vec![],
                vec![7, 8],
                vec![],
                vec![],
                vec![],
            ],
            names: (0..9).map(|id| format!("node {id}")).collect(),
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

struct Fixture {
    model: Model,
    query: TreeQuery,
    columns: TreeColumnSet<'static, Model>,
    state: TreeListViewState<usize>,
}

impl Fixture {
    fn new(model: Model, show_header: bool) -> Self {
        let query = TreeQuery::new();
        let mut state = TreeListViewState::new();
        let _ = state.expand_all(&model);
        let _ = state.ensure_projection(&model, &query);
        Self {
            model,
            query,
            columns: columns(show_header),
            state,
        }
    }

    fn render(&mut self, area: Rect, style: TreeListViewStyle<'_>) -> Buffer {
        let mut buffer = Buffer::empty(area);
        TreeListView::new(&self.model, &self.query, &Label, &self.columns, style).render(
            area,
            &mut buffer,
            &mut self.state,
        );
        buffer
    }
}

#[test]
fn virtualized_and_full_rendering_are_identical_at_every_viewport_position() {
    for model in [Model::sample(), Model::nested()] {
        for show_header in [false, true] {
            let mut full = Fixture::new(model.clone(), show_header);
            let mut virtualized = Fixture::new(model.clone(), show_header);
            for area in [Rect::new(0, 0, 20, 4), Rect::new(4, 2, 40, 2)] {
                for offset in 0..model.children.len() {
                    let _ = full.state.set_offset(offset);
                    let _ = virtualized.state.set_offset(offset);
                    assert_eq!(
                        full.render(
                            area,
                            TreeListViewStyle {
                                row_rendering: TreeRowRendering::Full,
                                ..TreeListViewStyle::borderless()
                            },
                        ),
                        virtualized.render(area, TreeListViewStyle::borderless()),
                        "rendering differs at offset {offset}, header {show_header}, area {area:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn hit_testing_reports_headers_rows_columns_and_scrollbars() {
    let mut fixture = Fixture::new(Model::sample(), true);
    let area = Rect::new(4, 2, 22, 4);
    let _ = fixture.render(area, TreeListViewStyle::borderless());
    let state = &fixture.state;

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
    let mut fixture = Fixture::new(Model::sample(), false);
    let _ = fixture.state.set_offset(usize::MAX);
    let area = Rect::new(0, 0, 20, 4);
    let buffer = fixture.render(area, TreeListViewStyle::borderless());

    assert_eq!(fixture.state.offset(), 3);
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
    let mut fixture = Fixture::new(Model::sample(), false);
    let _ = fixture.state.set_offset(usize::MAX);
    let area = Rect::new(0, 0, 20, 4);
    let buffer = fixture.render(
        area,
        TreeListViewStyle {
            horizontal_scroll: TreeHorizontalScroll::Disabled,
            ..TreeListViewStyle::borderless()
        },
    );

    assert_eq!(fixture.state.offset(), 2);
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
    let mut fixture = Fixture::new(Model::sample(), false);
    let _ = fixture.state.collapse_all();
    let _ = fixture.state.set_horizontal_offset(u16::MAX);
    let area = Rect::new(0, 0, 12, 8);
    let buffer = fixture.render(area, TreeListViewStyle::borderless());

    assert_eq!(fixture.state.horizontal_offset(), 16);
    assert_eq!(
        buffer.cell((10, 7)).map(ratatui::buffer::Cell::symbol),
        Some("█")
    );
    assert_eq!(
        buffer.cell((11, 7)).map(ratatui::buffer::Cell::symbol),
        Some("►")
    );
}
