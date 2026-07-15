use std::cmp::Ordering;

use ratatui::widgets::Cell;
use tui_treelistview::{
    ColumnDef, ColumnWidth, TreeChildren, TreeColumnSet, TreeFilter, TreeGlyphs, TreeLabelRenderer,
    TreeListViewState, TreeModel, TreeQuery, TreeRevision, TreeRowContext, TreeSort,
};

pub struct BenchTree {
    pub roots: Vec<usize>,
    pub children: Vec<Vec<usize>>,
}

impl BenchTree {
    pub fn balanced(node_count: usize, fanout: usize) -> Self {
        Self::forest(node_count, fanout, 1)
    }

    pub fn forest(node_count: usize, fanout: usize, root_count: usize) -> Self {
        let node_count = node_count.max(1);
        let root_count = root_count.clamp(1, node_count);
        let roots = (0..root_count).collect();
        let mut children = vec![Vec::new(); node_count];
        let mut next_id = root_count;

        for node_children in &mut children {
            for _ in 0..fanout {
                if next_id == node_count {
                    break;
                }
                node_children.push(next_id);
                next_id += 1;
            }
            if next_id == node_count {
                break;
            }
        }

        Self { roots, children }
    }

    pub fn chain(node_count: usize) -> Self {
        let node_count = node_count.max(1);
        let mut children = vec![Vec::new(); node_count];
        for (id, node_children) in children.iter_mut().enumerate().take(node_count - 1) {
            node_children.push(id + 1);
        }
        Self {
            roots: vec![0],
            children,
        }
    }

    pub fn leaves(&self) -> impl Iterator<Item = usize> + '_ {
        self.children
            .iter()
            .enumerate()
            .filter_map(|(id, children)| children.is_empty().then_some(id))
    }
}

impl TreeModel for BenchTree {
    type Id = usize;

    fn roots(&self) -> impl Iterator<Item = Self::Id> + '_ {
        self.roots.iter().copied()
    }

    fn children(&self, id: Self::Id) -> TreeChildren<'_, Self::Id> {
        TreeChildren::loaded(&self.children[id])
    }

    fn revision(&self) -> TreeRevision {
        TreeRevision::INITIAL
    }

    fn size_hint(&self) -> usize {
        self.children.len()
    }
}

pub struct WideIdTree {
    children: Vec<Vec<u128>>,
}

impl WideIdTree {
    pub fn balanced(node_count: usize, fanout: usize) -> Self {
        let tree = BenchTree::balanced(node_count, fanout);
        Self {
            children: tree
                .children
                .into_iter()
                .map(|children| children.into_iter().map(|id| id as u128).collect())
                .collect(),
        }
    }
}

impl TreeModel for WideIdTree {
    type Id = u128;

    fn roots(&self) -> impl Iterator<Item = Self::Id> + '_ {
        std::iter::once(0)
    }

    fn children(&self, id: Self::Id) -> TreeChildren<'_, Self::Id> {
        TreeChildren::loaded(&self.children[id as usize])
    }

    fn revision(&self) -> TreeRevision {
        TreeRevision::INITIAL
    }

    fn size_hint(&self) -> usize {
        self.children.len()
    }
}

pub struct Label;

impl TreeLabelRenderer<BenchTree> for Label {
    fn cell<'a>(
        &'a self,
        _model: &'a BenchTree,
        _id: usize,
        _context: &TreeRowContext<'_>,
        _glyphs: &TreeGlyphs<'a>,
    ) -> Cell<'a> {
        Cell::from("node")
    }
}

fn data_cell<'a>(_model: &'a BenchTree, _id: usize, _context: &TreeRowContext<'_>) -> Cell<'a> {
    Cell::from("data")
}

pub fn columns(count: usize) -> TreeColumnSet<'static, BenchTree> {
    let mut columns = Vec::with_capacity(count);
    let tree_width = if count == 1 {
        ColumnWidth::flexible(12, 48).expect("valid static column width")
    } else {
        ColumnWidth::fixed(24)
    };
    columns.push(ColumnDef::tree("Name", tree_width));
    columns.extend(
        (1..count).map(|index| {
            ColumnDef::data(format!("Data {index}"), ColumnWidth::fixed(24), data_cell)
        }),
    );
    TreeColumnSet::new(columns).expect("one tree column")
}

pub type BenchFilter = fn(&BenchTree, usize) -> bool;

pub const fn no_matches(_: &BenchTree, _: usize) -> bool {
    false
}

pub const fn sparse_filter(_: &BenchTree, id: usize) -> bool {
    id.is_multiple_of(17)
}

pub const fn all_matches(_: &BenchTree, _: usize) -> bool {
    true
}

pub fn descending(_: &BenchTree, left: usize, right: usize) -> Ordering {
    right.cmp(&left)
}

pub fn expanded_state<F, S>(model: &BenchTree, query: &TreeQuery<F, S>) -> TreeListViewState<usize>
where
    F: TreeFilter<BenchTree>,
    S: TreeSort<BenchTree>,
{
    let mut state = TreeListViewState::with_capacity(model.size_hint());
    let _ = state.expand_all(model);
    let _ = state.ensure_projection(model, query);
    let _ = state.select_first();
    state
}
