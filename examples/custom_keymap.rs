// Custom keymap example: map keys to TreeAction manually and call handle_action.
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Rect};
use ratatui::prelude::Buffer;
use ratatui::widgets::{Cell, StatefulWidget};

use tui_treelistview::{
    SimpleColumns, TreeAction, TreeGlyphs, TreeLabelRenderer, TreeListView, TreeListViewState,
    TreeModel, TreeRowContext, TreeListViewStyle,
};

// Minimal model: root with three children and string labels.
struct Model {
    children: Vec<Vec<usize>>,
    names: Vec<String>,
}

impl Model {
    // Build a small fixed tree.
    fn new() -> Self {
        Self {
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

// Standard TreeModel implementation used by the widget.
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

// Label renderer maps an id to a cell.
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

// Custom keymap: WASD + a few extra actions.
fn map_key(event: KeyEvent) -> Option<TreeAction> {
    match (event.code, event.modifiers) {
        (KeyCode::Char('w'), KeyModifiers::NONE) => Some(TreeAction::SelectPrev),
        (KeyCode::Char('s'), KeyModifiers::NONE) => Some(TreeAction::SelectNext),
        (KeyCode::Char('a'), KeyModifiers::NONE) => Some(TreeAction::SelectParent),
        (KeyCode::Char('d'), KeyModifiers::NONE) => Some(TreeAction::SelectChild),
        (KeyCode::Char('x'), KeyModifiers::NONE) => Some(TreeAction::ToggleNode),
        (KeyCode::Char('m'), KeyModifiers::NONE) => Some(TreeAction::ToggleMark),
        (KeyCode::Char('g'), KeyModifiers::NONE) => Some(TreeAction::ToggleGuides),
        _ => None,
    }
}

fn main() {
    // Build model + render helpers.
    let model = Model::new();
    let label = Label;
    // Single-column layout with no header.
    let columns =
        SimpleColumns::<0, Model>::new(Constraint::Percentage(100), "", []).without_header();

    // State stores expansion/selection for the widget.
    let mut state = TreeListViewState::new();
    state.set_expanded(0, None, true);

    // Use defaults for styling; input mapping happens outside the widget.
    let style = TreeListViewStyle::default();
    let widget = TreeListView::new(&model, &label, &columns, style);

    // Simulate a few key presses and apply actions manually.
    let mut buffer = Buffer::empty(Rect::new(0, 0, 40, 8));

    let demo_keys = [
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE),
        KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE),
    ];

    for key in demo_keys {
        if let Some(action) = map_key(key) {
            state.handle_action(&model, action);
        }
    }

    // Render into a buffer for the example.
    widget.render(Rect::new(0, 0, 40, 8), &mut buffer, &mut state);
}
