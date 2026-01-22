//! Tree list view widget for ratatui with filtering, marking, and optional key/edit handling.
//!
//! Feature flags:
//! - `keymap`: crossterm-based key bindings and `TreeListViewState::handle_key*` helpers.
//! - `edit`: `TreeEdit` trait and edit-action helpers.
//! - `serde`: serde support for `TreeListViewSnapshot`.

mod action;
mod columns;
mod context;
#[cfg(feature = "edit")]
mod edit;
mod glyphs;
#[cfg(feature = "keymap")]
mod keymap;
mod model;
pub mod prelude;
mod state;
mod style;
mod widget;

pub use action::{TreeAction, TreeEvent};
pub use columns::{
    AdaptiveColumns, ColumnDef, ColumnFn, ColumnWidth, SimpleColumns, TreeColumns,
    TreeColumnsLayout, distribute_widths,
};
pub use context::TreeRowContext;
#[cfg(feature = "edit")]
pub use edit::TreeEdit;
pub use glyphs::{
    TreeGlyphs, TreeLabelPrefix, TreeLabelProvider, TreeLabelRenderer, tree_label_line,
    tree_name_cell,
};
#[cfg(feature = "keymap")]
pub use keymap::{KeymapProfile, TreeKeyBindings};
pub use model::{NoFilter, TreeFilter, TreeFilterConfig, TreeModel};
pub use state::{TreeListViewSnapshot, TreeListViewState};
pub use style::{TreeListViewStyle, TreeScrollPolicy};
pub use widget::TreeListView;
