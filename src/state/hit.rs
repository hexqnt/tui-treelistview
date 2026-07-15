use std::hash::Hash;

use ratatui::layout::{Position, Rect};
use smallvec::SmallVec;

use super::TreeListViewState;

/// The region of the latest rendering that contains a coordinate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeHitRegion {
    Header,
    Row,
    VerticalScrollbar,
    HorizontalScrollbar,
}

/// A hit-test result for the most recently rendered layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeHit<Id> {
    Header {
        column: Option<usize>,
    },
    Row {
        id: Id,
        index: usize,
        column: Option<usize>,
    },
    VerticalScrollbar,
    HorizontalScrollbar,
}

impl<Id> TreeHit<Id> {
    #[must_use]
    pub const fn region(&self) -> TreeHitRegion {
        match self {
            Self::Header { .. } => TreeHitRegion::Header,
            Self::Row { .. } => TreeHitRegion::Row,
            Self::VerticalScrollbar => TreeHitRegion::VerticalScrollbar,
            Self::HorizontalScrollbar => TreeHitRegion::HorizontalScrollbar,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct ColumnHitBox {
    pub start: u16,
    pub width: u16,
}

#[derive(Clone, Debug, Default)]
pub struct TreeHitMap {
    pub table: Rect,
    pub rows: Rect,
    pub vertical_scrollbar: Option<Rect>,
    pub horizontal_scrollbar: Option<Rect>,
    pub range_start: usize,
    pub range_end: usize,
    pub horizontal_offset: u16,
    pub selection_width: u16,
    pub columns: SmallVec<[ColumnHitBox; 8]>,
}

impl<Id: Copy + Eq + Hash> TreeListViewState<Id> {
    /// Resolves a row and column from coordinates in the latest render call.
    #[must_use]
    pub fn hit_test(&self, position: Position) -> Option<TreeHit<Id>> {
        if self
            .hit_map
            .vertical_scrollbar
            .is_some_and(|area| contains(area, position))
        {
            return Some(TreeHit::VerticalScrollbar);
        }
        if self
            .hit_map
            .horizontal_scrollbar
            .is_some_and(|area| contains(area, position))
        {
            return Some(TreeHit::HorizontalScrollbar);
        }
        if !contains(self.hit_map.table, position) {
            return None;
        }

        let column = self.hit_column(position.x);
        if position.y < self.hit_map.rows.y {
            return Some(TreeHit::Header { column });
        }
        if !contains(self.hit_map.rows, position) {
            return None;
        }

        let row = self
            .hit_map
            .range_start
            .saturating_add(position.y.saturating_sub(self.hit_map.rows.y) as usize);
        if row >= self.hit_map.range_end {
            return None;
        }
        self.projection.nodes().get(row).map(|node| TreeHit::Row {
            id: node.id(),
            index: row,
            column,
        })
    }

    fn hit_column(&self, x: u16) -> Option<usize> {
        let local_x = x.saturating_sub(self.hit_map.table.x);
        if local_x < self.hit_map.selection_width {
            return None;
        }
        let virtual_x = local_x.saturating_add(self.hit_map.horizontal_offset);
        self.hit_map.columns.iter().position(|column| {
            virtual_x >= column.start && virtual_x < column.start.saturating_add(column.width)
        })
    }
}

const fn contains(area: Rect, position: Position) -> bool {
    position.x >= area.x
        && position.x < area.x.saturating_add(area.width)
        && position.y >= area.y
        && position.y < area.y.saturating_add(area.height)
}
