use std::cmp::Ordering;
use std::hash::Hash;
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

static NEXT_QUERY_POLICY_GENERATION: AtomicU64 = AtomicU64::new(1);

/// The state of a node's child list.
///
/// Unlike an empty slice, `Unloaded` and `Loading` preserve the fact that a node is a branch
/// whose children may be loaded asynchronously.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeChildren<'a, Id> {
    /// The node is known to be a leaf.
    Leaf,
    /// Children exist or may exist, but have not been loaded yet.
    Unloaded,
    /// Children are currently loading.
    Loading,
    /// Children are loaded and exposed as a stable slice.
    Loaded(&'a [Id]),
}

impl<'a, Id> TreeChildren<'a, Id> {
    /// Creates a loaded state, converting an empty slice into a leaf.
    #[must_use]
    pub const fn loaded(children: &'a [Id]) -> Self {
        if children.is_empty() {
            Self::Leaf
        } else {
            Self::Loaded(children)
        }
    }

    /// Returns the loaded children or an empty slice.
    #[must_use]
    pub const fn loaded_slice(self) -> &'a [Id] {
        match self {
            Self::Loaded(children) => children,
            Self::Leaf | Self::Unloaded | Self::Loading => &[],
        }
    }

    /// Returns `true` when the node is a potentially expandable branch.
    #[must_use]
    pub const fn is_branch(self) -> bool {
        !matches!(self, Self::Leaf)
    }
}

/// Controls how roots are projected.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TreeRootVisibility {
    /// Every root is displayed as a regular row.
    #[default]
    Visible,
    /// Roots are synthetic; their loaded children are displayed at level `0`.
    Hidden,
}

/// Selection policy used when the selected node disappears from the projection.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TreeSelectionFallback {
    /// Prefer the nearest visible ancestor, then the nearest row.
    #[default]
    ParentThenNearest,
    /// Select the row nearest to the previous index.
    Nearest,
    /// Clear the selection.
    Clear,
}

/// Tree filtering configuration.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum TreeFilterConfig {
    /// Filtering is disabled.
    #[default]
    Disabled,
    /// Keep matching nodes and the paths leading to them.
    Enabled {
        /// Force filtered paths to expand.
        auto_expand: bool,
    },
}

impl TreeFilterConfig {
    /// Enables filtering with automatic path expansion.
    #[must_use]
    pub const fn enabled() -> Self {
        Self::Enabled { auto_expand: true }
    }

    /// Enables filtering with manual path expansion.
    #[must_use]
    pub const fn enabled_manual_expand() -> Self {
        Self::Enabled { auto_expand: false }
    }
}

/// A monotonically increasing revision of model, filter, or sorting data.
///
/// The projection cache is rebuilt whenever any participating revision changes. A
/// [`TreeModel`] implementation must return a new value after changing its contents.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TreeRevision(u64);

impl TreeRevision {
    /// The initial revision of an immutable value.
    pub const INITIAL: Self = Self(0);

    /// Creates a revision from an external counter.
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the numeric revision value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Returns the next revision.
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }

    /// Advances the counter to the next revision.
    pub const fn advance(&mut self) {
        *self = self.next();
    }
}

impl From<u64> for TreeRevision {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

/// A filter that matches every node.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoFilter;

impl<T: TreeModel> TreeFilter<T> for NoFilter {
    #[inline]
    fn is_match(&self, _model: &T, _id: T::Id) -> bool {
        true
    }
}

/// No sorting; preserves model order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct NoSort;

impl<T: TreeModel> TreeSort<T> for NoSort {
    fn compare(&self, _model: &T, _left: T::Id, _right: T::Id) -> Ordering {
        Ordering::Equal
    }

    fn is_enabled(&self) -> bool {
        false
    }
}

/// A complete query for building the visible projection.
#[derive(Clone, Debug)]
pub struct TreeQuery<F = NoFilter, S = NoSort> {
    filter: QueryPolicy<F>,
    sort: QueryPolicy<S>,
    filter_config: TreeFilterConfig,
    root_visibility: TreeRootVisibility,
    selection_fallback: TreeSelectionFallback,
}

impl TreeQuery {
    /// Creates a query without filtering or sorting.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            filter: QueryPolicy::new(NoFilter, TreeRevision::INITIAL),
            sort: QueryPolicy::new(NoSort, TreeRevision::INITIAL),
            filter_config: TreeFilterConfig::Disabled,
            root_visibility: TreeRootVisibility::Visible,
            selection_fallback: TreeSelectionFallback::ParentThenNearest,
        }
    }
}

impl<F, S> TreeQuery<F, S> {
    /// Sets the filter and its current revision.
    #[must_use]
    pub fn with_filter<NF>(
        self,
        filter: NF,
        config: TreeFilterConfig,
        revision: TreeRevision,
    ) -> TreeQuery<NF, S> {
        TreeQuery {
            filter: QueryPolicy::replacement(filter, revision),
            sort: self.sort,
            filter_config: config,
            root_visibility: self.root_visibility,
            selection_fallback: self.selection_fallback,
        }
    }

    /// Sets the sorting policy and its current revision.
    #[must_use]
    pub fn with_sort<NS>(self, sort: NS, revision: TreeRevision) -> TreeQuery<F, NS> {
        TreeQuery {
            filter: self.filter,
            sort: QueryPolicy::replacement(sort, revision),
            filter_config: self.filter_config,
            root_visibility: self.root_visibility,
            selection_fallback: self.selection_fallback,
        }
    }

    /// Sets the filtering mode while preserving the filter itself.
    #[must_use]
    pub const fn with_filter_config(mut self, config: TreeFilterConfig) -> Self {
        self.filter_config = config;
        self
    }

    /// Sets how roots are displayed.
    #[must_use]
    pub const fn with_root_visibility(mut self, visibility: TreeRootVisibility) -> Self {
        self.root_visibility = visibility;
        self
    }

    /// Sets the selection fallback policy.
    #[must_use]
    pub const fn with_selection_fallback(mut self, fallback: TreeSelectionFallback) -> Self {
        self.selection_fallback = fallback;
        self
    }

    /// Returns the filter policy.
    #[must_use]
    pub const fn filter(&self) -> &F {
        &self.filter.value
    }

    /// Returns the mutable filter and automatically advances its revision.
    pub const fn filter_mut(&mut self) -> &mut F {
        self.filter.value_mut()
    }

    /// Returns the sibling sorting policy.
    #[must_use]
    pub const fn sort(&self) -> &S {
        &self.sort.value
    }

    /// Returns the mutable sorting policy and automatically advances its revision.
    pub const fn sort_mut(&mut self) -> &mut S {
        self.sort.value_mut()
    }

    /// Explicitly advances the filter revision, for example after captured data changes.
    pub const fn touch_filter(&mut self) {
        self.filter.touch();
    }

    /// Explicitly advances the sort revision, for example after captured data changes.
    pub const fn touch_sort(&mut self) {
        self.sort.touch();
    }

    /// Changes the filtering mode while preserving the filter itself.
    pub fn set_filter_config(&mut self, config: TreeFilterConfig) -> bool {
        if self.filter_config == config {
            return false;
        }
        self.filter_config = config;
        true
    }

    /// Changes how roots are displayed.
    pub fn set_root_visibility(&mut self, visibility: TreeRootVisibility) -> bool {
        let changed = self.root_visibility != visibility;
        self.root_visibility = visibility;
        changed
    }

    /// Changes the selection fallback policy.
    pub fn set_selection_fallback(&mut self, fallback: TreeSelectionFallback) -> bool {
        let changed = self.selection_fallback != fallback;
        self.selection_fallback = fallback;
        changed
    }

    /// Returns the current filtering mode.
    #[must_use]
    pub const fn filter_config(&self) -> TreeFilterConfig {
        self.filter_config
    }

    /// Returns the current root display mode.
    #[must_use]
    pub const fn root_visibility(&self) -> TreeRootVisibility {
        self.root_visibility
    }

    /// Returns the current selection fallback policy.
    #[must_use]
    pub const fn selection_fallback(&self) -> TreeSelectionFallback {
        self.selection_fallback
    }

    /// Returns the current filter-data revision.
    #[must_use]
    pub const fn filter_revision(&self) -> TreeRevision {
        self.filter.revision
    }

    /// Returns the current sort-data revision.
    #[must_use]
    pub const fn sort_revision(&self) -> TreeRevision {
        self.sort.revision
    }

    pub(crate) const fn filter_generation(&self) -> TreeRevision {
        self.filter.generation
    }

    pub(crate) const fn sort_generation(&self) -> TreeRevision {
        self.sort.generation
    }
}

impl Default for TreeQuery {
    fn default() -> Self {
        Self::new()
    }
}
#[derive(Clone, Debug)]
struct QueryPolicy<P> {
    value: P,
    revision: TreeRevision,
    generation: TreeRevision,
}

impl<P> QueryPolicy<P> {
    const fn new(value: P, revision: TreeRevision) -> Self {
        Self {
            value,
            revision,
            generation: TreeRevision::INITIAL,
        }
    }

    fn replacement(value: P, revision: TreeRevision) -> Self {
        Self {
            value,
            revision,
            generation: next_query_policy_generation(),
        }
    }

    const fn value_mut(&mut self) -> &mut P {
        self.revision.advance();
        &mut self.value
    }

    const fn touch(&mut self) {
        self.revision.advance();
    }
}

/// Минимальный контракт источника дерева, леса или корневого ациклического графа.
///
/// Общие дочерние вершины допустимы и создают отдельные вхождения видимых строк. Циклы, повторные
/// корни и повторные идентификаторы в одном списке детей недопустимы. Идентификаторы должны быть
/// стабильными и дешёвыми для копирования. Каждый идентификатор из `roots` или `children` должен
/// оставаться корректным для последующих вызовов методов модели.
pub trait TreeModel {
    /// The node identifier type.
    type Id: Copy + Eq + Hash;

    /// Returns forest roots in deterministic order.
    fn roots(&self) -> impl Iterator<Item = Self::Id> + '_;

    /// Returns the node's child state and loaded children.
    fn children(&self, id: Self::Id) -> TreeChildren<'_, Self::Id>;

    /// Returns the revision of the model structure and display data.
    fn revision(&self) -> TreeRevision;

    /// Returns an approximate number of available nodes.
    fn size_hint(&self) -> usize {
        0
    }
}

/// A node visibility filter.
pub trait TreeFilter<T: TreeModel> {
    /// Returns `true` when the node directly matches the filter.
    fn is_match(&self, model: &T, id: T::Id) -> bool;
}

impl<T, F> TreeFilter<T> for F
where
    T: TreeModel,
    F: Fn(&T, T::Id) -> bool,
{
    #[inline]
    fn is_match(&self, model: &T, id: T::Id) -> bool {
        self(model, id)
    }
}

/// A policy for sorting sibling nodes.
pub trait TreeSort<T: TreeModel> {
    /// Compares two sibling nodes.
    fn compare(&self, model: &T, left: T::Id, right: T::Id) -> Ordering;

    /// Returns `true` when sorting should be applied.
    fn is_enabled(&self) -> bool {
        true
    }
}

impl<T, F> TreeSort<T> for F
where
    T: TreeModel,
    F: Fn(&T, T::Id, T::Id) -> Ordering,
{
    fn compare(&self, model: &T, left: T::Id, right: T::Id) -> Ordering {
        self(model, left, right)
    }
}

fn next_query_policy_generation() -> TreeRevision {
    TreeRevision::new(NEXT_QUERY_POLICY_GENERATION.fetch_add(1, AtomicOrdering::Relaxed))
}
