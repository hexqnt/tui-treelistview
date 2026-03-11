use std::hash::Hash;

use crate::action::{TreeAction, TreeEvent};
use crate::model::{TreeFilter, TreeFilterConfig, TreeModel};

#[cfg(feature = "edit")]
use crate::edit::TreeEdit;

#[cfg(feature = "keymap")]
use crossterm::event::KeyEvent;

use super::TreeListViewState;

impl<Id: Copy + Eq + Hash> TreeListViewState<Id> {
    /// Handles a tree action and returns the resulting event.
    pub fn handle_action<T: TreeModel<Id = Id>, C>(
        &mut self,
        model: &T,
        action: TreeAction<C>,
    ) -> TreeEvent<C> {
        self.ensure_visible_nodes(model);
        let mut rebuild_visible = |state: &mut Self, model: &T| state.update_visible_nodes(model);
        self.handle_action_inner(model, action, &mut rebuild_visible)
    }

    /// Handles a tree action with filtering enabled and returns the resulting event.
    pub fn handle_action_filtered<T, F, C>(
        &mut self,
        model: &T,
        filter: &F,
        config: TreeFilterConfig,
        action: TreeAction<C>,
    ) -> TreeEvent<C>
    where
        T: TreeModel<Id = Id>,
        F: TreeFilter<T>,
    {
        self.ensure_visible_nodes_filtered(model, filter, config);
        let mut rebuild_visible = |state: &mut Self, model: &T| {
            state.ensure_visible_nodes_filtered(model, filter, config);
        };
        self.handle_action_inner(model, action, &mut rebuild_visible)
    }

    #[cfg(feature = "edit")]
    /// Applies edit actions to a mutable model and updates state.
    pub fn handle_edit_action<T: TreeEdit<Id = Id>, C>(
        &mut self,
        model: &mut T,
        action: TreeAction<C>,
        clipboard: &mut Option<Id>,
    ) -> bool {
        self.ensure_visible_nodes(model);
        match action {
            TreeAction::ReorderUp => self.reorder_selected(model, |model, parent, child| {
                model.move_child_up(parent, child)
            }),
            TreeAction::ReorderDown => self.reorder_selected(model, |model, parent, child| {
                model.move_child_down(parent, child)
            }),
            TreeAction::DetachNode => {
                if let Some((node_id, parent_id)) = self.selected_node_with_parent() {
                    if model.is_root(node_id) {
                        return false;
                    }
                    model.remove_child(parent_id, node_id);
                    self.prune_removed_marks(model);
                    self.invalidate_all();
                    return true;
                }
                false
            }
            TreeAction::DeleteNode => {
                if let Some(node_id) = self.selected_id() {
                    if model.is_root(node_id) {
                        return false;
                    }
                    model.delete_node(node_id);
                    self.prune_removed_marks(model);
                    self.invalidate_all();
                    return true;
                }
                false
            }
            TreeAction::YankNode => {
                if let Some(node_id) = self.selected_id() {
                    if model.is_root(node_id) {
                        return false;
                    }
                    *clipboard = Some(node_id);
                    return true;
                }
                false
            }
            TreeAction::PasteNode => {
                if let Some(node_id) = *clipboard
                    && let Some(parent_id) = self.selected_node().map(|node| node.id)
                {
                    model.add_child(parent_id, node_id);
                    self.invalidate_all();
                    return true;
                }
                false
            }
            TreeAction::ExpandAll => {
                self.expand_all(model);
                true
            }
            TreeAction::CollapseAll => {
                self.collapse_all();
                true
            }
            TreeAction::Custom(custom) => {
                drop(custom);
                false
            }
            _ => false,
        }
    }

    #[cfg(feature = "edit")]
    fn reorder_selected<T, M>(&mut self, model: &mut T, mut move_child: M) -> bool
    where
        T: TreeEdit<Id = Id>,
        M: FnMut(&mut T, Id, Id) -> bool,
    {
        if let Some((node_id, parent_id)) = self.selected_node_with_parent()
            && move_child(model, parent_id, node_id)
        {
            self.invalidate();
            self.ensure_visible_nodes(model);
            if let Some(idx) = self.visible_index_of(node_id) {
                self.list_state.select(Some(idx));
            }
            return true;
        }
        false
    }

    #[cfg(feature = "keymap")]
    /// Resolves a key event into an action and handles it.
    pub fn handle_key<T: TreeModel<Id = Id>>(&mut self, model: &T, key: KeyEvent) -> TreeEvent<()> {
        self.ensure_visible_nodes(model);
        let Some(action) = self.keymap.resolve(key) else {
            return TreeEvent::Unhandled;
        };
        let mut rebuild_visible = |state: &mut Self, model: &T| state.update_visible_nodes(model);
        self.handle_action_inner(model, action, &mut rebuild_visible)
    }

    #[cfg(feature = "keymap")]
    /// Resolves a key event with a custom mapping and handles it.
    pub fn handle_key_with<T, C, F>(&mut self, model: &T, key: KeyEvent, custom: F) -> TreeEvent<C>
    where
        T: TreeModel<Id = Id>,
        F: Fn(KeyEvent) -> Option<C>,
    {
        self.ensure_visible_nodes(model);
        let Some(action) = self.keymap.resolve_with(key, custom) else {
            return TreeEvent::Unhandled;
        };
        let mut rebuild_visible = |state: &mut Self, model: &T| state.update_visible_nodes(model);
        self.handle_action_inner(model, action, &mut rebuild_visible)
    }

    #[cfg(feature = "keymap")]
    /// Resolves a key event with filtering enabled and handles it.
    pub fn handle_key_filtered<T, F>(
        &mut self,
        model: &T,
        filter: &F,
        config: TreeFilterConfig,
        key: KeyEvent,
    ) -> TreeEvent<()>
    where
        T: TreeModel<Id = Id>,
        F: TreeFilter<T>,
    {
        self.ensure_visible_nodes_filtered(model, filter, config);
        let Some(action) = self.keymap.resolve(key) else {
            return TreeEvent::Unhandled;
        };
        let mut rebuild_visible = |state: &mut Self, model: &T| {
            state.ensure_visible_nodes_filtered(model, filter, config);
        };
        self.handle_action_inner(model, action, &mut rebuild_visible)
    }

    #[cfg(feature = "keymap")]
    /// Resolves a key event with filtering and custom mapping enabled and handles it.
    pub fn handle_key_filtered_with<T, F, C, R>(
        &mut self,
        model: &T,
        filter: &F,
        config: TreeFilterConfig,
        key: KeyEvent,
        custom: R,
    ) -> TreeEvent<C>
    where
        T: TreeModel<Id = Id>,
        F: TreeFilter<T>,
        R: Fn(KeyEvent) -> Option<C>,
    {
        self.ensure_visible_nodes_filtered(model, filter, config);
        let Some(action) = self.keymap.resolve_with(key, custom) else {
            return TreeEvent::Unhandled;
        };
        let mut rebuild_visible = |state: &mut Self, model: &T| {
            state.ensure_visible_nodes_filtered(model, filter, config);
        };
        self.handle_action_inner(model, action, &mut rebuild_visible)
    }

    fn handle_action_inner<T, C, R>(
        &mut self,
        model: &T,
        action: TreeAction<C>,
        rebuild_visible: &mut R,
    ) -> TreeEvent<C>
    where
        T: TreeModel<Id = Id>,
        R: FnMut(&mut Self, &T),
    {
        if matches!(&action, TreeAction::Custom(_)) {
            return TreeEvent::Action(action);
        }

        if self.visible_nodes.is_empty() {
            return TreeEvent::Unhandled;
        }

        match action {
            TreeAction::SelectPrev => {
                self.select_prev();
                TreeEvent::Handled
            }
            TreeAction::SelectNext => {
                self.select_next();
                TreeEvent::Handled
            }
            TreeAction::SelectParent => {
                self.select_parent();
                TreeEvent::Handled
            }
            TreeAction::SelectChild => {
                self.select_child_with_descendants(model, rebuild_visible);
                TreeEvent::Handled
            }
            TreeAction::ToggleRecursive => {
                if let Some(node) = self.selected_node()
                    && node.has_children
                {
                    let should_expand = !self.is_expanded(node.parent, node.id);
                    self.set_expanded_recursive(model, node.id, node.parent, should_expand);
                    self.dirty = true;
                    return TreeEvent::Handled;
                }
                TreeEvent::Unhandled
            }
            TreeAction::ToggleNode => {
                if let Some(node) = self.selected_node()
                    && node.has_children
                {
                    self.toggle(node.id, node.parent);
                    return TreeEvent::Handled;
                }
                TreeEvent::Unhandled
            }
            TreeAction::ExpandAll => {
                self.expand_all(model);
                TreeEvent::Handled
            }
            TreeAction::CollapseAll => {
                self.collapse_all();
                TreeEvent::Handled
            }
            TreeAction::ToggleGuides => {
                self.draw_lines = !self.draw_lines;
                TreeEvent::Handled
            }
            TreeAction::ToggleMark => {
                if let Some(node_id) = self.selected_id() {
                    self.toggle_node_mark(node_id);
                    return TreeEvent::Handled;
                }
                TreeEvent::Unhandled
            }
            TreeAction::SelectFirst => {
                self.select_first();
                TreeEvent::Handled
            }
            TreeAction::SelectLast => {
                self.select_last();
                TreeEvent::Handled
            }
            TreeAction::ReorderUp
            | TreeAction::ReorderDown
            | TreeAction::AddChild
            | TreeAction::EditNode
            | TreeAction::DetachNode
            | TreeAction::DeleteNode
            | TreeAction::YankNode
            | TreeAction::PasteNode
            | TreeAction::Custom(_) => TreeEvent::Action(action),
        }
    }
}
