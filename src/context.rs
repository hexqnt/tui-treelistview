use ratatui::style::Style;

#[derive(Clone, Copy)]
pub struct TreeRowContext<'a> {
    pub level: u16,
    pub is_tail_stack: &'a [bool],
    pub is_expanded: bool,
    pub has_children: bool,
    pub is_marked: bool,
    pub draw_lines: bool,
    pub line_style: Style,
}
