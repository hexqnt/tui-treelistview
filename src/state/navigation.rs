use std::hash::Hash;

use crate::projection::ProjectedNode;
use crate::style::TreeScrollPolicy;

use super::TreeListViewState;

impl<Id: Copy + Eq + Hash> TreeListViewState<Id> {
    /// Returns the selected identifier, the source of truth for selection.
    #[must_use]
    pub const fn selected_id(&self) -> Option<Id> {
        self.selected
    }

    /// Returns the selected node's current projection index.
    #[must_use]
    pub fn selected_index(&self) -> Option<usize> {
        self.selected
            .and_then(|selected| self.projection.index_of(selected))
    }

    /// Selects a visible node by identifier.
    pub fn select_id(&mut self, selected: Option<Id>) -> bool {
        let selected = selected.filter(|id| self.projection.index_of(*id).is_some());
        self.set_selection(selected)
    }

    /// Selects a row in the current projection.
    pub fn select_index(&mut self, index: Option<usize>) -> bool {
        let selected = index
            .and_then(|index| self.projection.nodes().get(index))
            .map(|node| node.id());
        self.set_selection(selected)
    }

    /// Selects the first row.
    pub fn select_first(&mut self) -> bool {
        self.select_index((!self.projection.is_empty()).then_some(0))
    }

    /// Selects the last row.
    pub fn select_last(&mut self) -> bool {
        self.select_index(
            (!self.projection.is_empty()).then_some(self.projection.len().saturating_sub(1)),
        )
    }

    /// Selects the previous row, starting at the last row when nothing is selected.
    pub fn select_prev(&mut self) -> bool {
        if self.projection.is_empty() {
            return self.set_selection(None);
        }
        let index = self.selected_index().map_or_else(
            || self.projection.len().saturating_sub(1),
            |index| index.saturating_sub(1),
        );
        self.select_index(Some(index))
    }

    /// Selects the next row, starting at the first row when nothing is selected.
    pub fn select_next(&mut self) -> bool {
        if self.projection.is_empty() {
            return self.set_selection(None);
        }
        let index = self.selected_index().map_or(0, |index| {
            index.saturating_add(1).min(self.projection.len() - 1)
        });
        self.select_index(Some(index))
    }

    /// Selects the visible parent.
    pub fn select_parent(&mut self) -> bool {
        let parent = self
            .selected_node()
            .and_then(ProjectedNode::parent)
            .filter(|parent| self.projection.index_of(*parent).is_some());
        parent.is_some() && self.set_selection(parent)
    }

    /// Selects the first visible direct child.
    pub fn select_first_child(&mut self) -> bool {
        let Some(index) = self.selected_index() else {
            return false;
        };
        let Some(parent) = self.projection.nodes().get(index).copied() else {
            return false;
        };
        let child = self
            .projection
            .nodes()
            .get(index.saturating_add(1))
            .filter(|candidate| candidate.level() == parent.level().saturating_add(1))
            .map(|candidate| candidate.id());
        child.is_some() && self.set_selection(child)
    }

    /// Returns the selected node's parent even when a synthetic parent is hidden.
    #[must_use]
    pub fn selected_parent_id(&self) -> Option<Id> {
        self.selected_node().and_then(ProjectedNode::parent)
    }

    #[must_use]
    pub fn selected_level(&self) -> Option<usize> {
        self.selected_node().map(ProjectedNode::level)
    }

    #[must_use]
    pub const fn visible_len(&self) -> usize {
        self.projection.len()
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.projection.is_empty()
    }

    pub fn visible_ids(&self) -> impl Iterator<Item = Id> + '_ {
        self.projection.nodes().iter().map(|node| node.id())
    }

    #[must_use]
    pub fn visible_index_of(&self, id: Id) -> Option<usize> {
        self.projection.index_of(id)
    }

    #[must_use]
    pub fn visible_contains(&self, id: Id) -> bool {
        self.projection.index_of(id).is_some()
    }

    /// Returns the index of the first viewport row.
    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Sets the first viewport row independently of selection.
    pub fn set_offset(&mut self, offset: usize) -> bool {
        let offset = offset.min(self.projection.len().saturating_sub(1));
        let changed = self.offset != offset;
        self.offset = offset;
        self.selection_needs_visibility = false;
        changed
    }

    /// Scrolls the viewport without changing selection.
    pub fn scroll_view_by(&mut self, amount: isize) -> bool {
        let offset = if amount.is_negative() {
            self.offset.saturating_sub(amount.unsigned_abs())
        } else {
            self.offset.saturating_add(amount.cast_unsigned())
        };
        self.set_offset(offset)
    }

    #[must_use]
    pub const fn horizontal_offset(&self) -> u16 {
        self.horizontal_offset
    }

    pub const fn set_horizontal_offset(&mut self, offset: u16) -> bool {
        let changed = self.horizontal_offset != offset;
        self.horizontal_offset = offset;
        self.column_needs_visibility = false;
        changed
    }

    pub const fn scroll_horizontal_by(&mut self, amount: i16) -> bool {
        let offset = if amount.is_negative() {
            self.horizontal_offset.saturating_sub(amount.unsigned_abs())
        } else {
            self.horizontal_offset
                .saturating_add(amount.cast_unsigned())
        };
        self.set_horizontal_offset(offset)
    }

    pub(crate) fn clamp_horizontal_offset(&mut self, maximum: u16) {
        self.horizontal_offset = self.horizontal_offset.min(maximum);
    }

    #[must_use]
    pub const fn selected_column(&self) -> Option<usize> {
        self.selected_column
    }

    pub fn select_column(&mut self, column: Option<usize>, column_count: usize) -> bool {
        let column = column.filter(|column| *column < column_count);
        let changed = self.selected_column != column;
        self.selected_column = column;
        if changed {
            self.column_needs_visibility = column.is_some();
        }
        changed
    }

    pub fn select_column_left(&mut self, column_count: usize) -> bool {
        if column_count == 0 {
            return self.select_column(None, 0);
        }
        let column = self
            .selected_column
            .filter(|column| *column < column_count)
            .map_or(column_count - 1, |column| column.saturating_sub(1));
        self.select_column(Some(column), column_count)
    }

    pub fn select_column_right(&mut self, column_count: usize) -> bool {
        if column_count == 0 {
            return self.select_column(None, 0);
        }
        let column = self
            .selected_column
            .filter(|column| *column < column_count)
            .map_or(0, |column| column.saturating_add(1).min(column_count - 1));
        self.select_column(Some(column), column_count)
    }

    pub(crate) fn ensure_selection_visible(
        &mut self,
        viewport_height: usize,
        policy: TreeScrollPolicy,
    ) {
        if !self.selection_needs_visibility {
            return;
        }
        let Some(selected) = self.selected_index() else {
            self.selection_needs_visibility = false;
            return;
        };
        let height = viewport_height.max(1);
        match policy {
            TreeScrollPolicy::KeepInView => {
                if selected < self.offset {
                    self.offset = selected;
                } else if selected >= self.offset.saturating_add(height) {
                    self.offset = selected.saturating_add(1).saturating_sub(height);
                }
            }
            TreeScrollPolicy::CenterOnSelect => {
                self.offset = selected.saturating_sub(height / 2);
            }
        }
        self.offset = self
            .offset
            .min(self.projection.len().saturating_sub(height));
        self.selection_needs_visibility = false;
    }

    pub(crate) fn clamp_offset_to_viewport(&mut self, viewport_height: usize) {
        let maximum = self.projection.len().saturating_sub(viewport_height.max(1));
        self.offset = self.offset.min(maximum);
    }

    pub(crate) const fn ensure_column_visible(
        &mut self,
        start: u16,
        width: u16,
        viewport_width: u16,
    ) {
        if !self.column_needs_visibility || viewport_width == 0 {
            return;
        }
        let end = start.saturating_add(width);
        if start < self.horizontal_offset {
            self.horizontal_offset = start;
        } else if end > self.horizontal_offset.saturating_add(viewport_width) {
            self.horizontal_offset = end.saturating_sub(viewport_width);
        }
        self.column_needs_visibility = false;
    }

    fn set_selection(&mut self, selected: Option<Id>) -> bool {
        let changed = self.selected != selected;
        self.selected = selected;
        if changed {
            self.selection_needs_visibility = selected.is_some();
        }
        changed
    }
}
