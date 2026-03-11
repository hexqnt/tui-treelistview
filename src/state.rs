use std::hash::Hash;

use ratatui::widgets::TableState;
use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "keymap")]
use crate::keymap::TreeKeyBindings;

mod actions;
mod marks;
mod navigation;
mod visibility;

/// A visible node row with metadata used for rendering and navigation.
#[derive(Clone)]
pub struct VisibleNode<Id> {
    pub(crate) id: Id,
    pub(crate) level: u16,
    pub(crate) parent: Option<Id>,
    pub(crate) has_children: bool,
    pub(crate) is_last_sibling: bool,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct ExpansionPath<Id> {
    parent: Option<Id>,
    id: Id,
}

impl<Id> ExpansionPath<Id> {
    const fn new(parent: Option<Id>, id: Id) -> Self {
        Self { parent, id }
    }
}

impl<Id> From<(Option<Id>, Id)> for ExpansionPath<Id> {
    fn from((parent, id): (Option<Id>, Id)) -> Self {
        Self::new(parent, id)
    }
}

impl<Id> From<ExpansionPath<Id>> for (Option<Id>, Id) {
    fn from(value: ExpansionPath<Id>) -> Self {
        (value.parent, value.id)
    }
}

#[derive(Clone, Copy)]
struct SelectedNode<Id> {
    id: Id,
    parent: Option<Id>,
    level: u16,
    has_children: bool,
}

/// Widget state: expanded nodes, selection, and visibility/mark caches.
pub struct TreeListViewState<Id> {
    list_state: TableState,
    // Track expansion by (parent, id) to keep it tied to a specific path (e.g., after moves).
    expanded: FxHashSet<ExpansionPath<Id>>,
    // Cached visible rows to avoid recomputing DFS every render.
    visible_nodes: Vec<VisibleNode<Id>>,
    // Fast lookup from node id to visible row index.
    visible_index: FxHashMap<Id, usize>,
    // Marks whether visible_nodes must be rebuilt.
    dirty: bool,
    manual_marked: FxHashSet<Id>,
    // Cached effective marks (propagated from children).
    effective_marked: FxHashSet<Id>,
    marks_dirty: bool,
    draw_lines: bool,
    filter_memo: FxHashMap<Id, bool>,
    mark_memo: FxHashMap<Id, bool>,
    mark_seeds: FxHashSet<Id>,
    #[cfg(feature = "keymap")]
    keymap: TreeKeyBindings,
}

/// Snapshot of state (selection, expansion, marks).
///
/// With the `serde` feature enabled, this type derives `Serialize`/`Deserialize`.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
pub struct TreeListViewSnapshot<Id> {
    /// Expanded nodes as `(parent, id)` pairs.
    pub expanded: Vec<(Option<Id>, Id)>,
    /// Nodes explicitly marked by the user.
    pub manual_marked: Vec<Id>,
    /// Selected row index in the visible list.
    pub selected: Option<usize>,
    /// Selected column index in the table state.
    pub selected_column: Option<usize>,
    /// Scroll offset within the visible list.
    pub offset: usize,
    /// Whether guide lines were enabled.
    pub draw_lines: bool,
}

impl<Id: Copy + Eq + Hash> Default for TreeListViewState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id: Copy + Eq + Hash> TreeListViewState<Id> {
    /// Creates a new empty state with default capacity.
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    /// Creates a state with preallocated capacity for the given number of nodes.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            list_state: TableState::default(),
            expanded: FxHashSet::with_capacity_and_hasher(capacity, FxBuildHasher),
            visible_nodes: Vec::with_capacity(capacity),
            visible_index: FxHashMap::with_capacity_and_hasher(capacity, FxBuildHasher),
            dirty: true,
            manual_marked: FxHashSet::with_capacity_and_hasher(capacity, FxBuildHasher),
            effective_marked: FxHashSet::with_capacity_and_hasher(capacity, FxBuildHasher),
            marks_dirty: true,
            draw_lines: true,
            filter_memo: FxHashMap::with_capacity_and_hasher(capacity.max(1), FxBuildHasher),
            mark_memo: FxHashMap::with_capacity_and_hasher(capacity.max(1), FxBuildHasher),
            mark_seeds: FxHashSet::with_capacity_and_hasher(capacity.max(1), FxBuildHasher),
            #[cfg(feature = "keymap")]
            keymap: TreeKeyBindings::new(),
        }
    }

    #[cfg(feature = "keymap")]
    /// Returns a mutable reference to the key binding set.
    pub const fn keymap_mut(&mut self) -> &mut TreeKeyBindings {
        &mut self.keymap
    }

    pub(crate) const fn list_state(&self) -> &TableState {
        &self.list_state
    }

    pub(crate) const fn list_state_mut(&mut self) -> &mut TableState {
        &mut self.list_state
    }

    pub(crate) fn visible_nodes(&self) -> &[VisibleNode<Id>] {
        &self.visible_nodes
    }

    fn visible_index_of(&self, id: Id) -> Option<usize> {
        self.visible_index.get(&id).copied()
    }

    const fn expansion_path(parent: Option<Id>, id: Id) -> ExpansionPath<Id> {
        ExpansionPath::new(parent, id)
    }

    fn selected_node(&self) -> Option<SelectedNode<Id>> {
        let index = self.list_state.selected()?;
        let node = self.visible_nodes.get(index)?;
        Some(SelectedNode {
            id: node.id,
            parent: node.parent,
            level: node.level,
            has_children: node.has_children,
        })
    }

    #[cfg(feature = "edit")]
    fn selected_node_with_parent(&self) -> Option<(Id, Id)> {
        let node = self.selected_node()?;
        Some((node.id, node.parent?))
    }

    pub(crate) fn is_expanded(&self, parent: Option<Id>, id: Id) -> bool {
        self.expanded.contains(&Self::expansion_path(parent, id))
    }

    /// Captures a snapshot of the current state for persistence or restore.
    #[must_use]
    pub fn snapshot(&self) -> TreeListViewSnapshot<Id> {
        TreeListViewSnapshot {
            expanded: self.expanded.iter().copied().map(Into::into).collect(),
            manual_marked: self.manual_marked.iter().copied().collect(),
            selected: self.list_state.selected(),
            // Keep column and offset so TableState restores precisely.
            selected_column: self.list_state.selected_column(),
            offset: self.list_state.offset(),
            draw_lines: self.draw_lines,
        }
    }

    /// Restores state from a previously captured snapshot.
    pub fn restore(&mut self, snapshot: TreeListViewSnapshot<Id>) {
        self.expanded = snapshot.expanded.into_iter().map(Into::into).collect();
        self.manual_marked = snapshot.manual_marked.into_iter().collect();
        self.draw_lines = snapshot.draw_lines;
        *self.list_state.offset_mut() = snapshot.offset;
        self.list_state.select(snapshot.selected);
        *self.list_state.selected_column_mut() = snapshot.selected_column;
        self.dirty = true;
        self.marks_dirty = true;
    }

    /// Returns whether guide lines are drawn.
    #[inline]
    #[must_use]
    pub const fn draw_lines(&self) -> bool {
        self.draw_lines
    }

    /// Enables or disables drawing of guide lines.
    pub const fn set_draw_lines(&mut self, draw: bool) {
        self.draw_lines = draw;
    }

    /// Marks the visible-node cache as dirty.
    pub const fn invalidate(&mut self) {
        self.dirty = true;
    }

    /// Marks both visible-node and mark caches as dirty.
    pub const fn invalidate_all(&mut self) {
        self.dirty = true;
        self.marks_dirty = true;
    }
}

#[cfg(test)]
mod tests {
    use crate::action::{TreeAction, TreeEvent};
    use crate::model::{TreeFilterConfig, TreeModel};

    use super::TreeListViewState;

    struct TestTree {
        children: Vec<Vec<usize>>,
    }

    impl TestTree {
        fn new() -> Self {
            Self {
                children: vec![
                    vec![1, 2], // 0
                    vec![3, 4], // 1
                    vec![],     // 2
                    vec![],     // 3
                    vec![],     // 4
                ],
            }
        }
    }

    impl TreeModel for TestTree {
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

    #[test]
    fn builds_visible_nodes_with_expansion() {
        let tree = TestTree::new();
        let mut state = TreeListViewState::<usize>::new();

        state.set_expanded(0, None, true);
        state.set_expanded(1, Some(0), true);
        state.ensure_visible_nodes(&tree);

        let ids: Vec<_> = state.visible_nodes().iter().map(|n| n.id).collect();
        let levels: Vec<_> = state.visible_nodes().iter().map(|n| n.level).collect();

        assert_eq!(ids, vec![0, 1, 3, 4, 2]);
        assert_eq!(levels, vec![0, 1, 2, 2, 1]);
    }

    #[test]
    fn filtered_view_keeps_matching_path() {
        let tree = TestTree::new();
        let mut state = TreeListViewState::<usize>::new();
        let filter = |_: &TestTree, id: usize| id == 4;

        state.ensure_visible_nodes_filtered(&tree, &filter, TreeFilterConfig::enabled());

        let ids: Vec<_> = state.visible_nodes().iter().map(|n| n.id).collect();
        assert_eq!(ids, vec![0, 1, 4]);
    }

    #[test]
    fn select_prev_clears_selection_when_empty() {
        let mut state = TreeListViewState::<usize>::new();
        state.list_state.select(Some(0));

        state.select_prev();

        assert_eq!(state.list_state.selected(), None);
    }

    #[test]
    fn filtered_view_without_matches_clears_selection() {
        let tree = TestTree::new();
        let mut state = TreeListViewState::<usize>::new();
        let filter = |_: &TestTree, _: usize| false;

        state.list_state.select(Some(0));
        state.ensure_visible_nodes_filtered(&tree, &filter, TreeFilterConfig::enabled());

        assert!(state.visible_nodes().is_empty());
        assert_eq!(state.list_state.selected(), None);
    }

    #[test]
    fn expand_all_action_expands_and_collapses() {
        let tree = TestTree::new();
        let mut state = TreeListViewState::<usize>::new();

        let event = state.handle_action(&tree, TreeAction::<()>::ExpandAll);
        assert!(matches!(event, TreeEvent::Handled));
        state.ensure_visible_nodes(&tree);

        let ids: Vec<_> = state.visible_nodes().iter().map(|n| n.id).collect();
        assert_eq!(ids, vec![0, 1, 3, 4, 2]);

        let event = state.handle_action(&tree, TreeAction::<()>::CollapseAll);
        assert!(matches!(event, TreeEvent::Handled));
        state.ensure_visible_nodes(&tree);

        let ids: Vec<_> = state.visible_nodes().iter().map(|n| n.id).collect();
        assert_eq!(ids, vec![0]);
    }

    #[test]
    fn visible_nodes_cache_has_children() {
        let tree = TestTree::new();
        let mut state = TreeListViewState::<usize>::new();

        state.set_expanded(0, None, true);
        state.set_expanded(1, Some(0), true);
        state.ensure_visible_nodes(&tree);

        let has_children = |id: usize| {
            state
                .visible_nodes()
                .iter()
                .find(|node| node.id == id)
                .map(|node| node.has_children)
        };

        assert_eq!(has_children(0), Some(true));
        assert_eq!(has_children(1), Some(true));
        assert_eq!(has_children(2), Some(false));
        assert_eq!(has_children(3), Some(false));
        assert_eq!(has_children(4), Some(false));
    }

    #[test]
    fn filtered_select_child_keeps_filtered_view() {
        let tree = TestTree::new();
        let mut state = TreeListViewState::<usize>::new();
        let filter = |_: &TestTree, id: usize| id == 4;
        let config = TreeFilterConfig::Enabled { auto_expand: false };

        state.ensure_visible_nodes_filtered(&tree, &filter, config);
        assert_eq!(
            state
                .visible_nodes()
                .iter()
                .map(|node| node.id)
                .collect::<Vec<_>>(),
            vec![0]
        );
        state.select_first();

        let event =
            state.handle_action_filtered(&tree, &filter, config, TreeAction::<()>::SelectChild);
        assert!(matches!(event, TreeEvent::Handled));

        let ids: Vec<_> = state.visible_nodes().iter().map(|node| node.id).collect();
        assert_eq!(ids, vec![0, 1]);
        assert_eq!(state.selected_id(), Some(1));
    }
}
