use std::hash::Hash;

use crate::context::TreeMarkState;
use crate::model::TreeModel;
use crate::traversal::TreePostorder;

use super::TreeListViewState;

impl<Id: Copy + Eq + Hash> TreeListViewState<Id> {
    /// Rebuilds tri-state marks after the model or manual marks change.
    pub fn ensure_mark_states<T: TreeModel<Id = Id>>(&mut self, model: &T) {
        let stamp = (model.revision(), self.manual_marked.revision());
        if self.mark_stamp == Some(stamp) {
            return;
        }

        self.mark_states.clear();
        for node in TreePostorder::forest(model) {
            let mark = if self.manual_marked.contains(&node.id) {
                TreeMarkState::Marked
            } else {
                let children = node.children;
                if children.is_empty() {
                    TreeMarkState::Unmarked
                } else {
                    let mut any = false;
                    let mut all = true;
                    for child in children {
                        let child_mark = self.mark_states.get(child).copied().unwrap_or_default();
                        any |= child_mark != TreeMarkState::Unmarked;
                        all &= child_mark == TreeMarkState::Marked;
                    }
                    if all {
                        TreeMarkState::Marked
                    } else if any {
                        TreeMarkState::Partial
                    } else {
                        TreeMarkState::Unmarked
                    }
                }
            };
            if mark != TreeMarkState::Unmarked {
                self.mark_states.insert(node.id, mark);
            }
        }

        for id in self.manual_marked.iter().copied() {
            self.mark_states.insert(id, TreeMarkState::Marked);
        }
        self.mark_stamp = Some(stamp);
    }

    /// Returns an aggregated mark from the most recently computed cache.
    #[must_use]
    pub fn mark_state(&self, id: Id) -> TreeMarkState {
        self.mark_state_cached(id)
    }

    #[must_use]
    pub fn is_manually_marked(&self, id: Id) -> bool {
        self.manual_marked.contains(&id)
    }

    /// Sets a node's manual mark.
    pub fn set_marked(&mut self, id: Id, marked: bool) -> bool {
        self.manual_marked.set_membership(id, marked)
    }

    /// Toggles a node's manual mark.
    pub fn toggle_marked(&mut self, id: Id) -> bool {
        let marked = !self.manual_marked.contains(&id);
        self.set_marked(id, marked)
    }

    /// Removes every manual mark.
    pub fn clear_marks(&mut self) -> bool {
        self.manual_marked.clear()
    }

    pub fn manual_marked_ids(&self) -> impl Iterator<Item = Id> + '_ {
        self.manual_marked.iter().copied()
    }
}
