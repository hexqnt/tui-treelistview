use std::hash::Hash;

#[cfg(feature = "keymap")]
use crossterm::event::KeyEvent;

use crate::action::{
    TreeAction, TreeEditAction, TreeEditRequest, TreeEvent, TreeIntent, TreeViewAction,
};
use crate::columns::TreeColumns;
use crate::context::TreeExpansionState;
use crate::edit::{TreeChangeSet, TreeEditCommand, TreeEditor, TreeSelectionUpdate};
use crate::model::{TreeFilter, TreeModel, TreeQuery, TreeSort};

use super::TreeListViewState;

#[derive(Clone, Copy)]
enum ExpansionAction {
    Expand,
    Toggle,
}

impl<Id: Copy + Eq + Hash> TreeListViewState<Id> {
    /// Handles an action against the current projection.
    pub fn handle_action<T, F, S, C, Custom>(
        &mut self,
        model: &T,
        query: &TreeQuery<F, S>,
        columns: &C,
        action: TreeAction<Custom>,
    ) -> TreeEvent<Id, Custom>
    where
        T: TreeModel<Id = Id>,
        F: TreeFilter<T>,
        S: TreeSort<T>,
        C: TreeColumns<T>,
    {
        self.ensure_projection(model, query);
        let event = match action {
            TreeAction::View(action) => {
                self.handle_view_action(model, columns.column_count(), action)
            }
            TreeAction::Edit(action) => self.handle_edit_intent(action),
            TreeAction::Custom(custom) => TreeEvent::Intent(TreeIntent::Custom(custom)),
        };
        if matches!(event, TreeEvent::Changed) {
            self.ensure_projection(model, query);
        }
        event
    }

    /// Applies a command through the model, reconciles persistent state, and rebuilds the projection.
    ///
    /// # Errors
    ///
    /// Returns the model-specific error from [`TreeEditor::apply`] without changing view state.
    pub fn apply_edit<T, F, S>(
        &mut self,
        model: &mut T,
        query: &TreeQuery<F, S>,
        command: TreeEditCommand<Id>,
    ) -> Result<TreeChangeSet<Id>, T::Error>
    where
        T: TreeEditor<Id = Id>,
        F: TreeFilter<T>,
        S: TreeSort<T>,
    {
        let changes = model.apply(command)?;
        self.reconcile_changes(&changes);
        if let TreeSelectionUpdate::Select(id) = changes.selection {
            self.expand_to(model, id);
        }
        self.ensure_projection(model, query);
        Ok(changes)
    }

    /// Reconciles marks, expansion, and selection with an exact model change set.
    pub fn reconcile_changes(&mut self, changes: &TreeChangeSet<Id>) {
        self.expanded.retain(|path| {
            !changes.removed.contains(&path.id)
                && !path
                    .parent
                    .is_some_and(|parent| changes.removed.contains(&parent))
                && !changes.moved.contains(&path.id)
        });

        self.manual_marked
            .retain(|id| !changes.removed.contains(id));

        match changes.selection {
            TreeSelectionUpdate::Keep => {}
            TreeSelectionUpdate::Select(id) => {
                self.selected = Some(id);
                self.selection_needs_visibility = true;
            }
            TreeSelectionUpdate::Clear => {
                self.selected = None;
                self.selection_needs_visibility = false;
            }
        }
    }

    fn handle_view_action<T, C>(
        &mut self,
        model: &T,
        column_count: usize,
        action: TreeViewAction,
    ) -> TreeEvent<Id, C>
    where
        T: TreeModel<Id = Id>,
    {
        let changed = match action {
            TreeViewAction::SelectPrev => self.select_prev(),
            TreeViewAction::SelectNext => self.select_next(),
            TreeViewAction::SelectParent => self.select_parent(),
            TreeViewAction::SelectFirstChild => self.select_first_child(),
            TreeViewAction::Expand => {
                return self.change_selected_expansion(ExpansionAction::Expand);
            }
            TreeViewAction::Collapse => self.collapse_selected(),
            TreeViewAction::ExpandOrSelectFirstChild => {
                return self.expand_or_select_first_child();
            }
            TreeViewAction::CollapseOrSelectParent => {
                if self.collapse_selected() {
                    true
                } else {
                    self.select_parent()
                }
            }
            TreeViewAction::ToggleNode => {
                return self.change_selected_expansion(ExpansionAction::Toggle);
            }
            TreeViewAction::ToggleRecursive => return self.toggle_selected_recursive(model),
            TreeViewAction::ExpandAll => self.expand_all(model),
            TreeViewAction::CollapseAll => self.collapse_all(),
            TreeViewAction::ToggleGuides => {
                self.draw_lines = !self.draw_lines;
                true
            }
            TreeViewAction::ToggleMark => self
                .selected
                .is_some_and(|selected| self.toggle_marked(selected)),
            TreeViewAction::SelectFirst => self.select_first(),
            TreeViewAction::SelectLast => self.select_last(),
            TreeViewAction::SelectColumnLeft => self.select_column_left(column_count),
            TreeViewAction::SelectColumnRight => self.select_column_right(column_count),
            TreeViewAction::SelectFirstColumn => {
                self.select_column((column_count > 0).then_some(0), column_count)
            }
            TreeViewAction::SelectLastColumn => self.select_column(
                (column_count > 0).then_some(column_count.saturating_sub(1)),
                column_count,
            ),
            TreeViewAction::ScrollViewUp => self.scroll_view_by(-1),
            TreeViewAction::ScrollViewDown => self.scroll_view_by(1),
            TreeViewAction::ScrollLeft => self.scroll_horizontal_by(-1),
            TreeViewAction::ScrollRight => self.scroll_horizontal_by(1),
        };
        changed_event(changed)
    }

    fn change_selected_expansion<C>(&mut self, action: ExpansionAction) -> TreeEvent<Id, C> {
        let Some(node) = self.selected_node() else {
            return TreeEvent::Unchanged;
        };
        match node.expansion() {
            TreeExpansionState::Collapsed => {
                self.set_expanded(node.id(), node.parent(), true);
                TreeEvent::Changed
            }
            TreeExpansionState::Expanded if matches!(action, ExpansionAction::Toggle) => {
                self.set_expanded(node.id(), node.parent(), false);
                TreeEvent::Changed
            }
            TreeExpansionState::Unloaded => TreeEvent::Intent(TreeIntent::LoadChildren(node.id())),
            TreeExpansionState::Leaf
            | TreeExpansionState::Expanded
            | TreeExpansionState::ForcedByFilter
            | TreeExpansionState::Loading => TreeEvent::Unchanged,
        }
    }

    fn collapse_selected(&mut self) -> bool {
        let Some(node) = self.selected_node() else {
            return false;
        };
        matches!(node.expansion(), TreeExpansionState::Expanded)
            && self.set_expanded(node.id(), node.parent(), false)
    }

    fn expand_or_select_first_child<C>(&mut self) -> TreeEvent<Id, C> {
        let event = self.change_selected_expansion(ExpansionAction::Expand);
        match event {
            TreeEvent::Unchanged => changed_event(self.select_first_child()),
            TreeEvent::Changed | TreeEvent::Intent(_) => event,
        }
    }

    fn toggle_selected_recursive<T, C>(&mut self, model: &T) -> TreeEvent<Id, C>
    where
        T: TreeModel<Id = Id>,
    {
        let Some(node) = self.selected_node() else {
            return TreeEvent::Unchanged;
        };
        match node.expansion() {
            TreeExpansionState::Collapsed | TreeExpansionState::Expanded => {
                let expand = matches!(node.expansion(), TreeExpansionState::Collapsed);
                changed_event(self.set_expanded_recursive(model, node.id(), node.parent(), expand))
            }
            TreeExpansionState::Unloaded => TreeEvent::Intent(TreeIntent::LoadChildren(node.id())),
            TreeExpansionState::Leaf
            | TreeExpansionState::ForcedByFilter
            | TreeExpansionState::Loading => TreeEvent::Unchanged,
        }
    }

    fn handle_edit_intent<C>(&self, action: TreeEditAction) -> TreeEvent<Id, C> {
        let Some(node) = self.selected_node() else {
            return TreeEvent::Unchanged;
        };
        let request = match action {
            TreeEditAction::ReorderUp => {
                let Some(parent) = node.parent() else {
                    return TreeEvent::Unchanged;
                };
                TreeEditRequest::ReorderUp {
                    node: node.id(),
                    parent,
                }
            }
            TreeEditAction::ReorderDown => {
                let Some(parent) = node.parent() else {
                    return TreeEvent::Unchanged;
                };
                TreeEditRequest::ReorderDown {
                    node: node.id(),
                    parent,
                }
            }
            TreeEditAction::AddChild => TreeEditRequest::AddChild { parent: node.id() },
            TreeEditAction::Rename => TreeEditRequest::Rename { node: node.id() },
            TreeEditAction::Detach => {
                let Some(parent) = node.parent() else {
                    return TreeEvent::Unchanged;
                };
                TreeEditRequest::Detach {
                    node: node.id(),
                    parent,
                }
            }
            TreeEditAction::Delete => TreeEditRequest::Delete { node: node.id() },
            TreeEditAction::Yank => TreeEditRequest::Yank { node: node.id() },
            TreeEditAction::Paste => TreeEditRequest::Paste { parent: node.id() },
        };
        TreeEvent::Intent(TreeIntent::Edit(request))
    }

    #[cfg(feature = "keymap")]
    /// Resolves a crossterm event into an action and handles it.
    pub fn handle_key<T, F, S, C>(
        &mut self,
        model: &T,
        query: &TreeQuery<F, S>,
        columns: &C,
        key: KeyEvent,
    ) -> TreeEvent<Id>
    where
        T: TreeModel<Id = Id>,
        F: TreeFilter<T>,
        S: TreeSort<T>,
        C: TreeColumns<T>,
    {
        self.handle_key_with(model, query, columns, key, |_| None::<()>)
    }

    #[cfg(feature = "keymap")]
    /// A version of [`handle_key`](Self::handle_key) with custom mapping.
    pub fn handle_key_with<T, F, S, C, Custom, R>(
        &mut self,
        model: &T,
        query: &TreeQuery<F, S>,
        columns: &C,
        key: KeyEvent,
        custom: R,
    ) -> TreeEvent<Id, Custom>
    where
        T: TreeModel<Id = Id>,
        F: TreeFilter<T>,
        S: TreeSort<T>,
        C: TreeColumns<T>,
        R: Fn(KeyEvent) -> Option<Custom>,
    {
        let Some(action) = self.keymap.resolve_with(key, custom) else {
            return TreeEvent::Unchanged;
        };
        self.handle_action(model, query, columns, action)
    }
}
const fn changed_event<Id, Custom>(changed: bool) -> TreeEvent<Id, Custom> {
    if changed {
        TreeEvent::Changed
    } else {
        TreeEvent::Unchanged
    }
}

