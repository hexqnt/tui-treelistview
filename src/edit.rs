use crate::model::TreeModel;

/// Optional edit interface for basic tree mutations.
pub trait TreeEdit: TreeModel {
    /// Returns `true` if the node is the root node.
    fn is_root(&self, id: Self::Id) -> bool;
    /// Moves the child one position up among its siblings.
    fn move_child_up(&mut self, parent: Self::Id, child: Self::Id) -> bool;
    /// Moves the child one position down among its siblings.
    fn move_child_down(&mut self, parent: Self::Id, child: Self::Id) -> bool;
    /// Detaches the child from the parent without deleting the node.
    fn remove_child(&mut self, parent: Self::Id, child: Self::Id);
    /// Removes the node from the tree entirely.
    fn delete_node(&mut self, id: Self::Id);
    /// Attaches the child as a child of the parent.
    fn add_child(&mut self, parent: Self::Id, child: Self::Id);
}
