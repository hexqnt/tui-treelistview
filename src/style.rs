use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::Borders;

/// Политика скролла при изменении выбранной строки.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeScrollPolicy {
    KeepInView,
    CenterOnSelect,
}

/// Визуальные настройки виджета дерева.
#[derive(Clone)]
pub struct TreeListViewStyle<'a> {
    pub title: Option<Line<'a>>,
    pub block_style: Style,
    pub border_style: Style,
    pub highlight_style: Style,
    pub mark_style: Style,
    pub line_style: Style,
    pub highlight_symbol: &'a str,
    pub borders: Borders,
    pub virtualize_rows: bool,
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
