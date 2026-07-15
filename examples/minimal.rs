use ratatui::layout::Rect;
use ratatui::prelude::Buffer;
use ratatui::widgets::StatefulWidget;

use tui_treelistview::{
    ColumnDef, ColumnWidth, TreeChildren, TreeColumnSet, TreeGlyphs, TreeLabelRenderer,
    TreeListView, TreeListViewState, TreeListViewStyle, TreeModel, TreeQuery, TreeRevision,
    TreeRowContext,
};

struct Model {
    children: Vec<Vec<usize>>,
    names: Vec<String>,
}

impl Model {
    fn new() -> Self {
        Self {
            children: vec![vec![1, 2], vec![], vec![]],
            names: vec!["root".into(), "alpha".into(), "beta".into()],
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

impl TreeLabelRenderer<Model> for Label {
    fn cell<'a>(
        &'a self,
        model: &'a Model,
        id: usize,
        context: &TreeRowContext<'_>,
        glyphs: &TreeGlyphs<'a>,
    ) -> ratatui::widgets::Cell<'a> {
        tui_treelistview::tree_name_cell(
            context,
            tui_treelistview::TreeLabelPrefix::borrowed(&model.names[id]),
            glyphs,
        )
    }
}

fn main() {
    let model = Model::new();
    let query = TreeQuery::new();
    let label = Label;
    let columns = TreeColumnSet::new([ColumnDef::tree(
        "Name",
        ColumnWidth::flexible(8, 24).expect("valid static column width"),
    )])
    .expect("one tree column");
    let mut state = TreeListViewState::new();
    state.set_expanded(0, None, true);

    let widget = TreeListView::new(
        &model,
        &query,
        &label,
        &columns,
        TreeListViewStyle::default(),
    );
    let area = Rect::new(0, 0, 40, 8);
    let mut buffer = Buffer::empty(area);
    widget.render(area, &mut buffer, &mut state);
}
