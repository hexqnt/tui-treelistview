use std::borrow::Cow;

use ratatui::text::{Line, Span};
use ratatui::widgets::Cell;

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
    let state_glyph = match context.node.expansion {
        TreeExpansionState::Leaf => (context.level > 0).then_some(glyphs.leaf),
        TreeExpansionState::Collapsed => Some(glyphs.collapsed),
        TreeExpansionState::Expanded | TreeExpansionState::ForcedByFilter => Some(glyphs.expanded),
        TreeExpansionState::Unloaded => Some(glyphs.unloaded),
        TreeExpansionState::Loading => Some(glyphs.loading),
    }
    .filter(|glyph| !glyph.is_empty());
    let prefix = parts.prefix.filter(|prefix| !prefix.is_empty());
    let guide_count = if context.level == 0 {
        0
    } else if context.render.draw_lines {
        context.is_tail_stack.len()
    } else {
        context.level
    };
    let label_spans = 2 * (usize::from(state_glyph.is_some()) + usize::from(prefix.is_some())) + 1;
    let capacity = guide_count
        .saturating_add(usize::from(guide_count > 0))
        .saturating_add(label_spans);
    let mut spans = Vec::with_capacity(capacity);

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

    for part in state_glyph
        .map(Cow::Borrowed)
        .into_iter()
        .chain(prefix)
        .chain([parts.name])
    {
        if !spans.is_empty() {
            spans.push(Span::raw(" "));
        }
        spans.push(Span::raw(part));
    }

    Line::from(spans)
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
    fn renders_every_expansion_state_for_both_glyph_sets() {
        for (expansion, unicode, ascii) in [
            (TreeExpansionState::Leaf, "node", "node"),
            (TreeExpansionState::Collapsed, "▶ node", "> node"),
            (TreeExpansionState::Expanded, "▼ node", "v node"),
            (TreeExpansionState::ForcedByFilter, "▼ node", "v node"),
            (TreeExpansionState::Unloaded, "◇ node", "? node"),
            (TreeExpansionState::Loading, "◌ node", "~ node"),
        ] {
            for (glyphs, expected) in [
                (TreeGlyphs::unicode(), unicode),
                (TreeGlyphs::ascii(), ascii),
            ] {
                assert_eq!(
                    tree_label_line(
                        &context(0, &[], expansion),
                        TreeLabelPrefix::borrowed("node"),
                        &glyphs,
                    )
                    .to_string(),
                    expected
                );
            }
        }
    }

    #[test]
    fn renders_icons_with_and_without_guides() {
        let mut context = context(2, &[false, true], TreeExpansionState::Leaf);
        for (draw_lines, expected) in [(true, "|  `-- * icon leaf"), (false, "       * icon leaf")]
        {
            context.render.draw_lines = draw_lines;
            let line = tree_label_line(
                &context,
                TreeLabelPrefix {
                    name: "leaf".into(),
                    prefix: Some("icon".into()),
                },
                &TreeGlyphs::ascii(),
            );
            assert_eq!(line.to_string(), expected);
        }
    }

    #[test]
    fn empty_glyphs_and_icons_do_not_add_separators() {
        let glyphs = TreeGlyphs {
            collapsed: "",
            ..TreeGlyphs::ascii()
        };
        for name in ["node", ""] {
            let line = tree_label_line(
                &context(0, &[], TreeExpansionState::Collapsed),
                TreeLabelPrefix {
                    name: name.into(),
                    prefix: Some("".into()),
                },
                &glyphs,
            );
            assert_eq!(line.to_string(), name);
        }
    }

    #[test]
    fn applies_line_style_only_to_guides() {
        let mut context = context(2, &[false, true], TreeExpansionState::Leaf);
        context.line_style = Style::new().fg(ratatui::style::Color::Blue);
        let line = tree_label_line(
            &context,
            TreeLabelPrefix::borrowed("leaf"),
            &TreeGlyphs::unicode(),
        );
        let (guides, label) = line.spans.split_at(2);
        assert!(guides.iter().all(|span| span.style == context.line_style));
        assert!(label.iter().all(|span| span.style == Style::default()));
    }

    #[test]
    fn reserves_only_the_spans_that_are_rendered() {
        for level in [0, 1, 16] {
            let tails = vec![false; level];
            for expansion in [TreeExpansionState::Leaf, TreeExpansionState::Collapsed] {
                let mut context = context(level, &tails, expansion);
                for draw_lines in [false, true] {
                    context.render.draw_lines = draw_lines;
                    for prefix in [None, Some(""), Some("icon")] {
                        let line = tree_label_line(
                            &context,
                            TreeLabelPrefix {
                                name: "node".into(),
                                prefix: prefix.map(Cow::Borrowed),
                            },
                            &TreeGlyphs::unicode(),
                        );
                        assert_eq!(line.spans.capacity(), line.spans.len());
                    }
                }
            }
        }
    }
}
