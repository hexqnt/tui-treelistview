use std::hash::Hash;

/// Minimal tree contract required by the widget.
///
/// A proper tree is expected (not a DAG):
/// - no cycles (DFS traversal is used directly);
/// - each node has exactly one parent;
/// - identifiers are stable between frames (for selection/expansion).
pub trait TreeModel {
    /// Node identifier type.
    type Id: Copy + Eq + Hash;

    /// Returns the root node (or `None` if the tree is empty).
    fn root(&self) -> Option<Self::Id>;
    /// Returns the node's children in a deterministic order.
    fn children(&self, id: Self::Id) -> &[Self::Id];
    /// Returns `true` if the node exists in the model.
    fn contains(&self, id: Self::Id) -> bool;
    /// Returns an approximate size hint (not required to be exact).
    fn size_hint(&self) -> usize {
        0
    }
}

/// Visibility filter for nodes (used to build a reduced list).
pub trait TreeFilter<T: TreeModel> {
    /// Returns `true` if the node matches the filter criteria.
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

/// Configuration for filtered rendering.
#[derive(Clone, Copy, Debug)]
pub struct TreeFilterConfig {
    /// Enables or disables filtering.
    pub enabled: bool,
    /// Auto-expands matching paths when filtering.
    pub auto_expand: bool,
}

impl TreeFilterConfig {
    /// Creates a configuration with filtering disabled.
    pub const fn disabled() -> Self {
        Self {
            enabled: false,
            auto_expand: false,
        }
    }

    /// Creates a configuration with filtering enabled and auto-expansion.
    pub const fn enabled() -> Self {
        Self {
            enabled: true,
            auto_expand: true,
        }
    }
}

impl Default for TreeFilterConfig {
    fn default() -> Self {
        Self::disabled()
    }
}

/// Filter that matches every node.
#[derive(Clone, Copy, Debug)]
pub struct NoFilter;

impl<T: TreeModel> TreeFilter<T> for NoFilter {
    #[inline]
    fn is_match(&self, _model: &T, _id: T::Id) -> bool {
        true
    }
}
