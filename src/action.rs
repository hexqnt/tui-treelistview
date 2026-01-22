/// Действия, которые может инициировать пользователь/приложение над деревом.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TreeAction<Custom = ()> {
    ReorderUp,
    ReorderDown,
    SelectPrev,
    SelectNext,
    SelectParent,
    SelectChild,
    ToggleRecursive,
    ToggleNode,
    AddChild,
    EditNode,
    /// Удалить связь с родителем (узел может остаться в дереве, если он разделяемый).
    DetachNode,
    /// Полное удаление узла из дерева.
    DeleteNode,
    YankNode,
    PasteNode,
    ToggleGuides,
    ToggleMark,
    SelectFirst,
    SelectLast,
    /// Пользовательское действие (пробрасывается наружу без обработки внутри виджета).
    Custom(Custom),
}

/// Результат обработки действия/клавиши.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TreeEvent<Custom = ()> {
    Handled,
    Unhandled,
    Action(TreeAction<Custom>),
}
