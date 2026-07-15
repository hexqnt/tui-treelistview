use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::prelude::Buffer;
use ratatui::widgets::StatefulWidget;

use tui_treelistview::{
    ColumnDef, ColumnWidth, TreeAction, TreeChildren, TreeColumnSet, TreeLabelPrefix,
    TreeLabelProvider, TreeListView, TreeListViewState, TreeListViewStyle, TreeModel, TreeQuery,
    TreeRevision, TreeViewAction,
};

struct Model {
    children: Vec<Vec<usize>>,
    names: Vec<String>,
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
}

struct Label;

impl TreeLabelProvider<Model> for Label {
    fn label_parts<'a>(&'a self, model: &'a Model, id: usize) -> TreeLabelPrefix<'a> {
        TreeLabelPrefix::borrowed(&model.names[id])
    }
}

const fn map_key(event: KeyEvent) -> Option<TreeAction> {
    let action = match (event.code, event.modifiers) {
        (KeyCode::Char('w'), KeyModifiers::NONE) => TreeViewAction::SelectPrev,
        (KeyCode::Char('s'), KeyModifiers::NONE) => TreeViewAction::SelectNext,
        (KeyCode::Char('a'), KeyModifiers::NONE) => TreeViewAction::CollapseOrSelectParent,
        (KeyCode::Char('d'), KeyModifiers::NONE) => TreeViewAction::ExpandOrSelectFirstChild,
        (KeyCode::Char('x'), KeyModifiers::NONE) => TreeViewAction::ToggleNode,
        (KeyCode::Char('m'), KeyModifiers::NONE) => TreeViewAction::ToggleMark,
        _ => return None,
    };
    Some(TreeAction::View(action))
}

fn main() {
    let model = Model {
        children: vec![vec![1, 2, 3], vec![], vec![], vec![]],
        names: vec!["root".into(), "alpha".into(), "beta".into(), "gamma".into()],
    };
    let query = TreeQuery::new();
    let label = Label;
    let columns = TreeColumnSet::new([ColumnDef::tree(
        "Name",
        ColumnWidth::flexible(8, 24).expect("valid static column width"),
    )])
    .expect("one tree column");
    let mut state = TreeListViewState::new();
    state.set_expanded(0, None, true);

    for key in [
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE),
    ] {
        if let Some(action) = map_key(key) {
            let _ = state.handle_action(&model, &query, &columns, action);
        }
    }

    let area = Rect::new(0, 0, 40, 8);
    let mut buffer = Buffer::empty(area);
    TreeListView::new(
        &model,
        &query,
        &label,
        &columns,
        TreeListViewStyle::default(),
    )
    .render(area, &mut buffer, &mut state);
}
