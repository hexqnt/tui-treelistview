use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::Borders;

/// Policy for keeping the selection in the vertical viewport.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TreeScrollPolicy {
    #[default]
    KeepInView,
    CenterOnSelect,
}

/// Strategy for building table rows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TreeRowRendering {
    Full,
    #[default]
    Virtualized,
}

/// Horizontal layout policy.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TreeHorizontalScroll {
    Disabled,
    #[default]
    Enabled,
}

/// Visual tree configuration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeListViewStyle<'a> {
    pub title: Option<Line<'a>>,
    pub block_style: Style,
    pub border_style: Style,
    pub highlight_style: Style,
    pub column_highlight_style: Style,
    pub cell_highlight_style: Style,
    pub marked_style: Style,
    pub partial_mark_style: Style,
    pub direct_match_style: Style,
    pub ancestor_match_style: Style,
    pub line_style: Style,
    pub highlight_symbol: &'a str,
    pub borders: Borders,
    pub column_spacing: u16,
    pub row_rendering: TreeRowRendering,
    pub horizontal_scroll: TreeHorizontalScroll,
    pub scroll_policy: TreeScrollPolicy,
}

impl TreeListViewStyle<'_> {
    /// Creates a style without an outer border.
    #[must_use]
    pub fn borderless() -> Self {
        Self {
            borders: Borders::NONE,
            ..Self::default()
        }
    }
}

impl Default for TreeListViewStyle<'_> {
    fn default() -> Self {
        Self {
            title: None,
            block_style: Style::default(),
            border_style: Style::default(),
            highlight_style: Style::default(),
            column_highlight_style: Style::default(),
            cell_highlight_style: Style::default(),
            marked_style: Style::default(),
            partial_mark_style: Style::default(),
            direct_match_style: Style::default(),
            ancestor_match_style: Style::default(),
            line_style: Style::default(),
            highlight_symbol: ">> ",
            borders: Borders::ALL,
            column_spacing: 1,
            row_rendering: TreeRowRendering::Virtualized,
            horizontal_scroll: TreeHorizontalScroll::Enabled,
            scroll_policy: TreeScrollPolicy::KeepInView,
        }
    }
}
