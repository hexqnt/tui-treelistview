use smallvec::SmallVec;

use crate::model::TreeModel;

/// An insertion position within a child list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeInsertPosition<Id> {
    First,
    Last,
    Before(Id),
    After(Id),
}

impl<Id: PartialEq> TreeInsertPosition<Id> {
    /// Resolves this logical position against the destination sibling list.
    ///
    /// Returns `None` when a `Before` or `After` anchor is absent.
    #[must_use]
    pub fn index_in(&self, siblings: &[Id]) -> Option<usize> {
        match self {
            Self::First => Some(0),
            Self::Last => Some(siblings.len()),
            Self::Before(anchor) => siblings.iter().position(|sibling| sibling == anchor),
            Self::After(anchor) => siblings
                .iter()
                .position(|sibling| sibling == anchor)
                .and_then(|index| index.checked_add(1)),
        }
    }
}

/// An atomic model editing command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TreeEditCommand<Id> {
    CreateChild {
        parent: Id,
    },
    Rename {
        node: Id,
    },
    Move {
        nodes: SmallVec<[Id; 4]>,
        parent: Id,
        position: TreeInsertPosition<Id>,
    },
    Detach {
        nodes: SmallVec<[Id; 4]>,
    },
    Delete {
        nodes: SmallVec<[Id; 4]>,
    },
}

/// A selection update after a successful edit.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TreeSelectionUpdate<Id> {
    #[default]
    Keep,
    Select(Id),
    Clear,
}

/// The exact model changes needed to reconcile widget state.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TreeChangeSet<Id> {
    pub inserted: SmallVec<[Id; 4]>,
    pub moved: SmallVec<[Id; 4]>,
    pub removed: SmallVec<[Id; 4]>,
    pub selection: TreeSelectionUpdate<Id>,
}

/// Applies typed editing commands to a domain model.
pub trait TreeEditor: TreeModel {
    type Error;

    /// Applies a command atomically and returns the actual changes.
    ///
    /// # Errors
    ///
    /// Returns a model-specific error when the command cannot be applied atomically.
    fn apply(
        &mut self,
        command: TreeEditCommand<Self::Id>,
    ) -> Result<TreeChangeSet<Self::Id>, Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::TreeInsertPosition;

    #[test]
    fn insert_positions_resolve_against_siblings() {
        let siblings = [10, 20, 30];
        assert_eq!(TreeInsertPosition::First.index_in(&siblings), Some(0));
        assert_eq!(TreeInsertPosition::Last.index_in(&siblings), Some(3));
        assert_eq!(TreeInsertPosition::Before(20).index_in(&siblings), Some(1));
        assert_eq!(TreeInsertPosition::After(20).index_in(&siblings), Some(2));
        assert_eq!(TreeInsertPosition::Before(40).index_in(&siblings), None);
    }
}
