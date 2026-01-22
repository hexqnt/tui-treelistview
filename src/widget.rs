use ratatui::layout::{Constraint, Rect};
use ratatui::prelude::Buffer;
use ratatui::widgets::{
    Block, Borders, Cell, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget,
    Table, TableState,
};
use smallvec::SmallVec;

use crate::columns::TreeColumns;
use crate::context::TreeRowContext;
use crate::glyphs::{TreeGlyphs, TreeLabelRenderer};
use crate::model::{NoFilter, TreeFilter, TreeFilterConfig, TreeModel};
use crate::state::{TreeListViewState, VisibleNode};
use crate::style::TreeListViewStyle;

/// Основной виджет дерева (table + stateful).
pub struct TreeListView<'a, T, L, C, F = NoFilter>
where
    T: TreeModel,
    L: TreeLabelRenderer<T>,
    C: TreeColumns<T>,
    F: TreeFilter<T>,
{
    model: &'a T,
    label: &'a L,
    columns: &'a C,
    style: TreeListViewStyle<'a>,
    glyphs: TreeGlyphs<'a>,
    filter: F,
    filter_config: TreeFilterConfig,
}

impl<'a, T, L, C> TreeListView<'a, T, L, C, NoFilter>
where
    T: TreeModel,
    L: TreeLabelRenderer<T>,
    C: TreeColumns<T>,
{
    pub const fn new(
        model: &'a T,
        label: &'a L,
        columns: &'a C,
        style: TreeListViewStyle<'a>,
    ) -> Self {
        Self {
            model,
            label,
            columns,
            style,
            glyphs: TreeGlyphs::unicode(),
            filter: NoFilter,
            filter_config: TreeFilterConfig::disabled(),
        }
    }

    pub const fn glyphs(mut self, glyphs: TreeGlyphs<'a>) -> Self {
        self.glyphs = glyphs;
        self
    }

    pub fn with_filter<F>(
        self,
        filter: F,
        filter_config: TreeFilterConfig,
    ) -> TreeListView<'a, T, L, C, F>
    where
        F: TreeFilter<T>,
    {
        TreeListView {
            model: self.model,
            label: self.label,
            columns: self.columns,
            style: self.style,
            glyphs: self.glyphs,
            filter,
            filter_config,
        }
    }
}

impl<'a, T, L, C, F> TreeListView<'a, T, L, C, F>
where
    T: TreeModel,
    L: TreeLabelRenderer<T>,
    C: TreeColumns<T>,
    F: TreeFilter<T>,
{
    #[inline]
    fn build_rows(
        &self,
        nodes: &[VisibleNode<T::Id>],
        state: &TreeListViewState<T::Id>,
    ) -> Vec<Row<'a>> {
        let mut rows = Vec::with_capacity(nodes.len());
        for node in nodes {
            let has_children = !self.model.children(node.id).is_empty();
            let is_expanded = state.is_expanded(node.parent, node.id);
            let is_marked = state.node_is_marked(node.id);
            let ctx = TreeRowContext {
                level: node.level,
                is_tail_stack: node.is_tail_stack.as_slice(),
                is_expanded,
                has_children,
                is_marked,
                draw_lines: state.draw_lines(),
                line_style: self.style.line_style,
            };
            let label_cell = self.label.cell(self.model, node.id, &ctx, &self.glyphs);
            let mut cells = SmallVec::<[Cell; 8]>::new();
            cells.push(label_cell);
            cells.extend(self.columns.cells(self.model, node.id));
            let mut row = Row::new(cells);
            if is_marked {
                row = row.style(self.style.mark_style);
            }
            rows.push(row);
        }
        rows
    }

    #[inline]
    fn build_table(
        &self,
        rows: Vec<Row<'a>>,
        constraints: &[Constraint],
        block: Block<'a>,
        header: Option<Row<'a>>,
    ) -> Table<'a> {
        let mut table = Table::new(rows, constraints.iter().copied())
            .style(self.style.block_style)
            .block(block)
            .row_highlight_style(self.style.highlight_style)
            .highlight_symbol(self.style.highlight_symbol);
        if let Some(header) = header {
            table = table.header(header);
        }
        table
    }

    #[inline]
    fn render_scrollbar(
        &self,
        area: Rect,
        buf: &mut Buffer,
        state: &TreeListViewState<T::Id>,
        inner_height: usize,
        scroll_rows: usize,
    ) {
        let scroll_len = scroll_rows.saturating_add(1);
        let position = state
            .list_state()
            .offset()
            .min(scroll_len.saturating_sub(1));
        let mut scrollbar_state = ScrollbarState::new(scroll_len)
            .position(position)
            .viewport_content_length(inner_height);
        Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .render(area, buf, &mut scrollbar_state);
    }
}

impl<T, L, C, F> StatefulWidget for TreeListView<'_, T, L, C, F>
where
    T: TreeModel,
    L: TreeLabelRenderer<T>,
    C: TreeColumns<T>,
    F: TreeFilter<T>,
{
    type State = TreeListViewState<T::Id>;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if self.filter_config.enabled {
            state.ensure_visible_nodes_filtered(self.model, &self.filter, self.filter_config);
        } else {
            state.ensure_visible_nodes(self.model);
        }
        state.ensure_mark_cache(self.model);

        let header = self.columns.header();
        let header_height = u16::from(header.is_some());

        let mut block = Block::default().borders(self.style.borders);
        if let Some(title) = self.style.title.clone() {
            block = block.title(title);
        }
        block = block
            .style(self.style.block_style)
            .border_style(self.style.border_style);

        let inner_height = block.inner(area).height.saturating_sub(header_height) as usize;
        state.ensure_selection_visible_with_policy(inner_height, self.style.scroll_policy);

        let visible_nodes = state.visible_nodes();
        let total_rows = visible_nodes.len();
        let (range_start, range_end) = if self.style.virtualize_rows {
            let start = state.list_state().offset().min(total_rows);
            let end = (start + inner_height).min(total_rows);
            (start, end)
        } else {
            (0, total_rows)
        };

        let nodes = &visible_nodes[range_start..range_end];
        let rows = self.build_rows(nodes, state);

        let scroll_rows = total_rows.saturating_sub(inner_height);

        let mut local_state = if self.style.virtualize_rows {
            Some(*state.list_state())
        } else {
            None
        };
        let table_state: &mut TableState = local_state.as_mut().map_or_else(
            || state.list_state_mut(),
            |state_ref| {
                *state_ref.offset_mut() = 0;
                if let Some(selected) = state_ref.selected() {
                    if selected < range_start || selected >= range_end {
                        state_ref.select(None);
                    } else {
                        state_ref.select(Some(selected - range_start));
                    }
                }
                state_ref
            },
        );

        let (table_area, table_block, constraints, header, scrollbar_area) = if scroll_rows > 0 {
            let table_area = Rect {
                width: area.width.saturating_sub(1),
                ..area
            };
            let scrollbar_area = Rect {
                x: area.x + area.width - 1,
                y: area.y,
                width: 1,
                height: area.height,
            };
            let mut table_borders = self.style.borders;
            table_borders.remove(Borders::RIGHT);
            let table_block = block.borders(table_borders);
            let constraints = self
                .columns
                .constraints_for_area(table_block.inner(table_area));
            (
                table_area,
                table_block,
                constraints,
                header.clone(),
                Some(scrollbar_area),
            )
        } else {
            let constraints = self.columns.constraints_for_area(block.inner(area));
            (area, block, constraints, header, None)
        };

        let table = self.build_table(rows, constraints.as_slice(), table_block, header);
        table.render(table_area, buf, table_state);

        if let Some(scrollbar_area) = scrollbar_area {
            self.render_scrollbar(scrollbar_area, buf, state, inner_height, scroll_rows);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Constraint;
    use ratatui::widgets::StatefulWidget;

    struct TestModel {
        children: Vec<Vec<usize>>,
        names: Vec<String>,
    }

    impl TestModel {
        fn new(child_count: usize) -> Self {
            let mut children = vec![Vec::new(); child_count + 1];
            let mut names = Vec::with_capacity(child_count + 1);
            names.push("root".to_string());
            for idx in 1..=child_count {
                children[0].push(idx);
                names.push(format!("node-{idx}"));
            }
            Self { children, names }
        }
    }

    impl TreeModel for TestModel {
        type Id = usize;

        fn root(&self) -> Option<Self::Id> {
            Some(0)
        }

        fn children(&self, id: Self::Id) -> &[Self::Id] {
            &self.children[id]
        }

        fn contains(&self, id: Self::Id) -> bool {
            id < self.children.len()
        }
    }

    struct Label;

    impl TreeLabelRenderer<TestModel> for Label {
        fn cell<'a>(
            &'a self,
            model: &'a TestModel,
            id: usize,
            _ctx: &TreeRowContext,
            _glyphs: &TreeGlyphs<'a>,
        ) -> Cell<'a> {
            Cell::from(model.names[id].as_str())
        }
    }

    struct Columns;

    impl TreeColumns<TestModel> for Columns {
        fn label_constraint(&self) -> Constraint {
            Constraint::Percentage(100)
        }

        fn other_constraints(&self) -> &[Constraint] {
            &[]
        }

        fn cells<'a>(&'a self, _model: &'a TestModel, _id: usize) -> SmallVec<[Cell<'a>; 8]> {
            SmallVec::new()
        }
    }

    #[test]
    fn render_smoke_with_scrollbar() {
        let model = TestModel::new(12);
        let label = Label;
        let columns = Columns;
        let style = TreeListViewStyle::default();
        let widget = TreeListView::new(&model, &label, &columns, style);

        let mut state = TreeListViewState::new();
        state.set_expanded(0, None, true);

        let area = Rect::new(0, 0, 20, 6);
        let mut buffer = Buffer::empty(area);

        widget.render(area, &mut buffer, &mut state);
    }
}
