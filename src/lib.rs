//! An interactive, model-agnostic Ratatui tree table.
//!
//! [`TreeModel`] borrows a forest from application-owned data, [`TreeQuery`] describes filtering
//! and sorting, and [`TreeListViewState`] owns stable UI state plus the derived [`TreeProjection`]
//! cache. [`TreeListView`] uses that same projection for viewport-only rendering, ensuring input
//! and display always agree.
//!
//! The model API supports stable generic IDs, multiple roots, asynchronous child states, and
//! explicit revisions. The view adds typed actions and intents, validated dynamic columns,
//! two-dimensional scrolling, tri-state marks, snapshots, and hit testing.
//!
//! Feature flags:
//! - `keymap`: crossterm-based key bindings and `TreeListViewState::handle_key*` helpers.
//! - `serde`: serde support for `TreeListViewSnapshot`.

#![allow(clippy::multiple_crate_versions)]

pub use action::{
    TreeAction, TreeEditAction, TreeEditRequest, TreeEvent, TreeIntent, TreeViewAction,
};
pub use adapters::{IndexedTree, IndexedTreeError, TreeModelRef};
pub use columns::{
    ColumnDef, ColumnWidth, ColumnWidthError, TreeCellRenderer, TreeColumnSet, TreeColumns,
    TreeColumnsError, distribute_widths,
};
pub use context::{
    TreeExpansionState, TreeMarkState, TreeMatchState, TreeRowContext, TreeRowNodeState,
    TreeRowRenderState,
};
pub use edit::{
    TreeChangeSet, TreeEditCommand, TreeEditor, TreeInsertPosition, TreeSelectionUpdate,
};
pub use glyphs::{
    TreeGlyphs, TreeLabelPrefix, TreeLabelProvider, TreeLabelRenderer, tree_label_line,
    tree_name_cell,
};
#[cfg(feature = "keymap")]
pub use keymap::{KeymapProfile, TreeKeyBindings};
pub use model::{
    NoFilter, NoSort, TreeChildren, TreeFilter, TreeFilterConfig, TreeModel, TreeQuery,
    TreeRevision, TreeRootVisibility, TreeSelectionFallback, TreeSort,
};
pub use projection::{ProjectedNode, TreeProjection};
pub use state::{TreeHit, TreeHitRegion, TreeListViewSnapshot, TreeListViewState};
pub use style::{TreeHorizontalScroll, TreeListViewStyle, TreeRowRendering, TreeScrollPolicy};
pub use widget::TreeListView;

mod action;
mod adapters;
mod columns;
mod context;
mod edit;
mod glyphs;
#[cfg(feature = "keymap")]
mod keymap;
mod model;
pub mod prelude;
mod projection;
mod state;
mod style;
mod traversal;
mod widget;
