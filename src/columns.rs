use std::error::Error;
use std::fmt::{self, Display, Formatter};

use ratatui::style::Style;
use ratatui::text::Line;
use ratatui::widgets::{Cell, Row};
use smallvec::SmallVec;

use crate::context::TreeRowContext;
use crate::model::TreeModel;

/// An error produced while constructing a valid column width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColumnWidthError {
    MinExceedsIdeal,
    IdealExceedsMax,
}

impl Display for ColumnWidthError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::MinExceedsIdeal => formatter.write_str("minimum width exceeds ideal width"),
            Self::IdealExceedsMax => formatter.write_str("ideal width exceeds maximum width"),
        }
    }
}

impl Error for ColumnWidthError {}

enum TreeColumnKind<'a, T: TreeModel> {
    Tree,
    Data(Box<dyn TreeCellRenderer<T> + 'a>),
}

/// An error produced while parsing a column set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeColumnsError {
    Empty,
    MissingTreeColumn,
    MultipleTreeColumns,
}

impl Display for TreeColumnsError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("column set is empty"),
            Self::MissingTreeColumn => formatter.write_str("tree column is missing"),
            Self::MultipleTreeColumns => formatter.write_str("multiple tree columns are defined"),
        }
    }
}

impl Error for TreeColumnsError {}

/// A validated width range satisfying `min <= ideal <= max`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ColumnWidth {
    min: u16,
    ideal: u16,
    max: u16,
}

impl ColumnWidth {
    /// Creates a width range after checking its invariants once.
    ///
    /// # Errors
    ///
    /// Returns [`ColumnWidthError`] unless `min <= ideal <= max`.
    pub const fn new(min: u16, ideal: u16, max: u16) -> Result<Self, ColumnWidthError> {
        if min > ideal {
            return Err(ColumnWidthError::MinExceedsIdeal);
        }
        if ideal > max {
            return Err(ColumnWidthError::IdealExceedsMax);
        }
        Ok(Self { min, ideal, max })
    }

    /// Creates a fixed-width column.
    #[must_use]
    pub const fn fixed(width: u16) -> Self {
        Self {
            min: width,
            ideal: width,
            max: width,
        }
    }

    /// Creates a flexible column without an upper bound.
    ///
    /// # Errors
    ///
    /// Returns [`ColumnWidthError::MinExceedsIdeal`] when `min > ideal`.
    pub const fn flexible(min: u16, ideal: u16) -> Result<Self, ColumnWidthError> {
        Self::new(min, ideal, u16::MAX)
    }

    #[must_use]
    pub const fn min(self) -> u16 {
        self.min
    }

    #[must_use]
    pub const fn ideal(self) -> u16 {
        self.ideal
    }

    #[must_use]
    pub const fn max(self) -> u16 {
        self.max
    }
}

struct OwnedCellRenderer<R>(R);

impl<T, R> TreeCellRenderer<T> for OwnedCellRenderer<R>
where
    T: TreeModel,
    R: Fn(&T, T::Id, &TreeRowContext<'_>) -> Cell<'static>,
{
    fn cell<'a>(&'a self, model: &'a T, id: T::Id, context: &TreeRowContext<'_>) -> Cell<'a> {
        (self.0)(model, id, context)
    }
}

/// A column definition. Exactly one column in a set must have the tree role.
pub struct ColumnDef<'a, T: TreeModel> {
    header: Line<'a>,
    width: ColumnWidth,
    kind: TreeColumnKind<'a, T>,
}

impl<'a, T: TreeModel> ColumnDef<'a, T> {
    /// Creates the primary tree column.
    #[must_use]
    pub fn tree(header: impl Into<Line<'a>>, width: ColumnWidth) -> Self {
        Self {
            header: header.into(),
            width,
            kind: TreeColumnKind::Tree,
        }
    }

    /// Creates an additional column whose renderer may borrow model data without allocation.
    ///
    /// Function items and custom [`TreeCellRenderer`] implementations can return cells tied to
    /// the model borrow. Use [`Self::data_owned`] for an ergonomic capturing closure.
    #[must_use]
    pub fn data<R>(header: impl Into<Line<'a>>, width: ColumnWidth, renderer: R) -> Self
    where
        R: TreeCellRenderer<T> + 'a,
    {
        Self {
            header: header.into(),
            width,
            kind: TreeColumnKind::Data(Box::new(renderer)),
        }
    }

    /// Creates an additional column from a capturing closure that returns an owned cell.
    ///
    /// Use [`Self::data`] when the returned cell borrows model data. This variant is convenient
    /// for closures that capture formatting configuration and produce owned text.
    #[must_use]
    pub fn data_owned<R>(header: impl Into<Line<'a>>, width: ColumnWidth, renderer: R) -> Self
    where
        R: Fn(&T, T::Id, &TreeRowContext<'_>) -> Cell<'static> + 'a,
    {
        Self::data(header, width, OwnedCellRenderer(renderer))
    }
}

/// A dynamic column set parsed and validated once at construction.
pub struct TreeColumnSet<'a, T: TreeModel> {
    columns: Vec<ColumnDef<'a, T>>,
    tree_column: usize,
    header_style: Style,
    show_header: bool,
}

impl<'a, T: TreeModel> TreeColumnSet<'a, T> {
    /// Validates the set and records its single tree column.
    ///
    /// # Errors
    ///
    /// Returns [`TreeColumnsError`] when the set is empty or does not contain exactly one tree
    /// column.
    pub fn new(
        columns: impl IntoIterator<Item = ColumnDef<'a, T>>,
    ) -> Result<Self, TreeColumnsError> {
        let columns: Vec<_> = columns.into_iter().collect();
        if columns.is_empty() {
            return Err(TreeColumnsError::Empty);
        }

        let mut tree_columns = columns.iter().enumerate().filter_map(|(index, column)| {
            matches!(column.kind, TreeColumnKind::Tree).then_some(index)
        });
        let Some(tree_column) = tree_columns.next() else {
            return Err(TreeColumnsError::MissingTreeColumn);
        };
        if tree_columns.next().is_some() {
            return Err(TreeColumnsError::MultipleTreeColumns);
        }

        Ok(Self {
            columns,
            tree_column,
            header_style: Style::default(),
            show_header: true,
        })
    }

    /// Sets the header style.
    #[must_use]
    pub const fn header_style(mut self, style: Style) -> Self {
        self.header_style = style;
        self
    }

    /// Hides the header.
    #[must_use]
    pub const fn without_header(mut self) -> Self {
        self.show_header = false;
        self
    }

    fn total_width(&self, width: impl Fn(ColumnWidth) -> u16) -> u16 {
        self.columns
            .iter()
            .fold(0, |sum, column| sum.saturating_add(width(column.width)))
    }
}

impl<T: TreeModel> TreeColumns<T> for TreeColumnSet<'_, T> {
    fn column_count(&self) -> usize {
        self.columns.len()
    }

    fn tree_column_index(&self) -> usize {
        self.tree_column
    }

    fn minimum_width(&self) -> u16 {
        self.total_width(ColumnWidth::min)
    }

    fn ideal_width(&self) -> u16 {
        self.total_width(ColumnWidth::ideal)
    }

    fn widths(&self, available: u16) -> SmallVec<[u16; 8]> {
        distribute_widths(available, self.columns.iter().map(|column| column.width))
    }

    fn header(&self) -> Option<Row<'_>> {
        self.show_header.then(|| {
            Row::new(self.columns.iter().map(|column| column.header.clone()))
                .style(self.header_style)
        })
    }

    fn header_height(&self) -> u16 {
        u16::from(self.show_header)
    }

    fn cells<'a>(
        &'a self,
        model: &'a T,
        id: T::Id,
        context: &TreeRowContext<'_>,
        tree_cell: Cell<'a>,
    ) -> SmallVec<[Cell<'a>; 8]> {
        let mut tree_cell = Some(tree_cell);
        self.columns
            .iter()
            .map(|column| match &column.kind {
                TreeColumnKind::Tree => tree_cell.take().unwrap_or_default(),
                TreeColumnKind::Data(renderer) => renderer.cell(model, id, context),
            })
            .collect()
    }
}

/// Renders an additional cell with full row context.
pub trait TreeCellRenderer<T: TreeModel> {
    fn cell<'a>(&'a self, model: &'a T, id: T::Id, context: &TreeRowContext<'_>) -> Cell<'a>;
}

impl<T, F> TreeCellRenderer<T> for F
where
    T: TreeModel,
    F: for<'a> Fn(&'a T, T::Id, &TreeRowContext<'_>) -> Cell<'a>,
{
    fn cell<'a>(&'a self, model: &'a T, id: T::Id, context: &TreeRowContext<'_>) -> Cell<'a> {
        self(model, id, context)
    }
}

/// Lays out columns and builds every cell in a row.
pub trait TreeColumns<T: TreeModel> {
    fn column_count(&self) -> usize;
    fn tree_column_index(&self) -> usize;
    fn minimum_width(&self) -> u16;
    fn ideal_width(&self) -> u16;
    fn widths(&self, available: u16) -> SmallVec<[u16; 8]>;
    fn header(&self) -> Option<Row<'_>>;
    fn header_height(&self) -> u16 {
        u16::from(self.header().is_some())
    }
    fn cells<'a>(
        &'a self,
        model: &'a T,
        id: T::Id,
        context: &TreeRowContext<'_>,
        tree_cell: Cell<'a>,
    ) -> SmallVec<[Cell<'a>; 8]>;
}

/// Distributes width as evenly as possible between `min`, `ideal`, and `max`.
///
/// A remainder smaller than the number of growable columns is assigned in column order.
#[must_use]
pub fn distribute_widths(
    total: u16,
    columns: impl IntoIterator<Item = ColumnWidth>,
) -> SmallVec<[u16; 8]> {
    let columns: SmallVec<[ColumnWidth; 8]> = columns.into_iter().collect();
    let mut widths: SmallVec<[u16; 8]> = columns.iter().map(|column| column.min).collect();
    let minimum = widths.iter().copied().fold(0_u16, u16::saturating_add);
    let mut remaining = total.saturating_sub(minimum);
    grow_towards(&mut widths, &columns, &mut remaining, |column| column.ideal);
    grow_towards(&mut widths, &columns, &mut remaining, |column| column.max);
    widths
}

fn grow_towards(
    widths: &mut [u16],
    columns: &[ColumnWidth],
    remaining: &mut u16,
    target: impl Fn(ColumnWidth) -> u16,
) {
    while *remaining > 0 {
        let active = widths
            .iter()
            .zip(columns)
            .filter(|(width, column)| **width < target(**column))
            .count();
        if active == 0 {
            return;
        }

        let active = u16::try_from(active).unwrap_or(u16::MAX);
        let share = (*remaining / active).max(1);
        let mut spent = 0_u16;
        for (width, column) in widths.iter_mut().zip(columns) {
            if *remaining == 0 {
                break;
            }
            let add = target(*column)
                .saturating_sub(*width)
                .min(share)
                .min(*remaining);
            *width = width.saturating_add(add);
            *remaining = remaining.saturating_sub(add);
            spent = spent.saturating_add(add);
        }
        if spent == 0 {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_width_rejects_invalid_ranges() {
        assert_eq!(
            ColumnWidth::new(5, 4, 6),
            Err(ColumnWidthError::MinExceedsIdeal)
        );
        assert_eq!(
            ColumnWidth::new(2, 7, 6),
            Err(ColumnWidthError::IdealExceedsMax)
        );
    }

    #[test]
    fn distribution_is_balanced_and_bounded() {
        let width = ColumnWidth::new(2, 4, 6).expect("valid width");
        let widths = distribute_widths(10, [width, width, width]);
        assert_eq!(widths.as_slice(), &[4, 3, 3]);

        let widths = distribute_widths(30, [width, width, width]);
        assert_eq!(widths.as_slice(), &[6, 6, 6]);
    }

    #[test]
    fn distribution_preserves_bounds_for_every_available_width() {
        let columns = [
            ColumnWidth::new(1, 4, 9).expect("valid width"),
            ColumnWidth::new(3, 5, 7).expect("valid width"),
            ColumnWidth::new(2, 8, 12).expect("valid width"),
        ];
        for total in 0..=40 {
            let widths = distribute_widths(total, columns);
            for (width, column) in widths.iter().zip(columns) {
                assert!(*width >= column.min());
                assert!(*width <= column.max());
            }
            let actual = widths.iter().copied().sum::<u16>();
            assert_eq!(actual, total.clamp(6, 28));
        }
    }
}
