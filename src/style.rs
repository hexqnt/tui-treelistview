use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::Borders;

/// Scroll policy used to keep the selection visible.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeScrollPolicy {
    /// Keep the selection visible; only scroll when it would leave the viewport.
    KeepInView,
    /// Keep the selection centered within the viewport.
    CenterOnSelect,
}

/// Visual settings for the tree list view widget.
#[derive(Clone)]
pub struct TreeListViewStyle<'a> {
    /// Optional title displayed in the surrounding block.
    pub title: Option<Line<'a>>,
    /// Style applied to the outer block.
    pub block_style: Style,
    /// Style applied to the block borders.
    pub border_style: Style,
    /// Style applied to the highlighted (selected) row.
    pub highlight_style: Style,
    /// Style applied to marked rows.
    pub mark_style: Style,
    /// Style applied to tree guide lines.
    pub line_style: Style,
    /// Symbol rendered before the selected row.
    pub highlight_symbol: &'a str,
    /// Borders to render around the widget.
    pub borders: Borders,
    /// Render only the visible slice of rows for large trees.
    pub virtualize_rows: bool,
    /// Scroll behavior for keeping the selection visible.
    pub scroll_policy: TreeScrollPolicy,
}

impl Default for TreeListViewStyle<'_> {
    fn default() -> Self {
        Self {
            title: None,
            block_style: Style::default(),
            border_style: Style::default(),
            highlight_style: Style::default(),
            mark_style: Style::default(),
            line_style: Style::default(),
            highlight_symbol: ">> ",
            borders: Borders::ALL,
            virtualize_rows: false,
            scroll_policy: TreeScrollPolicy::KeepInView,
        }
    }
}
