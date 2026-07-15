use ratatui::style::Style;

/// A node's effective expansion state in the current projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeExpansionState {
    Leaf,
    Collapsed,
    Expanded,
    ForcedByFilter,
    Unloaded,
    Loading,
}

impl TreeExpansionState {
    /// Returns `true` when the node's descendants are currently visible.
    #[must_use]
    pub const fn is_expanded(self) -> bool {
        matches!(self, Self::Expanded | Self::ForcedByFilter)
    }

    /// Returns `true` when the node accepts an expand action.
    #[must_use]
    pub const fn is_expandable(self) -> bool {
        !matches!(self, Self::Leaf | Self::Loading)
    }
}

/// A node's role in a filtered projection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeMatchState {
    Unfiltered,
    Direct,
    Ancestor,
}

/// A node's aggregated mark state.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TreeMarkState {
    #[default]
    Unmarked,
    Partial,
    Marked,
}

/// Node state available to row renderers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TreeRowNodeState {
    pub expansion: TreeExpansionState,
    pub mark: TreeMarkState,
    pub match_state: TreeMatchState,
}

/// View state available to row renderers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TreeRowRenderState {
    pub draw_lines: bool,
    pub is_selected: bool,
    pub selected_column: Option<usize>,
}

/// Context for rendering one tree row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TreeRowContext<'a> {
    /// Node depth, with roots at level `0`.
    pub level: usize,
    /// For each path level, indicates whether that node is the last sibling.
    pub is_tail_stack: &'a [bool],
    pub node: TreeRowNodeState,
    pub render: TreeRowRenderState,
    pub line_style: Style,
}
