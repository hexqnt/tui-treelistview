/// Actions that only change view state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeViewAction {
    SelectPrev,
    SelectNext,
    SelectParent,
    SelectFirstChild,
    Expand,
    Collapse,
    ExpandOrSelectFirstChild,
    CollapseOrSelectParent,
    ToggleNode,
    ToggleRecursive,
    ExpandAll,
    CollapseAll,
    ToggleGuides,
    ToggleMark,
    SelectFirst,
    SelectLast,
    SelectColumnLeft,
    SelectColumnRight,
    SelectFirstColumn,
    SelectLastColumn,
    ScrollViewUp,
    ScrollViewDown,
    ScrollLeft,
    ScrollRight,
}

/// High-level editing actions for the selected node.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeEditAction {
    ReorderUp,
    ReorderDown,
    AddChild,
    Rename,
    Detach,
    Delete,
    Yank,
    Paste,
}

/// An action produced by the application or user.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeAction<Custom = ()> {
    View(TreeViewAction),
    Edit(TreeEditAction),
    Custom(Custom),
}

impl<C> From<TreeViewAction> for TreeAction<C> {
    fn from(action: TreeViewAction) -> Self {
        Self::View(action)
    }
}

impl<C> From<TreeEditAction> for TreeAction<C> {
    fn from(action: TreeEditAction) -> Self {
        Self::Edit(action)
    }
}

/// A typed edit request enriched with the current selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeEditRequest<Id> {
    ReorderUp { node: Id, parent: Id },
    ReorderDown { node: Id, parent: Id },
    AddChild { parent: Id },
    Rename { node: Id },
    Detach { node: Id, parent: Id },
    Delete { node: Id },
    Yank { node: Id },
    Paste { parent: Id },
}

/// An intent that must be handled by the application.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeIntent<Id, Custom = ()> {
    LoadChildren(Id),
    Edit(TreeEditRequest<Id>),
    Custom(Custom),
}

/// The result of handling an action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeEvent<Id, Custom = ()> {
    /// View state changed.
    Changed,
    /// The action was valid but did not change state.
    Unchanged,
    /// The application or model must perform an operation.
    Intent(TreeIntent<Id, Custom>),
}
