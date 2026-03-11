use ratatui::style::Style;

/// Node-specific flags used while rendering a row.
#[derive(Clone, Copy)]
pub struct TreeRowNodeState {
    /// Whether the node is currently expanded.
    pub is_expanded: bool,
    /// Whether the node has children.
    pub has_children: bool,
    /// Whether the node is marked (directly or via its subtree).
    pub is_marked: bool,
}

/// Rendering flags that affect visual presentation.
#[derive(Clone, Copy)]
pub struct TreeRowRenderState {
    /// Whether guide lines should be rendered.
    pub draw_lines: bool,
}

/// Rendering context for a single tree row.
#[derive(Clone, Copy)]
pub struct TreeRowContext<'a> {
    /// Depth level of the node in the tree (root = 0).
    pub level: u16,
    /// Stack indicating whether each level on the path is the last child.
    pub is_tail_stack: &'a [bool],
    /// Node flags.
    pub node: TreeRowNodeState,
    /// Render flags.
    pub render: TreeRowRenderState,
    /// Style applied to guide line segments.
    pub line_style: Style,
}
