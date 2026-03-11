use std::hash::Hash;

use rustc_hash::FxHashMap;
use smallvec::SmallVec;

use crate::model::{TreeFilter, TreeFilterConfig, TreeModel};

use super::{TreeListViewState, VisibleNode};

struct FilterBuildCtx<'a, T, F, Id>
where
    T: TreeModel<Id = Id>,
    F: TreeFilter<T>,
    Id: Copy + Eq + Hash,
{
    model: &'a T,
    filter: &'a F,
    auto_expand: bool,
    memo: &'a mut FxHashMap<Id, bool>,
}

impl<Id: Copy + Eq + Hash> TreeListViewState<Id> {
    /// Expands the tree to the node and selects it if present.
    pub fn select_by_id<T: TreeModel<Id = Id>>(&mut self, model: &T, id: Id) -> bool {
        let _ = self.expand_to(model, id);
        self.ensure_visible_nodes(model);
        if let Some(idx) = self.visible_index_of(id) {
            self.list_state.select(Some(idx));
            true
        } else {
            false
        }
    }

    /// Expands the tree to the node and returns whether it becomes visible.
    pub fn ensure_visible_id<T: TreeModel<Id = Id>>(&mut self, model: &T, id: Id) -> bool {
        let _ = self.expand_to(model, id);
        self.ensure_visible_nodes(model);
        self.visible_index.contains_key(&id)
    }

    /// Expands all ancestors of the node so it becomes visible.
    pub fn expand_to<T: TreeModel<Id = Id>>(&mut self, model: &T, id: Id) -> bool {
        let Some(path) = Self::find_path_to(model, id) else {
            return false;
        };
        for (parent, node) in path {
            if !model.children(node).is_empty() {
                self.expanded.insert(Self::expansion_path(parent, node));
            }
        }
        self.dirty = true;
        true
    }

    /// Expands all nodes in the model.
    pub fn expand_all<T: TreeModel<Id = Id>>(&mut self, model: &T) {
        self.expanded.clear();
        let hint = model.size_hint();
        if hint > 0 {
            let extra = hint.saturating_sub(self.expanded.capacity());
            if extra > 0 {
                self.expanded.reserve(extra);
            }
        }
        if let Some(root) = model.root() {
            let stack_capacity = hint.max(1);
            let mut stack = Vec::with_capacity(stack_capacity);
            stack.push((None, root));
            while let Some((parent, node)) = stack.pop() {
                let children = model.children(node);
                if !children.is_empty() {
                    self.expanded.insert(Self::expansion_path(parent, node));
                    for child in children.iter().copied() {
                        stack.push((Some(node), child));
                    }
                }
            }
        }
        self.dirty = true;
    }

    /// Collapses all nodes.
    pub fn collapse_all(&mut self) {
        self.expanded.clear();
        self.dirty = true;
    }

    /// Ensures the visible node list is up to date (if marked dirty).
    pub fn ensure_visible_nodes<T: TreeModel<Id = Id>>(&mut self, model: &T) {
        if !self.dirty {
            return;
        }
        self.update_visible_nodes(model);
    }

    /// Ensures the visible node list is up to date with an active filter.
    pub fn ensure_visible_nodes_filtered<T, F>(
        &mut self,
        model: &T,
        filter: &F,
        config: TreeFilterConfig,
    ) where
        T: TreeModel<Id = Id>,
        F: TreeFilter<T>,
    {
        if !self.dirty {
            return;
        }

        let TreeFilterConfig::Enabled { auto_expand } = config else {
            self.update_visible_nodes(model);
            return;
        };

        self.visible_nodes.clear();
        self.visible_index.clear();
        self.reserve_visible_capacity(model);

        let mut memo = std::mem::take(&mut self.filter_memo);
        memo.clear();
        let memo_capacity = model.size_hint().max(1);
        let extra = memo_capacity.saturating_sub(memo.capacity());
        if extra > 0 {
            memo.reserve(extra);
        }

        if let Some(root) = model.root() {
            let mut ctx = FilterBuildCtx {
                model,
                filter,
                auto_expand,
                memo: &mut memo,
            };
            self.build_visible_nodes_filtered(&mut ctx, root, 0, None, true);
        }

        self.filter_memo = memo;
        self.dirty = false;
        self.clamp_selection();
    }

    /// Toggles expansion state for the given node.
    pub fn toggle(&mut self, node_id: Id, parent: Option<Id>) {
        let key = Self::expansion_path(parent, node_id);
        if self.expanded.contains(&key) {
            self.expanded.remove(&key);
        } else {
            self.expanded.insert(key);
        }
        self.dirty = true;
    }

    /// Sets expansion state for the given node.
    pub fn set_expanded(&mut self, node_id: Id, parent: Option<Id>, expand: bool) {
        let key = Self::expansion_path(parent, node_id);
        if expand {
            self.expanded.insert(key);
        } else {
            self.expanded.remove(&key);
        }
        self.dirty = true;
    }

    fn reserve_visible_capacity<T: TreeModel<Id = Id>>(&mut self, model: &T) {
        let hint = model.size_hint();
        if hint == 0 {
            return;
        }
        let node_extra = hint.saturating_sub(self.visible_nodes.capacity());
        if node_extra > 0 {
            self.visible_nodes.reserve(node_extra);
        }
        let index_extra = hint.saturating_sub(self.visible_index.capacity());
        if index_extra > 0 {
            self.visible_index.reserve(index_extra);
        }
    }

    pub(crate) fn update_visible_nodes<T: TreeModel<Id = Id>>(&mut self, model: &T) {
        self.visible_nodes.clear();
        self.visible_index.clear();
        self.reserve_visible_capacity(model);
        if let Some(root) = model.root() {
            self.build_visible_nodes(model, root, 0, None, true);
        }
        self.dirty = false;
        self.clamp_selection();
    }

    fn find_path_to<T: TreeModel<Id = Id>>(model: &T, target: Id) -> Option<Vec<(Option<Id>, Id)>> {
        let root = model.root()?;
        let mut path: Vec<(Option<Id>, Id)> = Vec::new();
        if Self::dfs_find_path(model, root, None, target, &mut path) {
            Some(path)
        } else {
            None
        }
    }

    fn dfs_find_path<T: TreeModel<Id = Id>>(
        model: &T,
        node: Id,
        parent: Option<Id>,
        target: Id,
        path: &mut Vec<(Option<Id>, Id)>,
    ) -> bool {
        path.push((parent, node));
        if node == target {
            return true;
        }
        for child in model.children(node).iter().copied() {
            if Self::dfs_find_path(model, child, Some(node), target, path) {
                return true;
            }
        }
        path.pop();
        false
    }

    fn build_visible_nodes<T: TreeModel<Id = Id>>(
        &mut self,
        model: &T,
        node_id: Id,
        level: u16,
        parent: Option<Id>,
        is_last_sibling: bool,
    ) {
        let children = model.children(node_id);
        let has_children = !children.is_empty();
        let idx = self.visible_nodes.len();
        self.visible_nodes.push(VisibleNode {
            id: node_id,
            level,
            parent,
            has_children,
            is_last_sibling,
        });
        self.visible_index.insert(node_id, idx);

        let is_expanded = has_children && self.is_expanded(parent, node_id);
        if !is_expanded {
            return;
        }

        let last_idx = children.len().saturating_sub(1);
        for (idx, child) in children.iter().copied().enumerate() {
            self.build_visible_nodes(model, child, level + 1, Some(node_id), idx == last_idx);
        }
    }

    fn subtree_has_match<T, F>(ctx: &mut FilterBuildCtx<'_, T, F, Id>, node_id: Id) -> bool
    where
        T: TreeModel<Id = Id>,
        F: TreeFilter<T>,
    {
        if let Some(&cached) = ctx.memo.get(&node_id) {
            return cached;
        }

        let mut matched = ctx.filter.is_match(ctx.model, node_id);
        if !matched {
            for child in ctx.model.children(node_id).iter().copied() {
                if Self::subtree_has_match(ctx, child) {
                    matched = true;
                    break;
                }
            }
        }

        ctx.memo.insert(node_id, matched);
        matched
    }

    fn build_visible_nodes_filtered<T, F>(
        &mut self,
        ctx: &mut FilterBuildCtx<'_, T, F, Id>,
        node_id: Id,
        level: u16,
        parent: Option<Id>,
        is_last_sibling: bool,
    ) -> bool
    where
        T: TreeModel<Id = Id>,
        F: TreeFilter<T>,
    {
        let self_match = ctx.filter.is_match(ctx.model, node_id);
        let children = ctx.model.children(node_id);
        let has_children = !children.is_empty();

        let mut visible_children: SmallVec<[Id; 8]> = SmallVec::new();
        for child in children.iter().copied() {
            if Self::subtree_has_match(ctx, child) {
                visible_children.push(child);
            }
        }

        let include_self = self_match || !visible_children.is_empty();
        if !include_self {
            return false;
        }

        let idx = self.visible_nodes.len();
        self.visible_nodes.push(VisibleNode {
            id: node_id,
            level,
            parent,
            has_children,
            is_last_sibling,
        });
        self.visible_index.insert(node_id, idx);

        let expand_children = ctx.auto_expand || self.is_expanded(parent, node_id);
        if expand_children && !visible_children.is_empty() {
            let last_idx = visible_children.len().saturating_sub(1);
            for (idx, child) in visible_children.iter().copied().enumerate() {
                self.build_visible_nodes_filtered(
                    ctx,
                    child,
                    level + 1,
                    Some(node_id),
                    idx == last_idx,
                );
            }
        }

        true
    }

    pub(crate) const fn clamp_selection(&mut self) {
        if self.visible_nodes.is_empty() {
            self.list_state.select(None);
            return;
        }

        if let Some(selected) = self.list_state.selected()
            && selected >= self.visible_nodes.len()
        {
            self.list_state
                .select(Some(self.visible_nodes.len().saturating_sub(1)));
        }
    }

    pub(crate) fn set_expanded_recursive<T: TreeModel<Id = Id>>(
        &mut self,
        model: &T,
        node_id: Id,
        parent: Option<Id>,
        expand: bool,
    ) {
        let children = model.children(node_id);
        let key = Self::expansion_path(parent, node_id);
        if expand {
            if !children.is_empty() {
                self.expanded.insert(key);
            }
        } else {
            self.expanded.remove(&key);
        }

        if children.is_empty() {
            return;
        }

        for child in children.iter().copied() {
            self.set_expanded_recursive(model, child, Some(node_id), expand);
        }
    }
}
