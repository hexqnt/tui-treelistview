// Edit-actions example: use TreeEdit + handle_edit_action to modify the model.
use ratatui::layout::{Constraint, Rect};
use ratatui::prelude::Buffer;
use ratatui::widgets::{Cell, StatefulWidget};

use tui_treelistview::{
    SimpleColumns, TreeAction, TreeEdit, TreeGlyphs, TreeLabelRenderer, TreeListView,
    TreeListViewState, TreeListViewStyle, TreeModel, TreeRowContext,
};

// Small editable tree with explicit parent/children lists.
#[derive(Clone)]
struct Model {
    root: usize,
    children: Vec<Vec<usize>>,
    names: Vec<String>,
}

impl Model {
    // Root with three children.
    fn new() -> Self {
        Self {
            root: 0,
            children: vec![vec![1, 2, 3], vec![], vec![], vec![]],
            names: vec![
                "root".to_string(),
                "alpha".to_string(),
                "beta".to_string(),
                "gamma".to_string(),
            ],
        }
    }
}

// Read-only traversal interface.
impl TreeModel for Model {
    type Id = usize;

    fn root(&self) -> Option<Self::Id> {
        Some(self.root)
    }

    fn children(&self, id: Self::Id) -> &[Self::Id] {
        &self.children[id]
    }

    fn contains(&self, id: Self::Id) -> bool {
        id < self.children.len()
    }
}

// Minimal edit API required by handle_edit_action.
impl TreeEdit for Model {
    fn is_root(&self, id: Self::Id) -> bool {
        id == self.root
    }

    fn move_child_up(&mut self, parent: Self::Id, child: Self::Id) -> bool {
        let children = &mut self.children[parent];
        if let Some(idx) = children.iter().position(|&id| id == child) {
            if idx == 0 {
                return false;
            }
            children.swap(idx, idx - 1);
            return true;
        }
        false
    }

    fn move_child_down(&mut self, parent: Self::Id, child: Self::Id) -> bool {
        let children = &mut self.children[parent];
        if let Some(idx) = children.iter().position(|&id| id == child) {
            if idx + 1 >= children.len() {
                return false;
            }
            children.swap(idx, idx + 1);
            return true;
        }
        false
    }

    fn remove_child(&mut self, parent: Self::Id, child: Self::Id) {
        self.children[parent].retain(|&id| id != child);
    }

    fn delete_node(&mut self, id: Self::Id) {
        if id == self.root {
            return;
        }
        for children in &mut self.children {
            children.retain(|&child_id| child_id != id);
        }
        if let Some(node_children) = self.children.get_mut(id) {
            node_children.clear();
        }
    }

    fn add_child(&mut self, parent: Self::Id, child: Self::Id) {
        if !self.children[parent].contains(&child) {
            self.children[parent].push(child);
        }
    }
}

// Label renderer for the tree column.
struct Label;

impl TreeLabelRenderer<Model> for Label {
    fn cell<'a>(
        &'a self,
        model: &'a Model,
        id: usize,
        _ctx: &TreeRowContext,
        _glyphs: &TreeGlyphs<'a>,
    ) -> Cell<'a> {
        Cell::from(model.names[id].as_str())
    }
}

fn main() {
    // Build model + helpers.
    let mut model = Model::new();
    let label = Label;
    // Single-column layout with no header.
    let columns =
        SimpleColumns::<0, Model>::new(Constraint::Percentage(100), "", []).without_header();

    // State drives expansion/selection.
    let mut state = TreeListViewState::new();
    state.set_expanded(0, None, true);
    state.select_by_id(&model, 2);

    // Local clipboard for yank/paste actions.
    let mut clipboard: Option<usize> = None;

    // Move selected node up, then yank and paste under root.
    state.handle_edit_action(&mut model, TreeAction::ReorderUp, &mut clipboard);
    state.handle_edit_action(&mut model, TreeAction::YankNode, &mut clipboard);
    state.select_by_id(&model, 0);
    state.handle_edit_action(&mut model, TreeAction::PasteNode, &mut clipboard);

    // Render to a buffer to keep the example terminal-free.
    let style = TreeListViewStyle::default();
    let widget = TreeListView::new(&model, &label, &columns, style);

    let mut buffer = Buffer::empty(Rect::new(0, 0, 40, 8));
    widget.render(Rect::new(0, 0, 40, 8), &mut buffer, &mut state);
}
