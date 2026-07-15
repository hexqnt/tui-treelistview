use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use crate::action::{TreeAction, TreeEditAction, TreeViewAction};

/// A key profile for vertical and hierarchical navigation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KeymapProfile {
    #[default]
    Default,
    Vim,
    Arrows,
}

/// A stateless key resolver stored with view state for convenient profile switching.
#[derive(Clone, Copy, Debug)]
pub struct TreeKeyBindings {
    profile: KeymapProfile,
}

impl TreeKeyBindings {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            profile: KeymapProfile::Default,
        }
    }

    #[must_use]
    pub const fn with_profile(profile: KeymapProfile) -> Self {
        Self { profile }
    }

    #[must_use]
    pub const fn profile(&self) -> KeymapProfile {
        self.profile
    }

    pub const fn set_profile(&mut self, profile: KeymapProfile) {
        self.profile = profile;
    }

    /// Resolves only press/repeat events and handles modifiers explicitly.
    #[must_use]
    pub fn resolve<C>(&self, key: KeyEvent) -> Option<TreeAction<C>> {
        if key.kind == KeyEventKind::Release {
            return None;
        }

        match (key.code, key.modifiers) {
            (KeyCode::Up, KeyModifiers::SHIFT) => {
                return Some(TreeEditAction::ReorderUp.into());
            }
            (KeyCode::Down, KeyModifiers::SHIFT) => {
                return Some(TreeEditAction::ReorderDown.into());
            }
            (KeyCode::Delete, KeyModifiers::SHIFT) => {
                return Some(TreeEditAction::Delete.into());
            }
            (KeyCode::Left, KeyModifiers::CONTROL) => {
                return Some(TreeViewAction::ScrollLeft.into());
            }
            (KeyCode::Right, KeyModifiers::CONTROL) => {
                return Some(TreeViewAction::ScrollRight.into());
            }
            _ => {}
        }

        if key.modifiers.is_empty()
            && let Some(action) = Self::navigation(self.profile, key.code)
        {
            return Some(action.into());
        }

        Self::common(key)
    }

    #[must_use]
    pub fn resolve_with<C, F>(&self, key: KeyEvent, custom: F) -> Option<TreeAction<C>>
    where
        F: Fn(KeyEvent) -> Option<C>,
    {
        custom(key)
            .map(TreeAction::Custom)
            .or_else(|| self.resolve(key))
    }

    const fn navigation(profile: KeymapProfile, code: KeyCode) -> Option<TreeViewAction> {
        match (profile, code) {
            (KeymapProfile::Default, KeyCode::Up | KeyCode::Char('k'))
            | (KeymapProfile::Vim, KeyCode::Char('k'))
            | (KeymapProfile::Arrows, KeyCode::Up) => Some(TreeViewAction::SelectPrev),
            (KeymapProfile::Default, KeyCode::Down | KeyCode::Char('j'))
            | (KeymapProfile::Vim, KeyCode::Char('j'))
            | (KeymapProfile::Arrows, KeyCode::Down) => Some(TreeViewAction::SelectNext),
            (KeymapProfile::Default, KeyCode::Left | KeyCode::Char('h'))
            | (KeymapProfile::Vim, KeyCode::Char('h'))
            | (KeymapProfile::Arrows, KeyCode::Left) => {
                Some(TreeViewAction::CollapseOrSelectParent)
            }
            (KeymapProfile::Default, KeyCode::Right | KeyCode::Char('l'))
            | (KeymapProfile::Vim, KeyCode::Char('l'))
            | (KeymapProfile::Arrows, KeyCode::Right) => {
                Some(TreeViewAction::ExpandOrSelectFirstChild)
            }
            _ => None,
        }
    }

    const fn common<C>(key: KeyEvent) -> Option<TreeAction<C>> {
        match (key.code, key.modifiers) {
            (KeyCode::Char(' '), KeyModifiers::NONE) => {
                Some(TreeAction::View(TreeViewAction::ToggleRecursive))
            }
            (KeyCode::Enter, KeyModifiers::NONE) => {
                Some(TreeAction::View(TreeViewAction::ToggleNode))
            }
            (KeyCode::Char('E'), KeyModifiers::SHIFT | KeyModifiers::NONE) => {
                Some(TreeAction::View(TreeViewAction::ExpandAll))
            }
            (KeyCode::Char('C'), KeyModifiers::SHIFT | KeyModifiers::NONE) => {
                Some(TreeAction::View(TreeViewAction::CollapseAll))
            }
            (KeyCode::Char('a' | '+'), KeyModifiers::NONE) => {
                Some(TreeAction::Edit(TreeEditAction::AddChild))
            }
            (KeyCode::Char('e'), KeyModifiers::NONE) => {
                Some(TreeAction::Edit(TreeEditAction::Rename))
            }
            (KeyCode::Delete | KeyCode::Char('d'), KeyModifiers::NONE) => {
                Some(TreeAction::Edit(TreeEditAction::Detach))
            }
            (KeyCode::Char('D'), KeyModifiers::SHIFT | KeyModifiers::NONE) => {
                Some(TreeAction::Edit(TreeEditAction::Delete))
            }
            (KeyCode::Char('y'), KeyModifiers::NONE) => {
                Some(TreeAction::Edit(TreeEditAction::Yank))
            }
            (KeyCode::Char('p'), KeyModifiers::NONE) => {
                Some(TreeAction::Edit(TreeEditAction::Paste))
            }
            (KeyCode::Char('g'), KeyModifiers::NONE) => {
                Some(TreeAction::View(TreeViewAction::ToggleGuides))
            }
            (KeyCode::Char('m' | 'M'), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                Some(TreeAction::View(TreeViewAction::ToggleMark))
            }
            (KeyCode::Home, KeyModifiers::NONE) => {
                Some(TreeAction::View(TreeViewAction::SelectFirst))
            }
            (KeyCode::End, KeyModifiers::NONE) => {
                Some(TreeAction::View(TreeViewAction::SelectLast))
            }
            (KeyCode::Tab, KeyModifiers::NONE) => {
                Some(TreeAction::View(TreeViewAction::SelectColumnRight))
            }
            (KeyCode::BackTab, KeyModifiers::SHIFT | KeyModifiers::NONE) => {
                Some(TreeAction::View(TreeViewAction::SelectColumnLeft))
            }
            (KeyCode::PageUp, KeyModifiers::NONE) => {
                Some(TreeAction::View(TreeViewAction::ScrollViewUp))
            }
            (KeyCode::PageDown, KeyModifiers::NONE) => {
                Some(TreeAction::View(TreeViewAction::ScrollViewDown))
            }
            _ => None,
        }
    }
}

impl Default for TreeKeyBindings {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ignores_release_and_unrelated_modifiers() {
        let bindings = TreeKeyBindings::new();
        let release =
            KeyEvent::new_with_kind(KeyCode::Down, KeyModifiers::NONE, KeyEventKind::Release);
        assert_eq!(bindings.resolve::<()>(release), None);

        let control_e = KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL);
        assert_eq!(bindings.resolve::<()>(control_e), None);
    }

    #[test]
    fn resolves_standard_tree_navigation() {
        let bindings = TreeKeyBindings::new();
        let right = KeyEvent::new(KeyCode::Right, KeyModifiers::NONE);
        assert_eq!(
            bindings.resolve::<()>(right),
            Some(TreeViewAction::ExpandOrSelectFirstChild.into())
        );
    }

    #[test]
    fn navigation_profiles_share_actions_but_restrict_keys() {
        let up = KeyEvent::new(KeyCode::Up, KeyModifiers::NONE);
        let k = KeyEvent::new(KeyCode::Char('k'), KeyModifiers::NONE);
        let vim = TreeKeyBindings::with_profile(KeymapProfile::Vim);
        let arrows = TreeKeyBindings::with_profile(KeymapProfile::Arrows);

        assert_eq!(vim.resolve::<()>(up), None);
        assert_eq!(
            vim.resolve::<()>(k),
            Some(TreeViewAction::SelectPrev.into())
        );
        assert_eq!(arrows.resolve::<()>(k), None);
        assert_eq!(
            arrows.resolve::<()>(up),
            Some(TreeViewAction::SelectPrev.into())
        );
    }
}
