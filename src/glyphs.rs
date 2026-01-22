use std::borrow::Cow;

use ratatui::text::{Line, Span};
use ratatui::widgets::Cell;

use crate::context::TreeRowContext;
use crate::model::TreeModel;

/// Glyph set used to render the tree structure and expanders.
#[derive(Clone, Copy)]
pub struct TreeGlyphs<'a> {
    /// Indentation for empty levels.
    pub indent: &'a str,
    /// Branch glyph for the last child.
    pub branch_last: &'a str,
    /// Branch glyph for intermediate children.
    pub branch: &'a str,
    /// Vertical continuation glyph.
    pub vert: &'a str,
    /// Empty spacer glyph (used when no line is drawn).
    pub empty: &'a str,
    /// Leaf glyph for nodes without children.
    pub leaf: &'a str,
    /// Expander glyph for expanded nodes.
    pub expanded: &'a str,
    /// Expander glyph for collapsed nodes.
    pub collapsed: &'a str,
}

impl TreeGlyphs<'static> {
    /// Returns a Unicode glyph set.
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
        }
    }

    /// Returns an ASCII-only glyph set.
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
        }
    }
}

/// Label parts: name with an optional prefix (e.g., marker or icon).
#[derive(Clone)]
pub struct TreeLabelPrefix<'a> {
    /// Node display name.
    pub name: &'a str,
    /// Optional prefix rendered before the name.
    pub prefix: Option<Cow<'a, str>>,
}

/// Provides label parts for a node.
pub trait TreeLabelProvider<T: TreeModel> {
    /// Returns name and optional prefix for the node.
    fn label_parts<'a>(&'a self, model: &'a T, id: T::Id) -> TreeLabelPrefix<'a>;
}

/// Renders a node into a `Cell` for the label column.
pub trait TreeLabelRenderer<T: TreeModel> {
    /// Builds the label cell for the given node.
    fn cell<'a>(
        &'a self,
        model: &'a T,
        id: T::Id,
        ctx: &TreeRowContext,
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
        ctx: &TreeRowContext,
        glyphs: &TreeGlyphs<'a>,
    ) -> Cell<'a> {
        let parts = self.label_parts(model, id);
        tree_name_cell(ctx, parts, glyphs)
    }
}

/// Builds a `Line` for the tree label, including guides and expanders.
pub fn tree_label_line<'a>(
    ctx: &TreeRowContext<'_>,
    parts: TreeLabelPrefix<'a>,
    glyphs: &TreeGlyphs<'a>,
) -> Line<'a> {
    let TreeLabelPrefix { name, prefix: op } = parts;
    let op = op.filter(|value| !value.is_empty());

    if ctx.level == 0 || !ctx.draw_lines {
        let expander = if ctx.has_children {
            if ctx.is_expanded {
                glyphs.expanded
            } else {
                glyphs.collapsed
            }
        } else if ctx.level == 0 {
            ""
        } else {
            glyphs.leaf
        };

        let mut spans = Vec::with_capacity(ctx.level as usize + 6);
        if ctx.level > 0 {
            for _ in 0..ctx.level {
                spans.push(Span::raw(glyphs.empty));
            }
        }
        if !expander.is_empty() {
            spans.push(Span::raw(expander));
        }
        if let Some(op) = op {
            spans.push(Span::raw(op));
        }
        spans.push(Span::raw(" "));
        spans.push(Span::raw(name));
        return Line::from(spans);
    }

    let mut name_spans = Vec::with_capacity(ctx.is_tail_stack.len() + 6);

    for (l, is_last) in ctx.is_tail_stack.iter().enumerate() {
        let part = if l == (ctx.level as usize) - 1 {
            if *is_last {
                glyphs.branch_last
            } else {
                glyphs.branch
            }
        } else if ctx.is_tail_stack[l] {
            glyphs.indent
        } else {
            glyphs.vert
        };
        name_spans.push(Span::styled(part, ctx.line_style));
    }

    let expander = if ctx.has_children {
        if ctx.is_expanded {
            glyphs.expanded
        } else {
            glyphs.collapsed
        }
    } else {
        glyphs.leaf
    };

    if !expander.is_empty() {
        name_spans.push(Span::raw(expander));
        name_spans.push(Span::raw(" "));
    }

    if let Some(op) = op {
        name_spans.push(Span::raw(op));
        name_spans.push(Span::raw(" "));
    }

    name_spans.push(Span::raw(name));
    Line::from(name_spans)
}

/// Convenience wrapper to build a label `Cell` from the label `Line`.
#[inline]
pub fn tree_name_cell<'a>(
    ctx: &TreeRowContext<'_>,
    parts: TreeLabelPrefix<'a>,
    glyphs: &TreeGlyphs<'a>,
) -> Cell<'a> {
    Cell::from(tree_label_line(ctx, parts, glyphs))
}
