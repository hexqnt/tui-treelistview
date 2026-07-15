use std::hash::Hash;

use rustc_hash::{FxBuildHasher, FxHashMap};
use smallvec::SmallVec;

use crate::context::TreeExpansionState;
use crate::model::{
    TreeChildren, TreeFilter, TreeModel, TreeQuery, TreeSelectionFallback, TreeSort,
};
use crate::projection::{OccurrencePath, ProjectedNode};
use crate::traversal::TreeWalk;

use super::{ExpansionPath, TreeListViewState};

impl<Id: Copy + Eq + Hash> TreeListViewState<Id> {
    /// Synchronizes the projection with model, query, and expansion revisions.
    ///
    /// Returns `true` when the projection was rebuilt.
    pub fn ensure_projection<T, F, S>(&mut self, model: &T, query: &TreeQuery<F, S>) -> bool
    where
        T: TreeModel<Id = Id>,
        F: TreeFilter<T>,
        S: TreeSort<T>,
    {
        let expansion_revision = self.expanded.revision();
        if self.projection.is_current(model, query, expansion_revision) {
            return false;
        }

        let old_index = self.selected_row;
        let old_path = old_index.and_then(|index| self.projection.occurrence_path(index));
        let expanded = &self.expanded;
        self.projection
            .rebuild(model, query, expansion_revision, |parent, id| {
                expanded.contains(&ExpansionPath::new(parent, id))
            });
        self.restore_selection_after_rebuild(
            old_index,
            old_path.as_ref(),
            query.selection_fallback(),
        );
        self.selection_needs_visibility = self.selected.is_some();
        self.clamp_offsets();
        true
    }

    /// Expands the path to a node and selects it when it is present in the projection.
    pub fn select_by_id<T, F, S>(&mut self, model: &T, query: &TreeQuery<F, S>, id: Id) -> bool
    where
        T: TreeModel<Id = Id>,
        F: TreeFilter<T>,
        S: TreeSort<T>,
    {
        if !self.expand_to(model, id) {
            return false;
        }
        self.ensure_projection(model, query);
        if let Some(index) = self.projection.index_of(id) {
            self.selected = Some(id);
            self.selected_row = Some(index);
            self.selection_needs_visibility = true;
            true
        } else {
            false
        }
    }

    /// Expands every loaded ancestor of a node.
    pub fn expand_to<T: TreeModel<Id = Id>>(&mut self, model: &T, target: Id) -> bool {
        let hint = model.size_hint();
        let mut parents = FxHashMap::with_capacity_and_hasher(hint, FxBuildHasher);
        let mut found = false;
        for node in TreeWalk::forest(model) {
            parents.insert(node.id, (node.parent, node.children.is_branch()));
            if node.id == target {
                found = true;
                break;
            }
        }
        if !found {
            return false;
        }

        let mut path = SmallVec::<[Id; 16]>::new();
        let mut cursor = Some(target);
        while let Some(id) = cursor {
            path.push(id);
            cursor = parents.get(&id).and_then(|(parent, _)| *parent);
        }
        path.reverse();

        self.expanded.mutate(|expanded| {
            let mut changed = false;
            for window in path.windows(2) {
                let (parent, is_branch) = parents[&window[0]];
                if is_branch {
                    changed |= expanded.insert(ExpansionPath::new(parent, window[0]));
                }
            }
            changed
        });
        true
    }

    /// Expands every loaded branch in the forest.
    pub fn expand_all<T: TreeModel<Id = Id>>(&mut self, model: &T) -> bool {
        self.expanded.mutate(|expanded| {
            let mut changed = false;
            for node in TreeWalk::forest(model) {
                if let TreeChildren::Loaded(children) = node.children
                    && !children.is_empty()
                {
                    changed |= expanded.insert(ExpansionPath::new(node.parent, node.id));
                }
            }
            changed
        })
    }

    /// Collapses every branch.
    pub fn collapse_all(&mut self) -> bool {
        self.expanded.clear()
    }

    /// Sets the expansion state of a specific path.
    pub fn set_expanded(&mut self, id: Id, parent: Option<Id>, expanded: bool) -> bool {
        let path = ExpansionPath::new(parent, id);
        self.expanded.set_membership(path, expanded)
    }

    /// Returns persisted expansion state rather than filter-forced state.
    #[must_use]
    pub fn node_is_expanded(&self, id: Id, parent: Option<Id>) -> bool {
        self.is_expanded(parent, id)
    }

    /// Returns the effective expansion state of a visible node.
    #[must_use]
    pub fn effective_expansion(&self, id: Id) -> Option<TreeExpansionState> {
        self.projection.get_by_id(id).map(ProjectedNode::expansion)
    }

    /// Iterates over persisted expanded paths in unspecified order.
    pub fn expanded_paths(&self) -> impl Iterator<Item = (Option<Id>, Id)> + '_ {
        self.expanded.iter().map(|path| (path.parent, path.id))
    }

    pub(crate) fn set_expanded_recursive<T: TreeModel<Id = Id>>(
        &mut self,
        model: &T,
        root: Id,
        parent: Option<Id>,
        expand: bool,
    ) -> bool {
        self.expanded.mutate(|expanded| {
            let mut changed = false;
            for node in TreeWalk::subtree(model, parent, root) {
                let path = ExpansionPath::new(node.parent, node.id);
                if expand {
                    if matches!(node.children, TreeChildren::Loaded(children) if !children.is_empty())
                    {
                        changed |= expanded.insert(path);
                    }
                } else {
                    changed |= expanded.remove(&path);
                }
            }
            changed
        })
    }

    fn restore_selection_after_rebuild(
        &mut self,
        old_index: Option<usize>,
        old_path: Option<&OccurrencePath<Id>>,
        fallback: TreeSelectionFallback,
    ) {
        if let Some(path) = old_path {
            if let Some(index) = self.projection.index_of_path(path) {
                self.select_rebuilt_row(Some(index));
                return;
            }

            if let Some(index) = self
                .selected
                .and_then(|selected| self.projection.index_of(selected))
            {
                self.select_rebuilt_row(Some(index));
                return;
            }

            if matches!(fallback, TreeSelectionFallback::ParentThenNearest) {
                for end in (1..path.len()).rev() {
                    if let Some(index) = self.projection.index_of_path_prefix(path, end) {
                        self.select_rebuilt_row(Some(index));
                        return;
                    }
                }
            }
        } else if let Some(index) = self
            .selected
            .and_then(|selected| self.projection.index_of(selected))
        {
            self.select_rebuilt_row(Some(index));
            return;
        }

        let selected_row = match fallback {
            TreeSelectionFallback::Clear => None,
            TreeSelectionFallback::Nearest | TreeSelectionFallback::ParentThenNearest => old_index
                .and_then(|index| {
                    let index = index.min(self.projection.len().saturating_sub(1));
                    self.projection.nodes().get(index).map(|_| index)
                }),
        };
        self.select_rebuilt_row(selected_row);
    }

    fn select_rebuilt_row(&mut self, selected_row: Option<usize>) {
        self.selected = selected_row
            .and_then(|index| self.projection.nodes().get(index))
            .map(|node| node.id());
        self.selected_row = selected_row;
    }

    fn clamp_offsets(&mut self) {
        self.offset = self.offset.min(self.projection.len().saturating_sub(1));
        if self.projection.is_empty() {
            self.offset = 0;
        }
    }
}
