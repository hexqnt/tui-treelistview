// Minimal example: a tiny tree with a single label column and default styling.
use ratatui::layout::{Constraint, Rect};
use ratatui::prelude::Buffer;
use ratatui::widgets::StatefulWidget;

use tui_treelistview::{
    SimpleColumns, TreeGlyphs, TreeLabelRenderer, TreeListView, TreeListViewState,
    TreeListViewStyle, TreeModel, TreeRowContext,
};

// Simple in-memory tree model with fixed children lists.
struct Model {
    children: Vec<Vec<usize>>,
    names: Vec<String>,
}

impl Model {
    // Build a three-node tree: root -> {alpha, beta}.
    fn new() -> Self {
        Self {
            children: vec![vec![1, 2], vec![], vec![]],
            names: vec!["root".to_string(), "alpha".to_string(), "beta".to_string()],
        }
    }
}

// The widget queries the tree through this trait only.
impl TreeModel for Model {
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
}

// Label renderer: maps a node id to the visible cell.
struct Label;

impl TreeLabelRenderer<Model> for Label {
    fn cell<'a>(
        &'a self,
        model: &'a Model,
        id: usize,
        _ctx: &TreeRowContext,
        _glyphs: &TreeGlyphs<'a>,
    ) -> ratatui::widgets::Cell<'a> {
        ratatui::widgets::Cell::from(model.names[id].as_str())
    }
}

fn main() {
    // Build model + render helpers.
    let model = Model::new();
    let label = Label;
    // Single-column layout: just the label, no header.
    let columns =
        SimpleColumns::<0, Model>::new(Constraint::Percentage(100), "", []).without_header();

    // State holds selection/expansion and must live across frames.
    let mut state = TreeListViewState::new();
    state.set_expanded(0, None, true);

    // Style controls borders/highlights and scrolling policy.
    let style = TreeListViewStyle::default();
    let widget = TreeListView::new(&model, &label, &columns, style);

    // Render into an in-memory buffer (no terminal required for the example).
    let area = Rect::new(0, 0, 40, 8);
    let mut buffer = Buffer::empty(area);

    widget.render(area, &mut buffer, &mut state);
}
