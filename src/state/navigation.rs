use std::hash::Hash;

use crate::model::TreeModel;
use crate::style::TreeScrollPolicy;

use super::TreeListViewState;

impl<Id: Copy + Eq + Hash> TreeListViewState<Id> {
    /// Selects the first visible row.
    pub const fn select_first(&mut self) {
        self.list_state.select_first();
    }

    /// Selects the last visible row.
    pub const fn select_last(&mut self) {
        self.list_state.select_last();
    }

    /// Scrolls the view down by the given number of rows.
    pub fn scroll_down_by(&mut self, amount: u16) {
        self.list_state.scroll_down_by(amount);
    }

    /// Scrolls the view up by the given number of rows.
    pub fn scroll_up_by(&mut self, amount: u16) {
        self.list_state.scroll_up_by(amount);
    }

    /// Returns the selected visible row index, if any.
    ///
    /// Uses the current visible-node cache.
    #[must_use]
    pub const fn selected_index(&self) -> Option<usize> {
        self.list_state.selected()
    }

    /// Returns the current scroll offset within the visible list.
    ///
    /// Uses the current visible-node cache.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.list_state.offset()
    }

    /// Returns the selected table column index, if any.
    #[must_use]
    pub const fn selected_column(&self) -> Option<usize> {
        self.list_state.selected_column()
    }

    /// Sets the selected visible row index.
    ///
    /// Out-of-range indices are clamped to the last visible row. Selecting any row in an empty
    /// visible list clears selection.
    pub const fn select_index(&mut self, selected: Option<usize>) {
        self.list_state.select(selected);
        self.clamp_selection();
    }

    /// Sets the current scroll offset within the visible list.
    ///
    /// The value is stored as provided; rendering clamps it to the visible range when needed.
    pub const fn set_offset(&mut self, offset: usize) {
        *self.list_state.offset_mut() = offset;
    }

    /// Sets the selected table column index.
    ///
    /// The widget does not clamp this value because column count is provided by `TreeColumns`.
    pub const fn select_column(&mut self, selected_column: Option<usize>) {
        *self.list_state.selected_column_mut() = selected_column;
    }

    /// Moves selection to the previous visible row.
    pub fn select_prev(&mut self) {
        if self.visible_nodes.is_empty() {
            self.list_state.select(None);
            return;
        }
        let selected = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some(selected.saturating_sub(1)));
    }

    /// Moves selection to the next visible row.
    pub fn select_next(&mut self) {
        if self.visible_nodes.is_empty() {
            self.list_state.select(None);
            return;
        }
        let selected = self.list_state.selected().unwrap_or(0);
        let new_selected = (selected + 1).min(self.visible_nodes.len().saturating_sub(1));
        self.list_state.select(Some(new_selected));
    }

    /// Adjusts scroll offset so the selection is within the viewport.
    pub fn ensure_selection_visible(&mut self, viewport_height: usize) {
        self.clamp_selection();
        let Some(selected) = self.list_state.selected() else {
            return;
        };
        let viewport_height = viewport_height.max(1);
        let offset = self.list_state.offset();
        if selected < offset {
            *self.list_state.offset_mut() = selected;
        } else if selected >= offset + viewport_height {
            *self.list_state.offset_mut() = selected + 1 - viewport_height;
        }
    }

    /// Adjusts selection visibility according to the provided scroll policy.
    pub fn ensure_selection_visible_with_policy(
        &mut self,
        viewport_height: usize,
        policy: TreeScrollPolicy,
    ) {
        match policy {
            TreeScrollPolicy::KeepInView => self.ensure_selection_visible(viewport_height),
            TreeScrollPolicy::CenterOnSelect => {
                self.ensure_selection_visible_centered(viewport_height);
            }
        }
    }

    fn ensure_selection_visible_centered(&mut self, viewport_height: usize) {
        self.clamp_selection();
        let Some(selected) = self.list_state.selected() else {
            return;
        };
        let viewport_height = viewport_height.max(1);
        let total = self.visible_nodes.len();
        if total <= viewport_height {
            *self.list_state.offset_mut() = 0;
            return;
        }

        // Center selection, then clamp to valid scroll range.
        let half = viewport_height / 2;
        let mut offset = selected.saturating_sub(half);
        let max_offset = total.saturating_sub(viewport_height);
        if offset > max_offset {
            offset = max_offset;
        }
        *self.list_state.offset_mut() = offset;
    }

    /// Returns the id of the currently selected node, if any.
    ///
    /// Uses the current visible-node cache.
    #[must_use]
    pub fn selected_id(&self) -> Option<Id> {
        self.selected_node().map(|node| node.id)
    }

    /// Returns the parent id of the currently selected node, if any.
    ///
    /// Uses the current visible-node cache.
    #[must_use]
    pub fn selected_parent_id(&self) -> Option<Id> {
        self.selected_node().and_then(|node| node.parent)
    }

    /// Returns the number of visible nodes in the current view.
    ///
    /// Uses the current visible-node cache.
    #[must_use]
    pub const fn visible_len(&self) -> usize {
        self.visible_nodes.len()
    }

    /// Returns whether the current visible list is empty.
    ///
    /// Uses the current visible-node cache.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.visible_nodes.is_empty()
    }

    /// Returns visible node ids in their current row order.
    ///
    /// Uses the current visible-node cache.
    pub fn visible_ids(&self) -> impl Iterator<Item = Id> + '_ {
        self.visible_nodes.iter().map(|node| node.id)
    }

    /// Returns the visible row index for a node id, if the node is currently visible.
    ///
    /// Uses the current visible-node cache.
    #[must_use]
    pub fn visible_index_of(&self, id: Id) -> Option<usize> {
        self.visible_index.get(&id).copied()
    }

    /// Returns whether a node id is currently visible.
    ///
    /// Uses the current visible-node cache.
    #[must_use]
    pub fn visible_contains(&self, id: Id) -> bool {
        self.visible_index.contains_key(&id)
    }

    /// Returns the depth level of the currently selected node.
    ///
    /// Uses the current visible-node cache.
    #[must_use]
    pub fn selected_level(&self) -> Option<u16> {
        self.selected_node().map(|node| node.level)
    }

    /// Returns whether the selected node is expanded (or `None` if nothing is selected).
    ///
    /// Uses the current visible-node cache.
    #[must_use]
    pub fn selected_is_expanded<T: TreeModel<Id = Id>>(&self, _model: &T) -> Option<bool> {
        let node = self.selected_node()?;
        Some(node.has_children && self.is_expanded(node.parent, node.id))
    }

    pub(crate) fn select_parent(&mut self) {
        let Some(selected_idx) = self.list_state.selected() else {
            return;
        };

        let Some(parent_id) = self
            .visible_nodes
            .get(selected_idx)
            .and_then(|node| node.parent)
        else {
            return;
        };

        if let Some(parent_idx) = self.visible_index_of(parent_id) {
            self.list_state.select(Some(parent_idx));
        }
    }

    pub(crate) fn select_child_with_descendants<T, R>(&mut self, model: &T, rebuild_visible: &mut R)
    where
        T: TreeModel<Id = Id>,
        R: FnMut(&mut Self, &T),
    {
        let Some(mut selected_idx) = self.list_state.selected() else {
            return;
        };
        let Some(selected_node) = self.visible_nodes.get(selected_idx) else {
            return;
        };
        let node_id = selected_node.id;
        let mut level = selected_node.level;
        let parent_id = selected_node.parent;

        if selected_node.has_children {
            let expand_key = Self::expansion_path(parent_id, node_id);
            if self.expanded.insert(expand_key) {
                self.dirty = true;
                rebuild_visible(self, model);

                if let Some(current_idx) = self.visible_index_of(node_id) {
                    selected_idx = current_idx;
                    if let Some(node) = self.visible_nodes.get(current_idx) {
                        level = node.level;
                    }
                    self.list_state.select(Some(current_idx));
                } else {
                    return;
                }
            }

            // Prefer children that themselves have descendants.
            let descendants_start = selected_idx + 1;
            for (offset, candidate) in self.visible_nodes[descendants_start..].iter().enumerate() {
                let candidate_level = candidate.level;
                if candidate_level <= level {
                    break;
                }
                if candidate_level == level + 1 && candidate.has_children {
                    let idx = descendants_start + offset;
                    self.list_state.select(Some(idx));
                    return;
                }
            }
        }

        // Fallback: pick the next node in the subtree that has children.
        let descendants_start = selected_idx + 1;
        for (offset, candidate) in self.visible_nodes[descendants_start..].iter().enumerate() {
            if candidate.level < level {
                break;
            }
            if candidate.has_children {
                let idx = descendants_start + offset;
                self.list_state.select(Some(idx));
                return;
            }
        }
    }
}
