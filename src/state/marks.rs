use std::hash::Hash;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::model::TreeModel;

use super::TreeListViewState;

impl<Id: Copy + Eq + Hash> TreeListViewState<Id> {
    /// Recomputes effective marks for the current view.
    pub fn ensure_mark_cache<T: TreeModel<Id = Id>>(&mut self, model: &T) {
        if !self.marks_dirty {
            return;
        }

        let mut memo = std::mem::take(&mut self.mark_memo);
        let mut seeds = std::mem::take(&mut self.mark_seeds);

        memo.clear();
        seeds.clear();

        // Memoize subtree mark results to avoid repeated walks.
        let seeds_capacity = self.visible_nodes.len() + self.manual_marked.len() + 1;
        let memo_capacity = model.size_hint().max(seeds_capacity);
        let memo_extra = memo_capacity.saturating_sub(memo.capacity());
        if memo_extra > 0 {
            memo.reserve(memo_extra);
        }
        let seeds_extra = seeds_capacity.saturating_sub(seeds.capacity());
        if seeds_extra > 0 {
            seeds.reserve(seeds_extra);
        }

        for node in &self.visible_nodes {
            seeds.insert(node.id);
        }
        if let Some(root_id) = model.root() {
            seeds.insert(root_id);
        }
        seeds.extend(self.manual_marked.iter().copied());

        let manual_marked = &self.manual_marked;
        for node_id in seeds.iter().copied() {
            Self::compute_effective_mark(node_id, model, manual_marked, &mut memo);
        }

        self.effective_marked.clear();
        self.effective_marked.extend(
            memo.iter()
                .filter_map(|(node_id, marked)| marked.then_some(*node_id)),
        );

        self.mark_memo = memo;
        self.mark_seeds = seeds;
        self.marks_dirty = false;
    }

    /// Returns `true` if the node is effectively marked.
    #[inline]
    #[must_use]
    pub fn node_is_marked(&self, node_id: Id) -> bool {
        self.effective_marked.contains(&node_id)
    }

    /// Removes marks that refer to nodes no longer in the model.
    pub fn prune_removed_marks<T: TreeModel<Id = Id>>(&mut self, model: &T) {
        self.manual_marked
            .retain(|node_id| model.contains(*node_id));
        self.effective_marked
            .retain(|node_id| model.contains(*node_id));
        self.marks_dirty = true;
    }

    pub(crate) fn toggle_node_mark(&mut self, node_id: Id) {
        if !self.manual_marked.insert(node_id) {
            self.manual_marked.remove(&node_id);
        }
        self.marks_dirty = true;
    }

    fn compute_effective_mark<T: TreeModel<Id = Id>>(
        node_id: Id,
        model: &T,
        manual_marked: &FxHashSet<Id>,
        memo: &mut FxHashMap<Id, bool>,
    ) -> bool {
        if let Some(&cached) = memo.get(&node_id) {
            return cached;
        }

        // Mark is true if explicitly set or if all children are marked.
        let result = if manual_marked.contains(&node_id) {
            true
        } else {
            let children = model.children(node_id);
            if children.is_empty() {
                false
            } else {
                children
                    .iter()
                    .copied()
                    .all(|child| Self::compute_effective_mark(child, model, manual_marked, memo))
            }
        };

        memo.insert(node_id, result);
        result
    }
}
