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
    #[must_use]
    pub fn selected_id(&self) -> Option<Id> {
        self.selected_node().map(|node| node.id)
    }

    /// Returns the parent id of the currently selected node, if any.
    #[must_use]
    pub fn selected_parent_id(&self) -> Option<Id> {
        self.selected_node().and_then(|node| node.parent)
    }

    /// Returns the number of visible nodes in the current view.
    #[must_use]
    pub const fn visible_len(&self) -> usize {
        self.visible_nodes.len()
    }

    /// Returns the depth level of the currently selected node.
    #[must_use]
    pub fn selected_level(&self) -> Option<u16> {
        self.selected_node().map(|node| node.level)
    }

    /// Returns whether the selected node is expanded (or `None` if nothing is selected).
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
            for idx in selected_idx + 1..self.visible_nodes.len() {
                let candidate = &self.visible_nodes[idx];
                let candidate_level = candidate.level;
                if candidate_level <= level {
                    break;
                }
                if candidate_level == level + 1 && candidate.has_children {
                    self.list_state.select(Some(idx));
                    return;
                }
            }
        }

        // Fallback: pick the next node in the subtree that has children.
        for idx in selected_idx + 1..self.visible_nodes.len() {
            let candidate = &self.visible_nodes[idx];
            if candidate.level < level {
                break;
            }
            if candidate.has_children {
                self.list_state.select(Some(idx));
                return;
            }
        }
    }
}
