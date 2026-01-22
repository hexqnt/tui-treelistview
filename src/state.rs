use std::hash::Hash;

use ratatui::widgets::TableState;
use rustc_hash::{FxBuildHasher, FxHashMap, FxHashSet};
use smallvec::SmallVec;

use crate::action::{TreeAction, TreeEvent};
use crate::model::{TreeFilter, TreeFilterConfig, TreeModel};
use crate::style::TreeScrollPolicy;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[cfg(feature = "edit")]
use crate::edit::TreeEdit;

#[cfg(feature = "keymap")]
use crate::keymap::TreeKeyBindings;
#[cfg(feature = "keymap")]
use crossterm::event::KeyEvent;

#[derive(Clone)]
pub struct VisibleNode<Id> {
    pub(crate) id: Id,
    pub(crate) level: u16,
    pub(crate) parent: Option<Id>,
    pub(crate) is_tail_stack: SmallVec<[bool; 8]>,
}

/// Состояние виджета: развёрнутые узлы, выделение, кеши видимости и меток.
pub struct TreeListViewState<Id> {
    list_state: TableState,
    expanded: FxHashSet<(Option<Id>, Id)>,
    visible_nodes: Vec<VisibleNode<Id>>,
    dirty: bool,
    manual_marked: FxHashSet<Id>,
    effective_marked: FxHashSet<Id>,
    marks_dirty: bool,
    draw_lines: bool,
    #[cfg(feature = "keymap")]
    keymap: TreeKeyBindings,
}

#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[derive(Clone, Debug)]
/// Сериализуемый снимок состояния (выбор, развёрнутые узлы, метки).
pub struct TreeListViewSnapshot<Id> {
    pub expanded: Vec<(Option<Id>, Id)>,
    pub manual_marked: Vec<Id>,
    pub selected: Option<usize>,
    pub selected_column: Option<usize>,
    pub offset: usize,
    pub draw_lines: bool,
}

impl<Id: Copy + Eq + Hash> Default for TreeListViewState<Id> {
    fn default() -> Self {
        Self::new()
    }
}

impl<Id: Copy + Eq + Hash> TreeListViewState<Id> {
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            list_state: TableState::default(),
            expanded: FxHashSet::with_capacity_and_hasher(capacity, FxBuildHasher),
            visible_nodes: Vec::with_capacity(capacity),
            dirty: true,
            manual_marked: FxHashSet::with_capacity_and_hasher(capacity, FxBuildHasher),
            effective_marked: FxHashSet::with_capacity_and_hasher(capacity, FxBuildHasher),
            marks_dirty: true,
            draw_lines: true,
            #[cfg(feature = "keymap")]
            keymap: TreeKeyBindings::new(),
        }
    }

    #[cfg(feature = "keymap")]
    pub const fn keymap_mut(&mut self) -> &mut TreeKeyBindings {
        &mut self.keymap
    }

    pub(crate) const fn list_state(&self) -> &TableState {
        &self.list_state
    }

    pub(crate) const fn list_state_mut(&mut self) -> &mut TableState {
        &mut self.list_state
    }

    pub(crate) fn visible_nodes(&self) -> &[VisibleNode<Id>] {
        &self.visible_nodes
    }

    pub(crate) fn is_expanded(&self, parent: Option<Id>, id: Id) -> bool {
        self.expanded.contains(&(parent, id))
    }

    pub fn snapshot(&self) -> TreeListViewSnapshot<Id> {
        TreeListViewSnapshot {
            expanded: self.expanded.iter().copied().collect(),
            manual_marked: self.manual_marked.iter().copied().collect(),
            selected: self.list_state.selected(),
            selected_column: self.list_state.selected_column(),
            offset: self.list_state.offset(),
            draw_lines: self.draw_lines,
        }
    }

    pub fn restore(&mut self, snapshot: TreeListViewSnapshot<Id>) {
        self.expanded = snapshot.expanded.into_iter().collect();
        self.manual_marked = snapshot.manual_marked.into_iter().collect();
        self.draw_lines = snapshot.draw_lines;
        *self.list_state.offset_mut() = snapshot.offset;
        self.list_state.select(snapshot.selected);
        *self.list_state.selected_column_mut() = snapshot.selected_column;
        self.dirty = true;
        self.marks_dirty = true;
    }

    pub const fn draw_lines(&self) -> bool {
        self.draw_lines
    }

    pub const fn set_draw_lines(&mut self, draw: bool) {
        self.draw_lines = draw;
    }

    pub const fn invalidate(&mut self) {
        self.dirty = true;
    }

    pub const fn invalidate_all(&mut self) {
        self.dirty = true;
        self.marks_dirty = true;
    }

    pub const fn select_first(&mut self) {
        self.list_state.select_first();
    }

    pub const fn select_last(&mut self) {
        self.list_state.select_last();
    }

    pub fn scroll_down_by(&mut self, amount: u16) {
        self.list_state.scroll_down_by(amount);
    }

    pub fn scroll_up_by(&mut self, amount: u16) {
        self.list_state.scroll_up_by(amount);
    }

    pub fn select_prev(&mut self) {
        if self.visible_nodes.is_empty() {
            self.list_state.select(None);
            return;
        }
        let selected = self.list_state.selected().unwrap_or(0);
        self.list_state.select(Some(selected.saturating_sub(1)));
    }

    pub fn select_next(&mut self) {
        if self.visible_nodes.is_empty() {
            self.list_state.select(None);
            return;
        }
        let selected = self.list_state.selected().unwrap_or(0);
        let new_selected = (selected + 1).min(self.visible_nodes.len().saturating_sub(1));
        self.list_state.select(Some(new_selected));
    }

    pub fn ensure_selection_visible(&mut self, viewport_height: usize) {
        self.clamp_selection();
        let Some(selected) = self.list_state.selected() else {
            return;
        };
        let viewport_height = viewport_height.max(1);
        let offset = self.list_state.offset();
        if selected < offset {
            *self.list_state.offset_mut() = selected;
        } else if selected >= offset + viewport_height {
            *self.list_state.offset_mut() = selected + 1 - viewport_height;
        }
    }

    pub fn ensure_selection_visible_with_policy(
        &mut self,
        viewport_height: usize,
        policy: TreeScrollPolicy,
    ) {
        match policy {
            TreeScrollPolicy::KeepInView => self.ensure_selection_visible(viewport_height),
            TreeScrollPolicy::CenterOnSelect => {
                self.ensure_selection_visible_centered(viewport_height);
            }
        }
    }

    fn ensure_selection_visible_centered(&mut self, viewport_height: usize) {
        self.clamp_selection();
        let Some(selected) = self.list_state.selected() else {
            return;
        };
        let viewport_height = viewport_height.max(1);
        let total = self.visible_nodes.len();
        if total <= viewport_height {
            *self.list_state.offset_mut() = 0;
            return;
        }

        let half = viewport_height / 2;
        let mut offset = selected.saturating_sub(half);
        let max_offset = total.saturating_sub(viewport_height);
        if offset > max_offset {
            offset = max_offset;
        }
        *self.list_state.offset_mut() = offset;
    }

    pub fn selected_id(&self) -> Option<Id> {
        self.list_state
            .selected()
            .and_then(|idx| self.visible_nodes.get(idx).map(|node| node.id))
    }

    pub fn selected_parent_id(&self) -> Option<Id> {
        self.list_state
            .selected()
            .and_then(|idx| self.visible_nodes.get(idx).and_then(|node| node.parent))
    }

    pub const fn visible_len(&self) -> usize {
        self.visible_nodes.len()
    }

    pub fn selected_level(&self) -> Option<u16> {
        self.list_state
            .selected()
            .and_then(|idx| self.visible_nodes.get(idx).map(|node| node.level))
    }

    pub fn selected_is_expanded<T: TreeModel<Id = Id>>(&self, model: &T) -> Option<bool> {
        self.list_state.selected().and_then(|idx| {
            self.visible_nodes.get(idx).map(|node| {
                if model.children(node.id).is_empty() {
                    false
                } else {
                    self.expanded.contains(&(node.parent, node.id))
                }
            })
        })
    }

    pub fn select_by_id<T: TreeModel<Id = Id>>(&mut self, model: &T, id: Id) -> bool {
        let _ = self.expand_to(model, id);
        self.ensure_visible_nodes(model);
        if let Some(idx) = self.visible_nodes.iter().position(|node| node.id == id) {
            self.list_state.select(Some(idx));
            true
        } else {
            false
        }
    }

    pub fn ensure_visible_id<T: TreeModel<Id = Id>>(&mut self, model: &T, id: Id) -> bool {
        let _ = self.expand_to(model, id);
        self.ensure_visible_nodes(model);
        self.visible_nodes.iter().any(|node| node.id == id)
    }

    pub fn expand_to<T: TreeModel<Id = Id>>(&mut self, model: &T, id: Id) -> bool {
        let Some(path) = Self::find_path_to(model, id) else {
            return false;
        };
        for (parent, node) in path {
            if !model.children(node).is_empty() {
                self.expanded.insert((parent, node));
            }
        }
        self.dirty = true;
        true
    }

    pub fn expand_all<T: TreeModel<Id = Id>>(&mut self, model: &T) {
        self.expanded.clear();
        if let Some(root) = model.root() {
            let mut stack = vec![(None, root)];
            while let Some((parent, node)) = stack.pop() {
                let children = model.children(node);
                if !children.is_empty() {
                    self.expanded.insert((parent, node));
                    for child in children.iter().copied() {
                        stack.push((Some(node), child));
                    }
                }
            }
        }
        self.dirty = true;
    }

    pub fn collapse_all(&mut self) {
        self.expanded.clear();
        self.dirty = true;
    }

    pub fn ensure_visible_nodes<T: TreeModel<Id = Id>>(&mut self, model: &T) {
        if !self.dirty {
            return;
        }
        self.update_visible_nodes(model);
    }

    pub fn ensure_visible_nodes_filtered<T, F>(
        &mut self,
        model: &T,
        filter: &F,
        config: TreeFilterConfig,
    ) where
        T: TreeModel<Id = Id>,
        F: TreeFilter<T>,
    {
        if !self.dirty {
            return;
        }
        if !config.enabled {
            self.update_visible_nodes(model);
            return;
        }

        self.visible_nodes.clear();
        if let Some(root) = model.root() {
            let mut is_tail_stack: SmallVec<[bool; 8]> = SmallVec::new();
            let mut memo: FxHashMap<Id, bool> = FxHashMap::default();
            self.build_visible_nodes_filtered(
                model,
                root,
                0,
                None,
                &mut is_tail_stack,
                filter,
                config,
                &mut memo,
            );
        }
        self.dirty = false;
        self.clamp_selection();
    }

    pub fn ensure_mark_cache<T: TreeModel<Id = Id>>(&mut self, model: &T) {
        if !self.marks_dirty {
            return;
        }

        let mut memo: FxHashMap<Id, bool> = FxHashMap::default();
        let mut seeds = FxHashSet::with_capacity_and_hasher(
            self.visible_nodes.len() + self.manual_marked.len() + 1,
            FxBuildHasher,
        );

        for node in &self.visible_nodes {
            seeds.insert(node.id);
        }
        if let Some(root_id) = model.root() {
            seeds.insert(root_id);
        }
        seeds.extend(self.manual_marked.iter().copied());

        for node_id in seeds {
            self.compute_effective_mark(node_id, model, &mut memo);
        }

        self.effective_marked.clear();
        self.effective_marked.extend(
            memo.into_iter()
                .filter_map(|(node_id, marked)| marked.then_some(node_id)),
        );
        self.marks_dirty = false;
    }

    pub fn node_is_marked(&self, node_id: Id) -> bool {
        self.effective_marked.contains(&node_id)
    }

    pub fn prune_removed_marks<T: TreeModel<Id = Id>>(&mut self, model: &T) {
        self.manual_marked
            .retain(|node_id| model.contains(*node_id));
        self.effective_marked
            .retain(|node_id| model.contains(*node_id));
        self.marks_dirty = true;
    }

    pub fn handle_action<T: TreeModel<Id = Id>, C>(
        &mut self,
        model: &T,
        action: TreeAction<C>,
    ) -> TreeEvent<C> {
        self.ensure_visible_nodes(model);
        self.handle_action_inner(model, action)
    }

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
        self.handle_action_inner(model, action)
    }

    #[cfg(feature = "edit")]
    pub fn handle_edit_action<T: TreeEdit<Id = Id>, C>(
        &mut self,
        model: &mut T,
        action: TreeAction<C>,
        clipboard: &mut Option<Id>,
    ) -> bool {
        self.ensure_visible_nodes(model);
        match action {
            TreeAction::ReorderUp => {
                if let Some(node_id) = self.selected_id()
                    && let Some(parent_id) = self.selected_parent_id()
                    && model.move_child_up(parent_id, node_id)
                {
                    self.invalidate();
                    self.ensure_visible_nodes(model);
                    if let Some(idx) = self
                        .visible_nodes
                        .iter()
                        .position(|node| node.id == node_id)
                    {
                        self.list_state.select(Some(idx));
                    }
                    return true;
                }
                false
            }
            TreeAction::ReorderDown => {
                if let Some(node_id) = self.selected_id()
                    && let Some(parent_id) = self.selected_parent_id()
                    && model.move_child_down(parent_id, node_id)
                {
                    self.invalidate();
                    self.ensure_visible_nodes(model);
                    if let Some(idx) = self
                        .visible_nodes
                        .iter()
                        .position(|node| node.id == node_id)
                    {
                        self.list_state.select(Some(idx));
                    }
                    return true;
                }
                false
            }
            TreeAction::DetachNode => {
                if let Some(node_id) = self.selected_id() {
                    if model.is_root(node_id) {
                        return false;
                    }
                    if let Some(parent_id) = self.selected_parent_id() {
                        model.remove_child(parent_id, node_id);
                        self.prune_removed_marks(model);
                        self.invalidate_all();
                        return true;
                    }
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
                    && let Some(parent_id) = self.selected_id()
                {
                    model.add_child(parent_id, node_id);
                    self.invalidate_all();
                    return true;
                }
                false
            }
            TreeAction::Custom(_) => false,
            _ => false,
        }
    }

    #[cfg(feature = "keymap")]
    pub fn handle_key<T: TreeModel<Id = Id>>(&mut self, model: &T, key: KeyEvent) -> TreeEvent<()> {
        self.ensure_visible_nodes(model);
        let Some(action) = self.keymap.resolve(key) else {
            return TreeEvent::Unhandled;
        };
        self.handle_action_inner(model, action)
    }

    #[cfg(feature = "keymap")]
    pub fn handle_key_with<T, C, F>(&mut self, model: &T, key: KeyEvent, custom: F) -> TreeEvent<C>
    where
        T: TreeModel<Id = Id>,
        F: Fn(KeyEvent) -> Option<C>,
    {
        self.ensure_visible_nodes(model);
        let Some(action) = self.keymap.resolve_with(key, custom) else {
            return TreeEvent::Unhandled;
        };
        self.handle_action_inner(model, action)
    }

    #[cfg(feature = "keymap")]
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
        self.handle_action_inner(model, action)
    }

    #[cfg(feature = "keymap")]
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
        self.handle_action_inner(model, action)
    }

    fn handle_action_inner<T: TreeModel<Id = Id>, C>(
        &mut self,
        model: &T,
        action: TreeAction<C>,
    ) -> TreeEvent<C> {
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
                self.select_child_with_descendants(model);
                TreeEvent::Handled
            }
            TreeAction::ToggleRecursive => {
                if let Some(node_id) = self.selected_id()
                    && self.has_children(model, node_id)
                {
                    let parent = self.selected_parent_id();
                    let should_expand = !self.expanded.contains(&(parent, node_id));
                    self.set_expanded_recursive(model, node_id, parent, should_expand);
                    self.dirty = true;
                    return TreeEvent::Handled;
                }
                TreeEvent::Unhandled
            }
            TreeAction::ToggleNode => {
                if let Some(node_id) = self.selected_id()
                    && self.has_children(model, node_id)
                {
                    let parent = self.selected_parent_id();
                    self.toggle(node_id, parent);
                    return TreeEvent::Handled;
                }
                TreeEvent::Unhandled
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
            | TreeAction::PasteNode => TreeEvent::Action(action),
            TreeAction::Custom(_) => TreeEvent::Action(action),
        }
    }

    pub fn toggle(&mut self, node_id: Id, parent: Option<Id>) {
        let key = (parent, node_id);
        if self.expanded.contains(&key) {
            self.expanded.remove(&key);
        } else {
            self.expanded.insert(key);
        }
        self.dirty = true;
    }

    pub fn set_expanded(&mut self, node_id: Id, parent: Option<Id>, expand: bool) {
        let key = (parent, node_id);
        if expand {
            self.expanded.insert(key);
        } else {
            self.expanded.remove(&key);
        }
        self.dirty = true;
    }

    fn has_children<T: TreeModel<Id = Id>>(&self, model: &T, node_id: Id) -> bool {
        !model.children(node_id).is_empty()
    }

    fn update_visible_nodes<T: TreeModel<Id = Id>>(&mut self, model: &T) {
        self.visible_nodes.clear();
        if let Some(root) = model.root() {
            let mut is_tail_stack: SmallVec<[bool; 8]> = SmallVec::new();
            self.build_visible_nodes(model, root, 0, None, &mut is_tail_stack);
        }
        self.dirty = false;
        self.clamp_selection();
    }

    fn find_path_to<T: TreeModel<Id = Id>>(model: &T, target: Id) -> Option<Vec<(Option<Id>, Id)>> {
        let root = model.root()?;
        let mut path: Vec<(Option<Id>, Id)> = Vec::new();
        if Self::dfs_find_path(model, root, None, target, &mut path) {
            Some(path)
        } else {
            None
        }
    }

    fn dfs_find_path<T: TreeModel<Id = Id>>(
        model: &T,
        node: Id,
        parent: Option<Id>,
        target: Id,
        path: &mut Vec<(Option<Id>, Id)>,
    ) -> bool {
        path.push((parent, node));
        if node == target {
            return true;
        }
        for child in model.children(node).iter().copied() {
            if Self::dfs_find_path(model, child, Some(node), target, path) {
                return true;
            }
        }
        path.pop();
        false
    }

    fn build_visible_nodes<T: TreeModel<Id = Id>>(
        &mut self,
        model: &T,
        node_id: Id,
        level: u16,
        parent: Option<Id>,
        is_tail_stack: &mut SmallVec<[bool; 8]>,
    ) {
        self.visible_nodes.push(VisibleNode {
            id: node_id,
            level,
            parent,
            is_tail_stack: is_tail_stack.clone(),
        });

        let is_expanded = self.expanded.contains(&(parent, node_id));
        if !is_expanded {
            return;
        }

        let children = model.children(node_id);
        for (i, child) in children.iter().copied().enumerate() {
            let is_last = i == children.len().saturating_sub(1);
            is_tail_stack.push(is_last);
            self.build_visible_nodes(model, child, level + 1, Some(node_id), is_tail_stack);
            is_tail_stack.pop();
        }
    }

    fn subtree_has_match<T, F>(
        &self,
        model: &T,
        node_id: Id,
        filter: &F,
        memo: &mut FxHashMap<Id, bool>,
    ) -> bool
    where
        T: TreeModel<Id = Id>,
        F: TreeFilter<T>,
    {
        if let Some(&cached) = memo.get(&node_id) {
            return cached;
        }

        let mut matched = filter.is_match(model, node_id);
        if !matched {
            for child in model.children(node_id).iter().copied() {
                if self.subtree_has_match(model, child, filter, memo) {
                    matched = true;
                    break;
                }
            }
        }

        memo.insert(node_id, matched);
        matched
    }

    fn build_visible_nodes_filtered<T, F>(
        &mut self,
        model: &T,
        node_id: Id,
        level: u16,
        parent: Option<Id>,
        is_tail_stack: &mut SmallVec<[bool; 8]>,
        filter: &F,
        config: TreeFilterConfig,
        memo: &mut FxHashMap<Id, bool>,
    ) -> bool
    where
        T: TreeModel<Id = Id>,
        F: TreeFilter<T>,
    {
        let self_match = filter.is_match(model, node_id);
        let children = model.children(node_id);

        let mut visible_children: SmallVec<[Id; 8]> = SmallVec::new();
        for child in children.iter().copied() {
            if self.subtree_has_match(model, child, filter, memo) {
                visible_children.push(child);
            }
        }

        let include_self = self_match || !visible_children.is_empty();
        if !include_self {
            return false;
        }

        self.visible_nodes.push(VisibleNode {
            id: node_id,
            level,
            parent,
            is_tail_stack: is_tail_stack.clone(),
        });

        let expand_children = config.auto_expand || self.expanded.contains(&(parent, node_id));
        if expand_children && !visible_children.is_empty() {
            let last_idx = visible_children.len().saturating_sub(1);
            for (idx, child) in visible_children.iter().copied().enumerate() {
                let is_last = idx == last_idx;
                is_tail_stack.push(is_last);
                self.build_visible_nodes_filtered(
                    model,
                    child,
                    level + 1,
                    Some(node_id),
                    is_tail_stack,
                    filter,
                    config,
                    memo,
                );
                is_tail_stack.pop();
            }
        }

        true
    }

    const fn clamp_selection(&mut self) {
        if self.visible_nodes.is_empty() {
            self.list_state.select(None);
            return;
        }

        if let Some(selected) = self.list_state.selected()
            && selected >= self.visible_nodes.len()
        {
            self.list_state
                .select(Some(self.visible_nodes.len().saturating_sub(1)));
        }
    }

    fn toggle_node_mark(&mut self, node_id: Id) {
        if !self.manual_marked.insert(node_id) {
            self.manual_marked.remove(&node_id);
        }
        self.marks_dirty = true;
    }

    fn compute_effective_mark<T: TreeModel<Id = Id>>(
        &self,
        node_id: Id,
        model: &T,
        memo: &mut FxHashMap<Id, bool>,
    ) -> bool {
        if let Some(&cached) = memo.get(&node_id) {
            return cached;
        }

        let result = if self.manual_marked.contains(&node_id) {
            true
        } else {
            let children = model.children(node_id);
            if children.is_empty() {
                false
            } else {
                children
                    .iter()
                    .copied()
                    .all(|child| self.compute_effective_mark(child, model, memo))
            }
        };

        memo.insert(node_id, result);
        result
    }

    fn select_parent(&mut self) {
        let Some(selected_idx) = self.list_state.selected() else {
            return;
        };

        let Some(parent_id) = self
            .visible_nodes
            .get(selected_idx)
            .and_then(|node| node.parent)
        else {
            return;
        };

        if let Some(parent_idx) = self
            .visible_nodes
            .iter()
            .position(|node| node.id == parent_id)
        {
            self.list_state.select(Some(parent_idx));
        }
    }

    fn select_child_with_descendants<T: TreeModel<Id = Id>>(&mut self, model: &T) {
        let Some(mut selected_idx) = self.list_state.selected() else {
            return;
        };
        let Some(selected_node) = self.visible_nodes.get(selected_idx) else {
            return;
        };
        let node_id = selected_node.id;
        let mut level = selected_node.level;
        let parent_id = selected_node.parent;

        if self.has_children(model, node_id) {
            let expand_key = (parent_id, node_id);
            if self.expanded.insert(expand_key) {
                self.dirty = true;
                self.update_visible_nodes(model);

                if let Some(current_idx) = self
                    .visible_nodes
                    .iter()
                    .position(|node| node.id == node_id)
                {
                    selected_idx = current_idx;
                    if let Some(node) = self.visible_nodes.get(current_idx) {
                        level = node.level;
                    }
                    self.list_state.select(Some(current_idx));
                } else {
                    return;
                }
            }

            for idx in selected_idx + 1..self.visible_nodes.len() {
                let candidate = &self.visible_nodes[idx];
                let candidate_level = candidate.level;
                if candidate_level <= level {
                    break;
                }
                if candidate_level == level + 1 && self.has_children(model, candidate.id) {
                    self.list_state.select(Some(idx));
                    return;
                }
            }
        }

        for idx in selected_idx + 1..self.visible_nodes.len() {
            let candidate = &self.visible_nodes[idx];
            if candidate.level < level {
                break;
            }
            if self.has_children(model, candidate.id) {
                self.list_state.select(Some(idx));
                return;
            }
        }
    }

    fn set_expanded_recursive<T: TreeModel<Id = Id>>(
        &mut self,
        model: &T,
        node_id: Id,
        parent: Option<Id>,
        expand: bool,
    ) {
        let children = model.children(node_id);
        let key = (parent, node_id);
        if expand {
            if !children.is_empty() {
                self.expanded.insert(key);
            }
        } else {
            self.expanded.remove(&key);
        }

        if children.is_empty() {
            return;
        }

        for child in children.iter().copied() {
            self.set_expanded_recursive(model, child, Some(node_id), expand);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestTree {
        children: Vec<Vec<usize>>,
    }

    impl TestTree {
        fn new() -> Self {
            Self {
                children: vec![
                    vec![1, 2], // 0
                    vec![3, 4], // 1
                    vec![],     // 2
                    vec![],     // 3
                    vec![],     // 4
                ],
            }
        }
    }

    impl TreeModel for TestTree {
        type Id = usize;

        fn root(&self) -> Option<Self::Id> {
            Some(0)
        }

        fn children(&self, id: Self::Id) -> &[Self::Id] {
            &self.children[id]
        }

        fn contains(&self, id: Self::Id) -> bool {
            id < self.children.len()
        }
    }

    #[test]
    fn builds_visible_nodes_with_expansion() {
        let tree = TestTree::new();
        let mut state = TreeListViewState::<usize>::new();

        state.set_expanded(0, None, true);
        state.set_expanded(1, Some(0), true);
        state.ensure_visible_nodes(&tree);

        let ids: Vec<_> = state.visible_nodes.iter().map(|n| n.id).collect();
        let levels: Vec<_> = state.visible_nodes.iter().map(|n| n.level).collect();

        assert_eq!(ids, vec![0, 1, 3, 4, 2]);
        assert_eq!(levels, vec![0, 1, 2, 2, 1]);
    }

    #[test]
    fn filtered_view_keeps_matching_path() {
        let tree = TestTree::new();
        let mut state = TreeListViewState::<usize>::new();
        let filter = |_: &TestTree, id: usize| id == 4;

        state.ensure_visible_nodes_filtered(&tree, &filter, TreeFilterConfig::enabled());

        let ids: Vec<_> = state.visible_nodes.iter().map(|n| n.id).collect();
        assert_eq!(ids, vec![0, 1, 4]);
    }

    #[test]
    fn select_prev_clears_selection_when_empty() {
        let mut state = TreeListViewState::<usize>::new();
        state.list_state.select(Some(0));

        state.select_prev();

        assert_eq!(state.list_state.selected(), None);
    }

    #[test]
    fn filtered_view_without_matches_clears_selection() {
        let tree = TestTree::new();
        let mut state = TreeListViewState::<usize>::new();
        let filter = |_: &TestTree, _: usize| false;

        state.list_state.select(Some(0));
        state.ensure_visible_nodes_filtered(&tree, &filter, TreeFilterConfig::enabled());

        assert!(state.visible_nodes.is_empty());
        assert_eq!(state.list_state.selected(), None);
    }
}
