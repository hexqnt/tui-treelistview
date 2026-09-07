use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::hash::Hash;

use smallvec::SmallVec;

use crate::model::{TreeChildren, TreeModel, TreeRevision};

/// An error produced while parsing an indexed tree.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IndexedTreeError {
    InvalidRoot(usize),
    DuplicateRoot(usize),
    MissingRoot(usize),
    InvalidChild { parent: usize, child: usize },
    MultipleParents(usize),
    RootHasParent(usize),
    Cycle,
}

impl Display for IndexedTreeError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRoot(root) => write!(formatter, "invalid root index: {root}"),
            Self::DuplicateRoot(root) => write!(formatter, "duplicate root index: {root}"),
            Self::MissingRoot(root) => write!(formatter, "top-level node is not a root: {root}"),
            Self::InvalidChild { parent, child } => {
                write!(
                    formatter,
                    "invalid child index {child} under parent {parent}"
                )
            }
            Self::MultipleParents(node) => write!(formatter, "node has multiple parents: {node}"),
            Self::RootHasParent(root) => write!(formatter, "root has a parent: {root}"),
            Self::Cycle => formatter.write_str("indexed tree contains a cycle"),
        }
    }
}

impl Error for IndexedTreeError {}

/// A zero-copy adapter over roots and a child accessor for arbitrary storage.
pub struct TreeModelRef<'a, Id, C> {
    roots: &'a [Id],
    children: C,
    revision: TreeRevision,
    size_hint: usize,
}

impl<'a, Id, C> TreeModelRef<'a, Id, C> {
    /// Creates an adapter. Tree validity remains an invariant of the source storage.
    #[must_use]
    pub const fn new(roots: &'a [Id], children: C, revision: TreeRevision) -> Self {
        Self {
            roots,
            children,
            revision,
            size_hint: 0,
        }
    }

    /// Sets an approximate node count for cache reservation.
    #[must_use]
    pub const fn with_size_hint(mut self, size_hint: usize) -> Self {
        self.size_hint = size_hint;
        self
    }
}

impl<'a, Id, C> TreeModel for TreeModelRef<'a, Id, C>
where
    Id: Copy + Eq + Hash,
    C: Fn(Id) -> TreeChildren<'a, Id>,
{
    type Id = Id;

    fn roots(&self) -> impl Iterator<Item = Self::Id> + '_ {
        self.roots.iter().copied()
    }

    fn children(&self, id: Self::Id) -> TreeChildren<'_, Self::Id> {
        (self.children)(id)
    }

    fn revision(&self) -> TreeRevision {
        self.revision
    }

    fn size_hint(&self) -> usize {
        self.size_hint
    }
}

/// A validated zero-copy adapter over an indexed adjacency list.
pub struct IndexedTree<'a, C = Vec<usize>>
where
    C: AsRef<[usize]>,
{
    roots: SmallVec<[usize; 1]>,
    children: &'a [C],
    revision: TreeRevision,
}

impl<'a, C> IndexedTree<'a, C>
where
    C: AsRef<[usize]>,
{
    /// Checks bounds, unique parents, and the absence of cycles.
    ///
    /// # Errors
    ///
    /// Returns [`IndexedTreeError`] when a root or child is invalid, a node has multiple
    /// parents, the root set is incomplete, or the graph contains a cycle.
    pub fn new(
        roots: impl IntoIterator<Item = usize>,
        children: &'a [C],
        revision: TreeRevision,
    ) -> Result<Self, IndexedTreeError> {
        let roots: SmallVec<[usize; 1]> = roots.into_iter().collect();
        let mut has_parent = vec![false; children.len()];
        let mut root_seen = vec![false; children.len()];
        for root in roots.iter().copied() {
            let Some(seen) = root_seen.get_mut(root) else {
                return Err(IndexedTreeError::InvalidRoot(root));
            };
            if *seen {
                return Err(IndexedTreeError::DuplicateRoot(root));
            }
            *seen = true;
        }

        for (parent, node_children) in children.iter().enumerate() {
            for &child in node_children.as_ref() {
                let Some(seen) = has_parent.get_mut(child) else {
                    return Err(IndexedTreeError::InvalidChild { parent, child });
                };
                if *seen {
                    return Err(IndexedTreeError::MultipleParents(child));
                }
                *seen = true;
            }
        }
        if let Some(root) = roots.iter().copied().find(|root| has_parent[*root]) {
            return Err(IndexedTreeError::RootHasParent(root));
        }
        if let Some(root) = has_parent
            .iter()
            .zip(&root_seen)
            .position(|(&has_parent, &is_root)| !has_parent && !is_root)
        {
            return Err(IndexedTreeError::MissingRoot(root));
        }

        // При единственном родителе цикл не достижим из корней: достаточно посчитать достижимые узлы.
        let mut stack = roots.clone();
        let mut processed = 0;
        while let Some(id) = stack.pop() {
            processed += 1;
            stack.extend_from_slice(children[id].as_ref());
        }
        if processed != children.len() {
            return Err(IndexedTreeError::Cycle);
        }

        Ok(Self {
            roots,
            children,
            revision,
        })
    }
}

impl<C> TreeModel for IndexedTree<'_, C>
where
    C: AsRef<[usize]>,
{
    type Id = usize;

    fn roots(&self) -> impl Iterator<Item = Self::Id> + '_ {
        self.roots.iter().copied()
    }

    fn children(&self, id: Self::Id) -> TreeChildren<'_, Self::Id> {
        TreeChildren::loaded(self.children[id].as_ref())
    }

    fn revision(&self) -> TreeRevision {
        self.revision
    }

    fn size_hint(&self) -> usize {
        self.children.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexed_tree_reports_invalid_structure() {
        let cases = [
            (vec![1], vec![vec![]], IndexedTreeError::InvalidRoot(1)),
            (vec![0, 0], vec![vec![]], IndexedTreeError::DuplicateRoot(0)),
            (vec![], vec![vec![]], IndexedTreeError::MissingRoot(0)),
            (
                vec![0],
                vec![vec![1]],
                IndexedTreeError::InvalidChild {
                    parent: 0,
                    child: 1,
                },
            ),
            (
                vec![0, 1],
                vec![vec![1], vec![]],
                IndexedTreeError::RootHasParent(1),
            ),
            (
                vec![0],
                vec![vec![1, 1], vec![]],
                IndexedTreeError::MultipleParents(1),
            ),
            (
                vec![0],
                vec![vec![], vec![2], vec![1]],
                IndexedTreeError::Cycle,
            ),
            (vec![], vec![vec![0]], IndexedTreeError::Cycle),
        ];
        for (roots, children, expected) in cases {
            assert_eq!(
                IndexedTree::new(roots, &children, TreeRevision::INITIAL).err(),
                Some(expected)
            );
        }
    }

    #[test]
    fn indexed_tree_accepts_empty_and_multiple_root_forests() {
        let empty: [Vec<usize>; 0] = [];
        let tree = IndexedTree::new([], &empty, TreeRevision::INITIAL).expect("valid empty forest");
        assert_eq!(tree.size_hint(), 0);
        assert_eq!(tree.roots().count(), 0);

        let children = [vec![2, 3], vec![], vec![], vec![]];
        let tree = IndexedTree::new([1, 0], &children, TreeRevision::INITIAL)
            .expect("valid forest with multiple roots");
        assert_eq!(tree.roots().collect::<Vec<_>>(), [1, 0]);
        assert_eq!(tree.children(0).loaded_slice(), &[2, 3]);
    }

    #[test]
    fn indexed_tree_rejects_shared_nodes_and_cycles() {
        let shared = vec![vec![2], vec![2], vec![]];
        assert!(matches!(
            IndexedTree::new([0, 1], &shared, TreeRevision::INITIAL),
            Err(IndexedTreeError::MultipleParents(2))
        ));

        let cycle = vec![vec![1], vec![0]];
        assert!(matches!(
            IndexedTree::new([], &cycle, TreeRevision::INITIAL),
            Err(IndexedTreeError::Cycle)
        ));

        let generic: [&[usize]; 2] = [&[1], &[]];
        let tree = IndexedTree::new([0], &generic, TreeRevision::INITIAL)
            .expect("slice-backed adjacency list is valid");
        assert_eq!(tree.children(0).loaded_slice(), &[1]);
    }
}
