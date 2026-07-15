use ratatui::layout::{Constraint, Rect};
use ratatui::prelude::Buffer;
use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{
    Block, HighlightSpacing, Row, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget,
    Table, TableState, Widget,
};
use smallvec::SmallVec;

use crate::columns::TreeColumns;
use crate::context::{
    TreeMarkState, TreeMatchState, TreeRowContext, TreeRowNodeState, TreeRowRenderState,
};
use crate::glyphs::{TreeGlyphs, TreeLabelRenderer};
use crate::model::{TreeFilter, TreeModel, TreeQuery, TreeSort};
use crate::projection::{ProjectedNode, TreeProjection};
use crate::state::TreeListViewState;
use crate::state::hit::{ColumnHitBox, TreeHitMap};
use crate::style::{TreeHorizontalScroll, TreeListViewStyle, TreeRowRendering};

/// A stateful tree table built around one projection shared by rendering and navigation.
pub struct TreeListView<'a, T, F, S, L, C> {
    model: &'a T,
    query: &'a TreeQuery<F, S>,
    label: &'a L,
    columns: &'a C,
    style: TreeListViewStyle<'a>,
    glyphs: TreeGlyphs<'a>,
}

impl<'a, T, F, S, L, C> TreeListView<'a, T, F, S, L, C>
where
    T: TreeModel,
    F: TreeFilter<T>,
    S: TreeSort<T>,
    L: TreeLabelRenderer<T>,
    C: TreeColumns<T>,
{
    /// Creates a widget with an explicit query shared by input and rendering.
    #[must_use]
    pub const fn new(
        model: &'a T,
        query: &'a TreeQuery<F, S>,
        label: &'a L,
        columns: &'a C,
        style: TreeListViewStyle<'a>,
    ) -> Self {
        Self {
            model,
            query,
            label,
            columns,
            style,
            glyphs: TreeGlyphs::unicode(),
        }
    }

    /// Sets the glyph collection.
    #[must_use]
    pub const fn glyphs(mut self, glyphs: TreeGlyphs<'a>) -> Self {
        self.glyphs = glyphs;
        self
    }

    fn build_rows(
        &self,
        projection: &TreeProjection<T::Id>,
        nodes: &[ProjectedNode<T::Id>],
        selected: Option<T::Id>,
        selected_column: Option<usize>,
        draw_lines: bool,
        marks: impl Fn(T::Id) -> TreeMarkState,
    ) -> Vec<Row<'a>> {
        let mut rows = Vec::with_capacity(nodes.len());
        let mut tails = nodes.first().map_or_else(SmallVec::new, |node| {
            Self::tail_stack_before(projection, *node)
        });

        for node in nodes {
            Self::update_tail_stack(&mut tails, *node);
            let is_selected = selected == Some(node.id());
            let mark = marks(node.id());
            let context = TreeRowContext {
                level: node.level(),
                is_tail_stack: &tails,
                node: TreeRowNodeState {
                    expansion: node.expansion(),
                    mark,
                    match_state: node.match_state(),
                },
                render: TreeRowRenderState {
                    draw_lines,
                    is_selected,
                    selected_column,
                },
                line_style: self.style.line_style,
            };
            let tree_cell = self
                .label
                .cell(self.model, node.id(), &context, &self.glyphs);
            let cells = self
                .columns
                .cells(self.model, node.id(), &context, tree_cell);
            rows.push(Row::new(cells).style(self.row_style(node.match_state(), mark)));
        }
        rows
    }

    fn row_style(&self, match_state: TreeMatchState, mark: TreeMarkState) -> Style {
        let match_style = match match_state {
            TreeMatchState::Unfiltered => Style::default(),
            TreeMatchState::Direct => self.style.direct_match_style,
            TreeMatchState::Ancestor => self.style.ancestor_match_style,
        };
        let mark_style = match mark {
            TreeMarkState::Unmarked => Style::default(),
            TreeMarkState::Partial => self.style.partial_mark_style,
            TreeMarkState::Marked => self.style.marked_style,
        };
        match_style.patch(mark_style)
    }

    fn tail_stack_before(
        projection: &TreeProjection<T::Id>,
        node: ProjectedNode<T::Id>,
    ) -> SmallVec<[bool; 32]> {
        let mut reversed = SmallVec::<[bool; 32]>::new();
        let mut parent = node.parent();
        while let Some(parent_id) = parent {
            let Some(parent_node) = projection.get_by_id(parent_id) else {
                break;
            };
            if parent_node.level() > 0 {
                reversed.push(parent_node.is_last_sibling());
            }
            parent = parent_node.parent();
        }
        reversed.reverse();
        reversed
    }

    fn update_tail_stack(tails: &mut SmallVec<[bool; 32]>, node: ProjectedNode<T::Id>) {
        if node.level() == 0 {
            tails.clear();
            return;
        }
        tails.truncate(node.level().saturating_sub(1));
        tails.push(node.is_last_sibling());
    }

    fn table(&self, rows: Vec<Row<'a>>, widths: &[u16], header: Option<Row<'a>>) -> Table<'a> {
        let constraints = widths.iter().copied().map(Constraint::Length);
        let mut table = Table::new(rows, constraints)
            .style(self.style.block_style)
            .row_highlight_style(self.style.highlight_style)
            .column_highlight_style(self.style.column_highlight_style)
            .cell_highlight_style(self.style.cell_highlight_style)
            .highlight_symbol(self.style.highlight_symbol)
            .highlight_spacing(HighlightSpacing::Always)
            .column_spacing(self.style.column_spacing);
        if let Some(header) = header {
            table = table.header(header);
        }
        table
    }

    fn block(&self) -> Block<'_> {
        let mut block = Block::default()
            .borders(self.style.borders)
            .style(self.style.block_style)
            .border_style(self.style.border_style);
        if let Some(title) = self.style.title.clone() {
            block = block.title(title);
        }
        block
    }

    fn prepare_render(&self, inner: Rect, state: &mut TreeListViewState<T::Id>) -> RenderPlan {
        state.ensure_projection(self.model, self.query);
        state.ensure_mark_states(self.model);
        state.select_column(state.selected_column(), self.columns.column_count());

        let header_height = self.columns.header_height().min(inner.height);
        let selection_width =
            u16::try_from(Line::from(self.style.highlight_symbol).width()).unwrap_or(u16::MAX);
        let layout = self.resolve_layout(
            inner,
            state.projection().len(),
            header_height,
            selection_width,
        );
        let viewport_height = usize::from(layout.table.height.saturating_sub(header_height));
        state.ensure_selection_visible(viewport_height, self.style.scroll_policy);
        state.clamp_offset_to_viewport(viewport_height);

        let max_horizontal = layout.virtual_width.saturating_sub(layout.table.width);
        if matches!(self.style.horizontal_scroll, TreeHorizontalScroll::Disabled) {
            state.set_horizontal_offset(0);
        } else {
            state.clamp_horizontal_offset(max_horizontal);
        }
        let column_boxes =
            column_hit_boxes(&layout.widths, selection_width, self.style.column_spacing);
        if let Some(column) = state.selected_column()
            && let Some(hit_box) = column_boxes.get(column)
        {
            state.ensure_column_visible(
                hit_box.start.saturating_sub(selection_width),
                hit_box.width,
                layout.table.width.saturating_sub(selection_width),
            );
            state.clamp_horizontal_offset(max_horizontal);
        }

        let offset = state.offset().min(state.projection().len());
        let visible_end = offset
            .saturating_add(viewport_height)
            .min(state.projection().len());
        let rows = RowWindow::new(
            self.style.row_rendering,
            offset..visible_end,
            state.projection().len(),
        );
        RenderPlan {
            layout,
            header_height,
            selection_width,
            viewport_height,
            column_boxes,
            rows,
        }
    }

    fn render_projected_rows(
        &self,
        buffer: &mut Buffer,
        state: &mut TreeListViewState<T::Id>,
        plan: RenderPlan,
    ) {
        let RenderPlan {
            layout,
            header_height,
            selection_width,
            viewport_height,
            column_boxes,
            rows: row_window,
        } = plan;
        let nodes = &state.projection().nodes()[row_window.rendered.clone()];
        let rows = self.build_rows(
            state.projection(),
            nodes,
            state.selected_id(),
            state.selected_column(),
            state.draw_lines(),
            |id| state.mark_state_cached(id),
        );
        let selected = state
            .selected_index()
            .and_then(|selected| row_window.rendered_index(selected));
        let mut table_state = TableState::new()
            .with_offset(row_window.table_offset)
            .with_selected(selected)
            .with_selected_column(state.selected_column());
        let table = self.table(rows, &layout.widths, self.columns.header());

        if layout.virtual_width > layout.table.width {
            let virtual_area = Rect::new(0, 0, layout.virtual_width, layout.table.height);
            state.render_buffer.resize(virtual_area);
            state.render_buffer.reset();
            StatefulWidget::render(
                table,
                virtual_area,
                &mut state.render_buffer,
                &mut table_state,
            );
            blit_horizontal(
                &state.render_buffer,
                buffer,
                layout.table,
                state.horizontal_offset(),
                selection_width,
            );
        } else {
            StatefulWidget::render(table, layout.table, buffer, &mut table_state);
        }

        render_scrollbars(
            &layout,
            buffer,
            state.offset(),
            state.horizontal_offset(),
            state.projection().len(),
            viewport_height,
        );
        state.hit_map = TreeHitMap {
            table: layout.table,
            rows: Rect {
                y: layout.table.y.saturating_add(header_height),
                height: layout.table.height.saturating_sub(header_height),
                ..layout.table
            },
            vertical_scrollbar: layout.vertical_scrollbar,
            horizontal_scrollbar: layout.horizontal_scrollbar,
            range_start: row_window.visible.start,
            range_end: row_window.visible.end,
            horizontal_offset: state.horizontal_offset(),
            selection_width,
            columns: column_boxes,
        };
    }

    fn resolve_layout(
        &self,
        inner: Rect,
        total_rows: usize,
        header_height: u16,
        selection_width: u16,
    ) -> RenderLayout {
        let gap_count =
            u16::try_from(self.columns.column_count().saturating_sub(1)).unwrap_or(u16::MAX);
        let spacing = self.style.column_spacing.saturating_mul(gap_count);
        let mut vertical = false;
        let mut horizontal = false;
        let mut widths = SmallVec::new();
        let mut virtual_width = inner.width;

        for _ in 0..4 {
            let table_width = inner.width.saturating_sub(u16::from(vertical));
            let table_height = inner.height.saturating_sub(u16::from(horizontal));
            let rows_height = usize::from(table_height.saturating_sub(header_height));
            let next_vertical = total_rows > rows_height;
            let column_viewport = table_width
                .saturating_sub(selection_width)
                .saturating_sub(spacing);
            let target = match self.style.horizontal_scroll {
                TreeHorizontalScroll::Enabled => column_viewport.max(self.columns.ideal_width()),
                TreeHorizontalScroll::Disabled => column_viewport,
            };
            widths = self.columns.widths(target);
            let column_width = widths.iter().copied().fold(0_u16, u16::saturating_add);
            virtual_width = selection_width
                .saturating_add(spacing)
                .saturating_add(column_width)
                .max(table_width);
            let next_horizontal =
                matches!(self.style.horizontal_scroll, TreeHorizontalScroll::Enabled)
                    && virtual_width > table_width;
            if next_vertical == vertical && next_horizontal == horizontal {
                break;
            }
            vertical = next_vertical;
            horizontal = next_horizontal;
        }

        let table = Rect {
            width: inner.width.saturating_sub(u16::from(vertical)),
            height: inner.height.saturating_sub(u16::from(horizontal)),
            ..inner
        };
        let vertical_scrollbar = vertical.then_some(Rect {
            x: table.x.saturating_add(table.width),
            y: table.y,
            width: 1,
            height: table.height,
        });
        let horizontal_scrollbar = horizontal.then_some(Rect {
            x: table.x,
            y: table.y.saturating_add(table.height),
            width: table.width,
            height: 1,
        });

        RenderLayout {
            table,
            vertical_scrollbar,
            horizontal_scrollbar,
            virtual_width,
            widths,
        }
    }
}

impl<T, F, S, L, C> StatefulWidget for TreeListView<'_, T, F, S, L, C>
where
    T: TreeModel,
    F: TreeFilter<T>,
    S: TreeSort<T>,
    L: TreeLabelRenderer<T>,
    C: TreeColumns<T>,
{
    type State = TreeListViewState<T::Id>;

    fn render(self, area: Rect, buffer: &mut Buffer, state: &mut Self::State) {
        if area.is_empty() {
            state.hit_map = TreeHitMap::default();
            return;
        }

        let block = self.block();
        let inner = block.inner(area);
        block.render(area, buffer);
        if inner.is_empty() {
            state.hit_map = TreeHitMap::default();
            return;
        }
        let plan = self.prepare_render(inner, state);
        self.render_projected_rows(buffer, state, plan);
    }
}

struct RenderLayout {
    table: Rect,
    vertical_scrollbar: Option<Rect>,
    horizontal_scrollbar: Option<Rect>,
    virtual_width: u16,
    widths: SmallVec<[u16; 8]>,
}

struct RenderPlan {
    layout: RenderLayout,
    header_height: u16,
    selection_width: u16,
    viewport_height: usize,
    column_boxes: SmallVec<[ColumnHitBox; 8]>,
    rows: RowWindow,
}

struct RowWindow {
    visible: std::ops::Range<usize>,
    rendered: std::ops::Range<usize>,
    table_offset: usize,
}

impl RowWindow {
    fn new(rendering: TreeRowRendering, visible: std::ops::Range<usize>, total: usize) -> Self {
        match rendering {
            TreeRowRendering::Virtualized => Self {
                rendered: visible.clone(),
                visible,
                table_offset: 0,
            },
            TreeRowRendering::Full => Self {
                table_offset: visible.start,
                visible,
                rendered: 0..total,
            },
        }
    }

    fn rendered_index(&self, index: usize) -> Option<usize> {
        self.rendered
            .contains(&index)
            .then(|| index - self.rendered.start)
    }
}

fn column_hit_boxes(
    widths: &[u16],
    selection_width: u16,
    spacing: u16,
) -> SmallVec<[ColumnHitBox; 8]> {
    let mut start = selection_width;
    widths
        .iter()
        .copied()
        .map(|width| {
            let hit_box = ColumnHitBox { start, width };
            start = start.saturating_add(width).saturating_add(spacing);
            hit_box
        })
        .collect()
}

fn blit_horizontal(
    source: &Buffer,
    target: &mut Buffer,
    area: Rect,
    offset: u16,
    fixed_prefix: u16,
) {
    for y in 0..area.height {
        for x in 0..area.width {
            let source_x = if x < fixed_prefix {
                x
            } else {
                x.saturating_add(offset)
            };
            let source_position = (source_x, y);
            let target_position = (area.x.saturating_add(x), area.y.saturating_add(y));
            if let (Some(source_cell), Some(target_cell)) = (
                source.cell(source_position),
                target.cell_mut(target_position),
            ) {
                target_cell.clone_from(source_cell);
            }
        }
    }
}

fn render_scrollbars(
    layout: &RenderLayout,
    buffer: &mut Buffer,
    vertical_offset: usize,
    horizontal_offset: u16,
    total_rows: usize,
    viewport_height: usize,
) {
    if let Some(area) = layout.vertical_scrollbar {
        let mut scrollbar_state =
            ScrollbarState::new(scrollbar_position_count(total_rows, viewport_height))
                .position(vertical_offset)
                .viewport_content_length(viewport_height);
        Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .render(area, buffer, &mut scrollbar_state);
    }
    if let Some(area) = layout.horizontal_scrollbar {
        let viewport_width = layout.table.width as usize;
        let mut scrollbar_state = ScrollbarState::new(scrollbar_position_count(
            layout.virtual_width as usize,
            viewport_width,
        ))
        .position(horizontal_offset as usize)
        .viewport_content_length(viewport_width);
        Scrollbar::default()
            .orientation(ScrollbarOrientation::HorizontalBottom)
            .render(area, buffer, &mut scrollbar_state);
    }
}

const fn scrollbar_position_count(content_length: usize, viewport_length: usize) -> usize {
    content_length
        .saturating_sub(viewport_length)
        .saturating_add(1)
}
