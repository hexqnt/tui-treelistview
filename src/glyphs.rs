use std::borrow::Cow;

use ratatui::text::{Line, Span};
use ratatui::widgets::Cell;
use smallvec::SmallVec;

use crate::context::{TreeExpansionState, TreeRowContext};
use crate::model::TreeModel;

/// Glyphs for tree structure and lazy-loading states.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TreeGlyphs<'a> {
    pub indent: &'a str,
    pub branch_last: &'a str,
    pub branch: &'a str,
    pub vert: &'a str,
    pub empty: &'a str,
    pub leaf: &'a str,
    pub expanded: &'a str,
    pub collapsed: &'a str,
    pub unloaded: &'a str,
    pub loading: &'a str,
}

impl TreeGlyphs<'static> {
    /// The default Unicode glyph set.
    #[must_use]
    pub const fn unicode() -> Self {
        Self {
            indent: "   ",
            branch_last: "└──",
            branch: "├──",
            vert: "│  ",
            empty: "   ",
            leaf: "•",
            expanded: "▼",
            collapsed: "▶",
            unloaded: "◇",
            loading: "◌",
        }
    }

    /// An ASCII glyph set for terminals without Unicode support.
    #[must_use]
    pub const fn ascii() -> Self {
        Self {
            indent: "   ",
            branch_last: "`--",
            branch: "|--",
            vert: "|  ",
            empty: "   ",
            leaf: "*",
            expanded: "v",
            collapsed: ">",
            unloaded: "?",
            loading: "~",
        }
    }
}

/// A node name with an optional leading icon.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TreeLabelPrefix<'a> {
    pub name: Cow<'a, str>,
    pub prefix: Option<Cow<'a, str>>,
}

impl<'a> TreeLabelPrefix<'a> {
    /// Creates a borrowed name without a prefix.
    #[must_use]
    pub const fn borrowed(name: &'a str) -> Self {
        Self {
            name: Cow::Borrowed(name),
            prefix: None,
        }
    }
}

/// A simplified provider for node names and icons.
pub trait TreeLabelProvider<T: TreeModel> {
    fn label_parts<'a>(&'a self, model: &'a T, id: T::Id) -> TreeLabelPrefix<'a>;
}

/// A complete renderer for the primary tree cell.
pub trait TreeLabelRenderer<T: TreeModel> {
    fn cell<'a>(
        &'a self,
        model: &'a T,
        id: T::Id,
        context: &TreeRowContext<'_>,
        glyphs: &TreeGlyphs<'a>,
    ) -> Cell<'a>;
}

impl<T, P> TreeLabelRenderer<T> for P
where
    T: TreeModel,
    P: TreeLabelProvider<T>,
{
    fn cell<'a>(
        &'a self,
        model: &'a T,
        id: T::Id,
        context: &TreeRowContext<'_>,
        glyphs: &TreeGlyphs<'a>,
    ) -> Cell<'a> {
        tree_name_cell(context, self.label_parts(model, id), glyphs)
    }
}

/// Builds the primary cell contents, including guides and branch state.
#[must_use]
pub fn tree_label_line<'a>(
    context: &TreeRowContext<'_>,
    parts: TreeLabelPrefix<'a>,
    glyphs: &TreeGlyphs<'a>,
) -> Line<'a> {
    let mut spans =
        SmallVec::<[Span<'a>; 16]>::with_capacity(context.is_tail_stack.len().saturating_add(6));

    if context.level > 0 {
        if context.render.draw_lines {
            let branch_level = context.level - 1;
            for (level, &is_last) in context.is_tail_stack.iter().enumerate() {
                let glyph = if level == branch_level {
                    if is_last {
                        glyphs.branch_last
                    } else {
                        glyphs.branch
                    }
                } else if is_last {
                    glyphs.indent
                } else {
                    glyphs.vert
                };
                spans.push(Span::styled(glyph, context.line_style));
            }
        } else {
            spans.extend((0..context.level).map(|_| Span::raw(glyphs.empty)));
        }
    }

    let state_glyph = match context.node.expansion {
        TreeExpansionState::Leaf => (context.level > 0).then_some(glyphs.leaf),
        TreeExpansionState::Collapsed => Some(glyphs.collapsed),
        TreeExpansionState::Expanded | TreeExpansionState::ForcedByFilter => Some(glyphs.expanded),
        TreeExpansionState::Unloaded => Some(glyphs.unloaded),
        TreeExpansionState::Loading => Some(glyphs.loading),
    };

    if let Some(glyph) = state_glyph.filter(|glyph| !glyph.is_empty()) {
        push_separator(&mut spans);
        spans.push(Span::raw(glyph));
    }
    if let Some(prefix) = parts.prefix.filter(|prefix| !prefix.is_empty()) {
        push_separator(&mut spans);
        spans.push(Span::raw(prefix));
    }
    push_separator(&mut spans);
    spans.push(Span::raw(parts.name));

    Line::from(spans.into_vec())
}

fn push_separator(spans: &mut SmallVec<[Span<'_>; 16]>) {
    if !spans.is_empty() {
        spans.push(Span::raw(" "));
    }
}

/// Wraps [`tree_label_line`] in a table cell.
#[inline]
#[must_use]
pub fn tree_name_cell<'a>(
    context: &TreeRowContext<'_>,
    parts: TreeLabelPrefix<'a>,
    glyphs: &TreeGlyphs<'a>,
) -> Cell<'a> {
    Cell::from(tree_label_line(context, parts, glyphs))
}

#[cfg(test)]
mod tests {
    use ratatui::style::Style;

    use super::*;
    use crate::context::{TreeMarkState, TreeMatchState, TreeRowNodeState, TreeRowRenderState};

    fn context(level: usize, tails: &[bool], expansion: TreeExpansionState) -> TreeRowContext<'_> {
        TreeRowContext {
            level,
            is_tail_stack: tails,
            node: TreeRowNodeState {
                expansion,
                mark: TreeMarkState::Unmarked,
                match_state: TreeMatchState::Unfiltered,
            },
            render: TreeRowRenderState {
                draw_lines: true,
                is_selected: false,
                selected_column: None,
            },
            line_style: Style::default(),
        }
    }

    #[test]
    fn renders_root_and_nested_leaf() {
        let root = tree_label_line(
            &context(0, &[], TreeExpansionState::Collapsed),
            TreeLabelPrefix::borrowed("root"),
            &TreeGlyphs::unicode(),
        );
        assert_eq!(root.to_string(), "▶ root");

        let leaf = tree_label_line(
            &context(2, &[false, true], TreeExpansionState::Leaf),
            TreeLabelPrefix::borrowed("leaf"),
            &TreeGlyphs::unicode(),
        );
        assert_eq!(leaf.to_string(), "│  └── • leaf");
    }

    #[test]
    fn renders_lazy_states() {
        let unloaded = tree_label_line(
            &context(0, &[], TreeExpansionState::Unloaded),
            TreeLabelPrefix::borrowed("remote"),
            &TreeGlyphs::unicode(),
        );
        assert_eq!(unloaded.to_string(), "◇ remote");
    }
}
