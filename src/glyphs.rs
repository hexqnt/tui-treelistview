use std::borrow::Cow;

use ratatui::text::{Line, Span};
use ratatui::widgets::Cell;

use crate::context::TreeRowContext;
use crate::model::TreeModel;

#[derive(Clone, Copy)]
pub struct TreeGlyphs<'a> {
    pub indent: &'a str,
    pub branch_last: &'a str,
    pub branch: &'a str,
    pub vert: &'a str,
    pub empty: &'a str,
    pub leaf: &'a str,
    pub expanded: &'a str,
    pub collapsed: &'a str,
}

impl TreeGlyphs<'static> {
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

#[derive(Clone)]
pub struct TreeLabelPrefix<'a> {
    pub name: &'a str,
    pub prefix: Option<Cow<'a, str>>,
}

pub trait TreeLabelProvider<T: TreeModel> {
    fn label_parts<'a>(&'a self, model: &'a T, id: T::Id) -> TreeLabelPrefix<'a>;
}

pub trait TreeLabelRenderer<T: TreeModel> {
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

pub fn tree_name_cell<'a>(
    ctx: &TreeRowContext<'_>,
    parts: TreeLabelPrefix<'a>,
    glyphs: &TreeGlyphs<'a>,
) -> Cell<'a> {
    Cell::from(tree_label_line(ctx, parts, glyphs))
}
