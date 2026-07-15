use std::hash::Hash;
use std::ops::Deref;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

use crate::context::TreeMarkState;
use crate::model::TreeRevision;
use crate::projection::{ProjectedNode, TreeProjection};

pub use hit::{TreeHit, TreeHitRegion};

mod actions;
pub mod hit;
mod marks;
mod navigation;
mod visibility;

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

struct RevisionedSet<T> {
    values: FxHashSet<T>,
    revision: TreeRevision,
}

impl<T: Eq + Hash> RevisionedSet<T> {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            values: FxHashSet::with_capacity_and_hasher(capacity, FxBuildHasher),
            revision: TreeRevision::INITIAL,
        }
    }

    const fn revision(&self) -> TreeRevision {
        self.revision
    }

    fn mutate(&mut self, mutation: impl FnOnce(&mut FxHashSet<T>) -> bool) -> bool {
        let changed = mutation(&mut self.values);
        if changed {
            self.revision.advance();
        }
        changed
    }

    fn set_membership(&mut self, value: T, present: bool) -> bool {
        self.mutate(|values| {
            if present {
                values.insert(value)
            } else {
                values.remove(&value)
            }
        })
    }

    fn clear(&mut self) -> bool {
        self.mutate(|values| {
            if values.is_empty() {
                false
            } else {
                values.clear();
                true
            }
        })
    }

    fn retain(&mut self, mut keep: impl FnMut(&T) -> bool) -> bool {
        self.mutate(|values| {
            let old_len = values.len();
            values.retain(&mut keep);
            values.len() != old_len
        })
    }

    fn replace(&mut self, values: FxHashSet<T>) -> bool {
        self.mutate(|current| {
            if *current == values {
                false
            } else {
                *current = values;
                true
            }
        })
    }
}

impl<T> Deref for RevisionedSet<T> {
    type Target = FxHashSet<T>;

    fn deref(&self) -> &Self::Target {
        &self.values
    }
}

/// Persistent view state and its derived caches.
pub struct TreeListViewState<Id> {
    projection: TreeProjection<Id>,
    selected: Option<Id>,
    selected_row: Option<usize>,
    selection_needs_visibility: bool,
    offset: usize,
    selected_column: Option<usize>,
    column_needs_visibility: bool,
    horizontal_offset: u16,
    expanded: RevisionedSet<ExpansionPath<Id>>,
    manual_marked: RevisionedSet<Id>,
    mark_states: FxHashMap<Id, TreeMarkState>,
    mark_stamp: Option<(TreeRevision, TreeRevision)>,
    draw_lines: bool,
    pub(crate) hit_map: hit::TreeHitMap,
    pub(crate) render_buffer: Buffer,
    #[cfg(feature = "keymap")]
    keymap: crate::keymap::TreeKeyBindings,
}

impl<Id: Copy + Eq + Hash> TreeListViewState<Id> {
    /// Creates empty view state.
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    /// Creates view state with preallocated projection storage.
    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            projection: TreeProjection::with_capacity(capacity),
            selected: None,
            selected_row: None,
            selection_needs_visibility: false,
            offset: 0,
            selected_column: None,
            column_needs_visibility: false,
            horizontal_offset: 0,
            expanded: RevisionedSet::with_capacity(capacity),
            manual_marked: RevisionedSet::with_capacity(capacity),
            mark_states: FxHashMap::with_capacity_and_hasher(capacity, FxBuildHasher),
            mark_stamp: None,
            draw_lines: true,
            hit_map: hit::TreeHitMap::default(),
            render_buffer: Buffer::empty(Rect::ZERO),
            #[cfg(feature = "keymap")]
            keymap: crate::keymap::TreeKeyBindings::new(),
        }
    }

    /// Restores state from a snapshot.
    #[must_use]
    pub fn from_snapshot(snapshot: TreeListViewSnapshot<Id>) -> Self {
        let mut state = Self::new();
        state.restore(snapshot);
        state
    }

    /// Returns the current projection. Before reading it, the application must call
    /// [`TreeListViewState::ensure_projection`] or render the widget.
    #[must_use]
    pub const fn projection(&self) -> &TreeProjection<Id> {
        &self.projection
    }

    /// Captures the persistent part of the state.
    #[must_use]
    pub fn snapshot(&self) -> TreeListViewSnapshot<Id> {
        TreeListViewSnapshot {
            expanded: self
                .expanded
                .iter()
                .map(|path| (path.parent, path.id))
                .collect(),
            manual_marked: self.manual_marked.iter().copied().collect(),
            selected: self.selected,
            selected_column: self.selected_column,
            offset: self.offset,
            horizontal_offset: self.horizontal_offset,
            draw_lines: self.draw_lines,
        }
    }

    /// Restores persistent state and resets derived caches.
    pub fn restore(&mut self, snapshot: TreeListViewSnapshot<Id>) {
        self.expanded.replace(
            snapshot
                .expanded
                .into_iter()
                .map(|(parent, id)| ExpansionPath::new(parent, id))
                .collect(),
        );
        self.manual_marked
            .replace(snapshot.manual_marked.into_iter().collect());
        self.selected = snapshot.selected;
        self.selected_row = None;
        self.selection_needs_visibility = self.selected.is_some();
        self.selected_column = snapshot.selected_column;
        self.column_needs_visibility = self.selected_column.is_some();
        self.offset = snapshot.offset;
        self.horizontal_offset = snapshot.horizontal_offset;
        self.draw_lines = snapshot.draw_lines;
    }

    #[must_use]
    pub const fn draw_lines(&self) -> bool {
        self.draw_lines
    }

    pub const fn set_draw_lines(&mut self, draw: bool) {
        self.draw_lines = draw;
    }

    pub(crate) fn is_expanded(&self, parent: Option<Id>, id: Id) -> bool {
        self.expanded.contains(&ExpansionPath::new(parent, id))
    }

    pub(crate) fn mark_state_cached(&self, id: Id) -> TreeMarkState {
        self.mark_states.get(&id).copied().unwrap_or_default()
    }

    pub(crate) fn selected_node(&self) -> Option<ProjectedNode<Id>> {
        let selected = self.selected?;
        self.selected_row
            .and_then(|index| self.projection.nodes().get(index))
            .copied()
            .filter(|node| node.id() == selected)
    }

    #[cfg(feature = "keymap")]
    /// Returns the mutable key bindings.
    pub const fn keymap_mut(&mut self) -> &mut crate::keymap::TreeKeyBindings {
        &mut self.keymap
    }
}

impl<Id: Copy + Eq + Hash> Default for TreeListViewState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

/// The serializable persistent part of view state.
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeListViewSnapshot<Id> {
    pub expanded: Vec<(Option<Id>, Id)>,
    pub manual_marked: Vec<Id>,
    pub selected: Option<Id>,
    pub selected_column: Option<usize>,
    pub offset: usize,
    pub horizontal_offset: u16,
    pub draw_lines: bool,
}
