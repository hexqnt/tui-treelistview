/// Actions that a user or application can initiate on the tree view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeAction<Custom = ()> {
    /// Move the selected node up within its siblings.
    ReorderUp,
    /// Move the selected node down within its siblings.
    ReorderDown,
    /// Move selection to the previous visible row.
    SelectPrev,
    /// Move selection to the next visible row.
    SelectNext,
    /// Move selection to the parent node.
    SelectParent,
    /// Expand the selection; if possible, move to an expandable descendant.
    SelectChild,
    /// Toggle expansion recursively for the selected subtree.
    ToggleRecursive,
    /// Toggle expansion for the selected node only.
    ToggleNode,
    /// Expand all nodes in the tree.
    ExpandAll,
    /// Collapse all nodes in the tree.
    CollapseAll,
    /// Request adding a child under the selected node.
    AddChild,
    /// Request editing the selected node.
    EditNode,
    /// Detach the node from its parent without deleting it.
    DetachNode,
    /// Delete the node from the tree entirely.
    DeleteNode,
    /// Store the selected node id in the clipboard.
    YankNode,
    /// Paste the clipboard node as a child of the selected node.
    PasteNode,
    /// Toggle drawing of guide lines.
    ToggleGuides,
    /// Toggle the mark state for the selected node.
    ToggleMark,
    /// Select the first visible row.
    SelectFirst,
    /// Select the last visible row.
    SelectLast,
    /// Custom action forwarded to the caller without internal handling.
    Custom(Custom),
}

/// Result of handling an action or key event.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TreeEvent<Custom = ()> {
    /// The action was handled internally and state was updated.
    Handled,
    /// The action was ignored (e.g., nothing selected / nothing to do).
    Unhandled,
    /// The action is forwarded to the caller for handling.
    Action(TreeAction<Custom>),
}
