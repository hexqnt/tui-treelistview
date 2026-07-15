use std::hash::Hash;

use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};
use smallvec::SmallVec;

use crate::context::{TreeExpansionState, TreeMatchState};
use crate::model::{
    TreeChildren, TreeFilter, TreeFilterConfig, TreeModel, TreeQuery, TreeRevision,
    TreeRootVisibility, TreeSort,
};
use crate::traversal::TreePostorder;

pub struct OccurrencePath<Id> {
    root_parent: Option<Id>,
    ids: SmallVec<[Id; 16]>,
}

impl<Id> OccurrencePath<Id> {
    pub fn len(&self) -> usize {
        self.ids.len()
    }
}

/// A node in the flat visible tree projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProjectedNode<Id> {
    id: Id,
    parent: Option<Id>,
    parent_index: Option<usize>,
    level: usize,
    is_last_sibling: bool,
    visible_child_count: usize,
    expansion: TreeExpansionState,
    match_state: TreeMatchState,
}

impl<Id: Copy> ProjectedNode<Id> {
    #[must_use]
    pub const fn id(self) -> Id {
        self.id
    }

    #[must_use]
    pub const fn parent(self) -> Option<Id> {
        self.parent
    }

    /// Возвращает индекс родительского вхождения в проекции строк.
    ///
    /// В отличие от [`Self::parent`], различает повторные вхождения одной вершины
    /// модели в проекции DAG.
    #[must_use]
    pub const fn parent_index(self) -> Option<usize> {
        self.parent_index
    }

    #[must_use]
    pub const fn level(self) -> usize {
        self.level
    }

    #[must_use]
    pub const fn is_last_sibling(self) -> bool {
        self.is_last_sibling
    }

    #[must_use]
    pub const fn visible_child_count(self) -> usize {
        self.visible_child_count
    }

    #[must_use]
    pub const fn expansion(self) -> TreeExpansionState {
        self.expansion
    }

    #[must_use]
    pub const fn match_state(self) -> TreeMatchState {
        self.match_state
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProjectionStamp {
    model: TreeRevision,
    filter: PolicyStamp,
    sort: PolicyStamp,
    expansion: TreeRevision,
    filter_config: TreeFilterConfig,
    root_visibility: TreeRootVisibility,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PolicyStamp {
    revision: TreeRevision,
    generation: TreeRevision,
}

impl PolicyStamp {
    const fn new(revision: TreeRevision, generation: TreeRevision) -> Self {
        Self {
            revision,
            generation,
        }
    }
}

/// A cached flat projection shared by navigation and rendering.
pub struct TreeProjection<Id> {
    nodes: Vec<ProjectedNode<Id>>,
    index: FxHashMap<Id, usize>,
    filter_memo: FxHashMap<Id, bool>,
    direct_matches: FxHashSet<Id>,
    stamp: Option<ProjectionStamp>,
}

impl<Id: Copy + Eq + Hash> TreeProjection<Id> {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            nodes: Vec::with_capacity(capacity),
            index: FxHashMap::with_capacity_and_hasher(capacity, FxBuildHasher),
            filter_memo: FxHashMap::with_capacity_and_hasher(capacity, FxBuildHasher),
            direct_matches: FxHashSet::with_capacity_and_hasher(capacity, FxBuildHasher),
            stamp: None,
        }
    }

    /// Returns rows in display order.
    #[must_use]
    pub fn nodes(&self) -> &[ProjectedNode<Id>] {
        &self.nodes
    }

    /// Returns the number of visible rows.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Returns `true` when the projection is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Возвращает индекс первого видимого вхождения узла.
    #[must_use]
    pub fn index_of(&self, id: Id) -> Option<usize> {
        self.index.get(&id).copied()
    }

    /// Возвращает первое видимое вхождение узла по идентификатору.
    #[must_use]
    pub fn get_by_id(&self, id: Id) -> Option<ProjectedNode<Id>> {
        self.index_of(id)
            .and_then(|index| self.nodes.get(index))
            .copied()
    }

    pub(crate) fn is_current<T, F, S>(
        &self,
        model: &T,
        query: &TreeQuery<F, S>,
        expansion: TreeRevision,
    ) -> bool
    where
        T: TreeModel<Id = Id>,
    {
        self.stamp == Some(Self::stamp(model, query, expansion))
    }

    pub(crate) fn rebuild<T, F, S, E>(
        &mut self,
        model: &T,
        query: &TreeQuery<F, S>,
        expansion_revision: TreeRevision,
        is_expanded: E,
    ) where
        T: TreeModel<Id = Id>,
        F: TreeFilter<T>,
        S: TreeSort<T>,
        E: Fn(Option<Id>, Id) -> bool,
    {
        self.nodes.clear();
        self.index.clear();
        self.reserve(model.size_hint());

        let filtering = matches!(query.filter_config(), TreeFilterConfig::Enabled { .. });
        if filtering {
            self.compute_filter_matches(model, query.filter());
        } else {
            self.filter_memo.clear();
            self.direct_matches.clear();
        }

        let mut roots: SmallVec<[Id; 8]> = model.roots().collect();
        Self::sort_ids(model, query.sort(), &mut roots);
        let mut stack = Vec::with_capacity(model.size_hint().min(1024).max(roots.len()));

        match query.root_visibility() {
            TreeRootVisibility::Visible => {
                Self::push_children(&mut stack, &roots, None, None, 0);
            }
            TreeRootVisibility::Hidden => {
                for root in roots.iter().rev().copied() {
                    let mut children =
                        self.visible_children(query, model.children(root).loaded_slice());
                    Self::sort_ids(model, query.sort(), &mut children);
                    Self::push_children(&mut stack, &children, Some(root), None, 0);
                }
            }
        }

        while let Some(frame) = stack.pop() {
            if filtering && !self.filter_memo.get(&frame.id).copied().unwrap_or(false) {
                continue;
            }

            let children_state = model.children(frame.id);
            let mut visible_children = match children_state {
                TreeChildren::Loaded(children) => self.visible_children(query, children),
                TreeChildren::Leaf | TreeChildren::Unloaded | TreeChildren::Loading => {
                    SmallVec::new()
                }
            };
            Self::sort_ids(model, query.sort(), &mut visible_children);

            let expansion = match children_state {
                TreeChildren::Leaf => TreeExpansionState::Leaf,
                TreeChildren::Unloaded => TreeExpansionState::Unloaded,
                TreeChildren::Loading => TreeExpansionState::Loading,
                TreeChildren::Loaded(_) if visible_children.is_empty() => TreeExpansionState::Leaf,
                TreeChildren::Loaded(_) => match query.filter_config() {
                    TreeFilterConfig::Enabled { auto_expand: true } => {
                        TreeExpansionState::ForcedByFilter
                    }
                    TreeFilterConfig::Disabled
                    | TreeFilterConfig::Enabled { auto_expand: false } => {
                        if is_expanded(frame.parent, frame.id) {
                            TreeExpansionState::Expanded
                        } else {
                            TreeExpansionState::Collapsed
                        }
                    }
                },
            };
            let match_state = if !filtering {
                TreeMatchState::Unfiltered
            } else if self.direct_matches.contains(&frame.id) {
                TreeMatchState::Direct
            } else {
                TreeMatchState::Ancestor
            };

            let index = self.nodes.len();
            self.nodes.push(ProjectedNode {
                id: frame.id,
                parent: frame.parent,
                parent_index: frame.parent_index,
                level: frame.level,
                is_last_sibling: frame.is_last_sibling,
                visible_child_count: visible_children.len(),
                expansion,
                match_state,
            });
            self.index.entry(frame.id).or_insert(index);

            if expansion.is_expanded() {
                Self::push_children(
                    &mut stack,
                    &visible_children,
                    Some(frame.id),
                    Some(index),
                    frame.level.saturating_add(1),
                );
            }
        }

        self.stamp = Some(Self::stamp(model, query, expansion_revision));
    }

    fn stamp<T, F, S>(
        model: &T,
        query: &TreeQuery<F, S>,
        expansion: TreeRevision,
    ) -> ProjectionStamp
    where
        T: TreeModel<Id = Id>,
    {
        ProjectionStamp {
            model: model.revision(),
            filter: PolicyStamp::new(query.filter_revision(), query.filter_generation()),
            sort: PolicyStamp::new(query.sort_revision(), query.sort_generation()),
            expansion,
            filter_config: query.filter_config(),
            root_visibility: query.root_visibility(),
        }
    }

    fn reserve(&mut self, hint: usize) {
        if hint == 0 {
            return;
        }
        reserve_to(&mut self.nodes, hint);
        reserve_map_to(&mut self.index, hint);
        reserve_map_to(&mut self.filter_memo, hint);
        let extra = hint.saturating_sub(self.direct_matches.len());
        self.direct_matches.reserve(extra);
    }

    fn visible_children<F, S>(
        &self,
        query: &TreeQuery<F, S>,
        children: &[Id],
    ) -> SmallVec<[Id; 8]> {
        match query.filter_config() {
            TreeFilterConfig::Disabled => children.iter().copied().collect(),
            TreeFilterConfig::Enabled { .. } => children
                .iter()
                .copied()
                .filter(|child| self.filter_memo.get(child).copied().unwrap_or(false))
                .collect(),
        }
    }

    fn compute_filter_matches<T, F>(&mut self, model: &T, filter: &F)
    where
        T: TreeModel<Id = Id>,
        F: TreeFilter<T>,
    {
        self.filter_memo.clear();
        self.direct_matches.clear();
        for node in TreePostorder::forest(model) {
            let direct = filter.is_match(model, node.id);
            if direct {
                self.direct_matches.insert(node.id);
            }
            let descendant = node
                .children
                .iter()
                .any(|child| self.filter_memo.get(child).copied().unwrap_or(false));
            self.filter_memo.insert(node.id, direct || descendant);
        }
    }

    fn sort_ids<T, S>(model: &T, sort: &S, ids: &mut [Id])
    where
        T: TreeModel<Id = Id>,
        S: TreeSort<T>,
    {
        if sort.is_enabled() {
            ids.sort_by(|left, right| sort.compare(model, *left, *right));
        }
    }

    pub(crate) fn occurrence_path(&self, index: usize) -> Option<OccurrencePath<Id>> {
        let mut ids = SmallVec::new();
        let mut cursor = Some(index);
        let mut root_parent = None;
        while let Some(index) = cursor {
            let node = self.nodes.get(index)?;
            ids.push(node.id);
            cursor = node.parent_index;
            if cursor.is_none() {
                root_parent = node.parent;
            }
        }
        ids.reverse();
        Some(OccurrencePath { root_parent, ids })
    }

    pub(crate) fn index_of_path(&self, path: &OccurrencePath<Id>) -> Option<usize> {
        self.index_of_path_prefix(path, path.len())
    }

    pub(crate) fn index_of_path_prefix(
        &self,
        path: &OccurrencePath<Id>,
        end: usize,
    ) -> Option<usize> {
        let ids = path.ids.get(..end)?;
        let (&id, _) = ids.split_last()?;
        let first = self.index_of(id)?;
        if self.path_matches(first, path.root_parent, ids) {
            return Some(first);
        }
        self.nodes[first + 1..]
            .iter()
            .enumerate()
            .filter(|(_, node)| node.id == id)
            .find_map(|(offset, _)| {
                let index = first + 1 + offset;
                self.path_matches(index, path.root_parent, ids)
                    .then_some(index)
            })
    }

    fn path_matches(&self, index: usize, root_parent: Option<Id>, ids: &[Id]) -> bool {
        let mut cursor = Some(index);
        let mut actual_root_parent = None;
        for &expected_id in ids.iter().rev() {
            let Some(node) = cursor.and_then(|index| self.nodes.get(index)) else {
                return false;
            };
            if node.id != expected_id {
                return false;
            }
            cursor = node.parent_index;
            actual_root_parent = node.parent;
        }
        cursor.is_none() && actual_root_parent == root_parent
    }

    fn push_children(
        stack: &mut Vec<ProjectionFrame<Id>>,
        children: &[Id],
        parent: Option<Id>,
        parent_index: Option<usize>,
        level: usize,
    ) {
        let last = children.len().saturating_sub(1);
        stack.extend(
            children
                .iter()
                .copied()
                .enumerate()
                .rev()
                .map(|(index, id)| ProjectionFrame {
                    id,
                    parent,
                    parent_index,
                    level,
                    is_last_sibling: index == last,
                }),
        );
    }
}

struct ProjectionFrame<Id> {
    id: Id,
    parent: Option<Id>,
    parent_index: Option<usize>,
    level: usize,
    is_last_sibling: bool,
}

fn reserve_to<T>(values: &mut Vec<T>, capacity: usize) {
    values.reserve(capacity.saturating_sub(values.len()));
}

fn reserve_map_to<K: Eq + Hash, V>(values: &mut FxHashMap<K, V>, capacity: usize) {
    values.reserve(capacity.saturating_sub(values.len()));
}
