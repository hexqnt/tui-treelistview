use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::action::TreeAction;

/// Navigation key profiles.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum KeymapProfile {
    /// Arrow keys plus Vim-like alternatives.
    #[default]
    Default,
    /// Vim-only navigation (h/j/k/l).
    Vim,
    /// Arrow-only navigation.
    Arrows,
}

/// Key bindings resolver for tree actions.
#[derive(Clone, Copy, Debug)]
pub struct TreeKeyBindings {
    profile: KeymapProfile,
}

impl Default for TreeKeyBindings {
    fn default() -> Self {
        Self::new()
    }
}

impl TreeKeyBindings {
    /// Creates bindings with the default profile.
    pub const fn new() -> Self {
        Self {
            profile: KeymapProfile::Default,
        }
    }

    /// Creates bindings with a specific profile.
    pub const fn with_profile(profile: KeymapProfile) -> Self {
        Self { profile }
    }

    /// Returns the current keymap profile.
    pub const fn profile(&self) -> KeymapProfile {
        self.profile
    }

    /// Sets the active keymap profile.
    pub const fn set_profile(&mut self, profile: KeymapProfile) {
        self.profile = profile;
    }

    /// Resolves a key event into a tree action.
    pub fn resolve<C>(&self, key: KeyEvent) -> Option<TreeAction<C>> {
        // Shift-modified shortcuts take priority over profile navigation.
        if key.modifiers.contains(KeyModifiers::SHIFT) {
            match key.code {
                KeyCode::Up => return Some(TreeAction::ReorderUp),
                KeyCode::Down => return Some(TreeAction::ReorderDown),
                KeyCode::Delete => return Some(TreeAction::DeleteNode),
                _ => {}
            }
        }

        let nav_action = match self.profile {
            KeymapProfile::Default => self.resolve_default_nav(key),
            KeymapProfile::Vim => self.resolve_vim_nav(key),
            KeymapProfile::Arrows => self.resolve_arrow_nav(key),
        };
        if nav_action.is_some() {
            return nav_action;
        }

        self.resolve_common(key)
    }

    /// Resolves a key event with a custom mapping first.
    pub fn resolve_with<C, F>(&self, key: KeyEvent, custom: F) -> Option<TreeAction<C>>
    where
        F: Fn(KeyEvent) -> Option<C>,
    {
        if let Some(action) = custom(key) {
            return Some(TreeAction::Custom(action));
        }

        self.resolve(key)
    }

    const fn resolve_default_nav<C>(&self, key: KeyEvent) -> Option<TreeAction<C>> {
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => Some(TreeAction::SelectPrev),
            KeyCode::Down | KeyCode::Char('j') => Some(TreeAction::SelectNext),
            KeyCode::Left | KeyCode::Char('h') => Some(TreeAction::SelectParent),
            KeyCode::Right | KeyCode::Char('l') => Some(TreeAction::SelectChild),
            _ => None,
        }
    }

    const fn resolve_vim_nav<C>(&self, key: KeyEvent) -> Option<TreeAction<C>> {
        match key.code {
            KeyCode::Char('k') => Some(TreeAction::SelectPrev),
            KeyCode::Char('j') => Some(TreeAction::SelectNext),
            KeyCode::Char('h') => Some(TreeAction::SelectParent),
            KeyCode::Char('l') => Some(TreeAction::SelectChild),
            _ => None,
        }
    }

    const fn resolve_arrow_nav<C>(&self, key: KeyEvent) -> Option<TreeAction<C>> {
        match key.code {
            KeyCode::Up => Some(TreeAction::SelectPrev),
            KeyCode::Down => Some(TreeAction::SelectNext),
            KeyCode::Left => Some(TreeAction::SelectParent),
            KeyCode::Right => Some(TreeAction::SelectChild),
            _ => None,
        }
    }

    fn resolve_common<C>(&self, key: KeyEvent) -> Option<TreeAction<C>> {
        match key.code {
            KeyCode::Char(' ') => Some(TreeAction::ToggleRecursive),
            KeyCode::Enter => Some(TreeAction::ToggleNode),
            KeyCode::Char('E') => Some(TreeAction::ExpandAll),
            KeyCode::Char('C') => Some(TreeAction::CollapseAll),
            KeyCode::Char('a' | '+') => Some(TreeAction::AddChild),
            KeyCode::Char('e') => Some(TreeAction::EditNode),
            KeyCode::Delete | KeyCode::Char('d') => Some(TreeAction::DetachNode),
            KeyCode::Char('D') => Some(TreeAction::DeleteNode),
            KeyCode::Char('y') => Some(TreeAction::YankNode),
            KeyCode::Char('p') => Some(TreeAction::PasteNode),
            KeyCode::Char('g') => Some(TreeAction::ToggleGuides),
            KeyCode::Char('m' | 'M') => Some(TreeAction::ToggleMark),
            KeyCode::Home => Some(TreeAction::SelectFirst),
            KeyCode::End => Some(TreeAction::SelectLast),
            _ => None,
        }
    }
}
