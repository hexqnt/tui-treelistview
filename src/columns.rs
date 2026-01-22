use ratatui::layout::{Constraint, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Cell, Row};
use smallvec::SmallVec;

use crate::model::TreeModel;

/// Column layout and cell rendering for tree rows.
pub trait TreeColumns<T: TreeModel> {
    /// Returns the constraint for the label (tree) column.
    fn label_constraint(&self) -> Constraint;
    /// Returns constraints for the additional columns.
    fn other_constraints(&self) -> &[Constraint];
    /// Returns an optional header row for the table.
    fn header(&self) -> Option<Row<'_>> {
        None
    }
    /// Returns cells for the additional columns of a row.
    fn cells<'a>(&'a self, model: &'a T, id: T::Id) -> SmallVec<[Cell<'a>; 8]>;
    /// Returns constraints for all columns based on the available area.
    fn constraints_for_area(&self, _area: Rect) -> SmallVec<[Constraint; 8]> {
        let mut constraints = SmallVec::<[Constraint; 8]>::new();
        constraints.push(self.label_constraint());
        constraints.extend_from_slice(self.other_constraints());
        constraints
    }
}

/// Simple container for a label constraint and a fixed set of other constraints.
pub struct TreeColumnsLayout<const N: usize> {
    label: Constraint,
    other: [Constraint; N],
}

impl<const N: usize> TreeColumnsLayout<N> {
    /// Creates a new layout with a label constraint and `N` other constraints.
    pub const fn new(label: Constraint, other: [Constraint; N]) -> Self {
        Self { label, other }
    }

    /// Returns the label column constraint.
    pub const fn label(&self) -> Constraint {
        self.label
    }

    /// Returns the other column constraints.
    pub const fn other(&self) -> &[Constraint] {
        &self.other
    }
}

/// Function pointer type for rendering a single column cell.
pub type ColumnFn<T> = for<'a> fn(&'a T, <T as TreeModel>::Id) -> Cell<'a>;

/// Column definition: header label, width constraint, and cell renderer.
#[derive(Clone, Copy)]
pub struct ColumnDef<T: TreeModel> {
    /// Header label for the column.
    pub header: &'static str,
    /// Width constraint for the column.
    pub constraint: Constraint,
    /// Renderer for the column cell.
    pub cell: ColumnFn<T>,
}

impl<T: TreeModel> ColumnDef<T> {
    /// Creates a new column definition.
    pub const fn new(header: &'static str, constraint: Constraint, cell: ColumnFn<T>) -> Self {
        Self {
            header,
            constraint,
            cell,
        }
    }
}

/// Fixed-width column layout with optional header.
pub struct SimpleColumns<const N: usize, T: TreeModel> {
    label_constraint: Constraint,
    label_header: &'static str,
    columns: [ColumnDef<T>; N],
    constraints: [Constraint; N],
    header_style: Style,
    show_header: bool,
}

impl<const N: usize, T: TreeModel> SimpleColumns<N, T> {
    /// Creates a new fixed column layout.
    pub fn new(
        label_constraint: Constraint,
        label_header: &'static str,
        columns: [ColumnDef<T>; N],
    ) -> Self {
        let constraints = std::array::from_fn(|idx| columns[idx].constraint);
        Self {
            label_constraint,
            label_header,
            columns,
            constraints,
            header_style: Style::default(),
            show_header: true,
        }
    }

    /// Sets the header row style.
    pub const fn header_style(mut self, style: Style) -> Self {
        self.header_style = style;
        self
    }

    /// Disables the header row.
    pub const fn without_header(mut self) -> Self {
        self.show_header = false;
        self
    }
}

impl<const N: usize, T: TreeModel> TreeColumns<T> for SimpleColumns<N, T> {
    fn label_constraint(&self) -> Constraint {
        self.label_constraint
    }

    fn other_constraints(&self) -> &[Constraint] {
        &self.constraints
    }

    fn header(&self) -> Option<Row<'_>> {
        if !self.show_header {
            return None;
        }

        let mut cells = SmallVec::<[Cell; 8]>::new();
        cells.push(Cell::from(self.label_header));
        for column in &self.columns {
            cells.push(Cell::from(column.header));
        }

        Some(Row::new(cells).style(self.header_style))
    }

    fn cells<'a>(&'a self, model: &'a T, id: T::Id) -> SmallVec<[Cell<'a>; 8]> {
        let mut cells = SmallVec::<[Cell<'a>; 8]>::new();
        for column in &self.columns {
            cells.push((column.cell)(model, id));
        }
        cells
    }
}

/// Width constraints for a column in adaptive layout.
#[derive(Clone, Copy, Debug)]
pub struct ColumnWidth {
    /// Minimum width.
    pub min: u16,
    /// Ideal width (used before expanding toward max).
    pub ideal: u16,
    /// Maximum width.
    pub max: u16,
}

impl ColumnWidth {
    /// Creates a fixed width (min = ideal = max).
    pub const fn fixed(width: u16) -> Self {
        Self {
            min: width,
            ideal: width,
            max: width,
        }
    }
}

/// Distributes `total` width across columns respecting `min`/`ideal`/`max`.
///
/// If `total` is outside the feasible range (`sum(min)`..=`sum(max)`), the returned widths are
/// clamped to `min` or `max` respectively (so the sum may differ from `total`).
pub fn distribute_widths(total: u16, columns: &[ColumnWidth]) -> SmallVec<[u16; 8]> {
    let mut widths = SmallVec::<[u16; 8]>::with_capacity(columns.len());
    let mut min_sum: u16 = 0;
    for col in columns {
        min_sum = min_sum.saturating_add(col.min);
        widths.push(col.min);
    }

    let mut remaining = total.saturating_sub(min_sum);
    if remaining == 0 {
        return widths;
    }

    // First grow toward ideal widths.
    for (idx, col) in columns.iter().enumerate() {
        if remaining == 0 {
            break;
        }
        let target = col.ideal.max(col.min);
        let add = target.saturating_sub(widths[idx]).min(remaining);
        widths[idx] = widths[idx].saturating_add(add);
        remaining = remaining.saturating_sub(add);
    }

    // Then expand toward max widths if space remains.
    for (idx, col) in columns.iter().enumerate() {
        if remaining == 0 {
            break;
        }
        let add = col.max.saturating_sub(widths[idx]).min(remaining);
        widths[idx] = widths[idx].saturating_add(add);
        remaining = remaining.saturating_sub(add);
    }

    widths
}

/// Adaptive layout that fits columns into the available area.
pub struct AdaptiveColumns<const N: usize, T: TreeModel> {
    label_header: &'static str,
    label_width: ColumnWidth,
    columns: [ColumnDef<T>; N],
    column_widths: [ColumnWidth; N],
    fallback_constraints: [Constraint; N],
    header_style: Style,
    show_header: bool,
}

impl<const N: usize, T: TreeModel> AdaptiveColumns<N, T> {
    /// Creates a new adaptive column layout.
    pub fn new(
        label_width: ColumnWidth,
        label_header: &'static str,
        columns: [ColumnDef<T>; N],
        column_widths: [ColumnWidth; N],
    ) -> Self {
        let fallback_constraints =
            std::array::from_fn(|idx| Constraint::Length(column_widths[idx].ideal));
        Self {
            label_header,
            label_width,
            columns,
            column_widths,
            fallback_constraints,
            header_style: Style::default(),
            show_header: true,
        }
    }

    /// Sets the header row style.
    pub const fn header_style(mut self, style: Style) -> Self {
        self.header_style = style;
        self
    }

    /// Disables the header row.
    pub const fn without_header(mut self) -> Self {
        self.show_header = false;
        self
    }
}

impl<const N: usize, T: TreeModel> TreeColumns<T> for AdaptiveColumns<N, T> {
    fn label_constraint(&self) -> Constraint {
        Constraint::Length(self.label_width.ideal)
    }

    fn other_constraints(&self) -> &[Constraint] {
        &self.fallback_constraints
    }

    fn header(&self) -> Option<Row<'_>> {
        if !self.show_header {
            return None;
        }

        let mut cells = SmallVec::<[Cell; 8]>::new();
        cells.push(Cell::from(self.label_header));
        for column in &self.columns {
            cells.push(Cell::from(column.header));
        }

        Some(Row::new(cells).style(self.header_style))
    }

    fn cells<'a>(&'a self, model: &'a T, id: T::Id) -> SmallVec<[Cell<'a>; 8]> {
        let mut cells = SmallVec::<[Cell<'a>; 8]>::new();
        for column in &self.columns {
            cells.push((column.cell)(model, id));
        }
        cells
    }

    fn constraints_for_area(&self, area: Rect) -> SmallVec<[Constraint; 8]> {
        let mut widths = SmallVec::<[ColumnWidth; 8]>::new();
        widths.push(self.label_width);
        widths.extend_from_slice(&self.column_widths);

        let raw_widths = distribute_widths(area.width, &widths);
        let mut constraints = SmallVec::<[Constraint; 8]>::new();
        for width in raw_widths {
            constraints.push(Constraint::Length(width));
        }
        constraints
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::{Constraint, Rect};

    #[test]
    fn distribute_widths_respects_min_ideal_max() {
        let columns = [
            ColumnWidth {
                min: 4,
                ideal: 6,
                max: 8,
            },
            ColumnWidth {
                min: 4,
                ideal: 4,
                max: 6,
            },
        ];
        let widths = distribute_widths(12, &columns);
        assert_eq!(widths.as_slice(), &[8, 4]);
    }

    #[test]
    fn adaptive_columns_sum_to_area_width() {
        fn cell_stub(_: &TestModel, _: usize) -> Cell<'_> {
            Cell::from("")
        }

        struct TestModel;
        impl TreeModel for TestModel {
            type Id = usize;

            fn root(&self) -> Option<Self::Id> {
                None
            }

            fn children(&self, _id: Self::Id) -> &[Self::Id] {
                &[]
            }

            fn contains(&self, _id: Self::Id) -> bool {
                false
            }
        }

        let columns = [
            ColumnDef::new("A", Constraint::Length(4), cell_stub),
            ColumnDef::new("B", Constraint::Length(4), cell_stub),
        ];
        let widths = [
            ColumnWidth {
                min: 4,
                ideal: 6,
                max: 8,
            },
            ColumnWidth {
                min: 4,
                ideal: 6,
                max: 8,
            },
        ];
        let layout = AdaptiveColumns::new(
            ColumnWidth {
                min: 6,
                ideal: 8,
                max: 10,
            },
            "Name",
            columns,
            widths,
        );
        let constraints = layout.constraints_for_area(Rect::new(0, 0, 20, 1));

        let total: u16 = constraints
            .iter()
            .map(|c| match c {
                Constraint::Length(len) => *len,
                _ => 0,
            })
            .sum();
        assert_eq!(total, 20);
    }
}
