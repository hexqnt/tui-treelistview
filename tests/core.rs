use std::cmp::Ordering;

use smallvec::smallvec;
use tui_treelistview::{
    ColumnDef, ColumnWidth, IndexedTree, IndexedTreeError, ProjectedNode, TreeAction,
    TreeChangeSet, TreeChildren, TreeColumnSet, TreeEditCommand, TreeEditor, TreeEvent,
    TreeExpansionState, TreeFilter, TreeFilterConfig, TreeIntent, TreeListViewSnapshot,
    TreeListViewState, TreeMarkState, TreeModel, TreeModelRef, TreeQuery, TreeRevision,
    TreeRootVisibility, TreeSelectionFallback, TreeSelectionUpdate, TreeSort, TreeViewAction,
};

#[derive(Clone, Debug)]
enum Children {
    Leaf,
    Unloaded,
    Loading,
    Loaded(Vec<usize>),
}

#[derive(Clone, Debug)]
struct TestTree {
    roots: Vec<usize>,
    children: Vec<Children>,
    revision: TreeRevision,
}

impl TestTree {
    fn forest() -> Self {
        Self {
            roots: vec![0, 4],
            children: vec![
                Children::Loaded(vec![1, 2]),
                Children::Loaded(vec![3]),
                Children::Leaf,
                Children::Leaf,
                Children::Loaded(vec![5]),
                Children::Leaf,
            ],
            revision: TreeRevision::INITIAL,
        }
    }

    fn remove(&mut self, parent: usize, node: usize) {
        if let Children::Loaded(children) = &mut self.children[parent] {
            children.retain(|child| *child != node);
        }
        self.revision.advance();
    }
}

impl TreeModel for TestTree {
    type Id = usize;

    fn roots(&self) -> impl Iterator<Item = Self::Id> + '_ {
        self.roots.iter().copied()
    }

    fn children(&self, id: Self::Id) -> TreeChildren<'_, Self::Id> {
        match &self.children[id] {
            Children::Leaf => TreeChildren::Leaf,
            Children::Unloaded => TreeChildren::Unloaded,
            Children::Loading => TreeChildren::Loading,
            Children::Loaded(children) => TreeChildren::Loaded(children),
        }
    }

    fn revision(&self) -> TreeRevision {
        self.revision
    }

    fn size_hint(&self) -> usize {
        self.children.len()
    }
}

#[derive(Clone, Copy)]
struct ExactMatch(usize);

impl TreeFilter<TestTree> for ExactMatch {
    fn is_match(&self, _: &TestTree, id: usize) -> bool {
        id == self.0
    }
}

#[derive(Clone, Copy)]
struct NumericOrder {
    descending: bool,
}

impl TreeSort<TestTree> for NumericOrder {
    fn compare(&self, _: &TestTree, left: usize, right: usize) -> Ordering {
        if self.descending {
            right.cmp(&left)
        } else {
            left.cmp(&right)
        }
    }
}

#[derive(Clone, Debug)]
struct EditableTree(TestTree);

impl TreeModel for EditableTree {
    type Id = usize;

    fn roots(&self) -> impl Iterator<Item = Self::Id> + '_ {
        self.0.roots.iter().copied()
    }

    fn children(&self, id: Self::Id) -> TreeChildren<'_, Self::Id> {
        self.0.children(id)
    }

    fn revision(&self) -> TreeRevision {
        self.0.revision
    }
}

impl TreeEditor for EditableTree {
    type Error = &'static str;

    fn apply(
        &mut self,
        command: TreeEditCommand<Self::Id>,
    ) -> Result<TreeChangeSet<Self::Id>, Self::Error> {
        match command {
            TreeEditCommand::Delete { nodes } => {
                let node = *nodes.first().ok_or("empty delete")?;
                self.0.remove(0, node);
                Ok(TreeChangeSet {
                    removed: smallvec![node],
                    selection: TreeSelectionUpdate::Select(0),
                    ..TreeChangeSet::default()
                })
            }
            TreeEditCommand::CreateChild { parent } => {
                let child = self.0.children.len();
                self.0.children.push(Children::Leaf);
                match &mut self.0.children[parent] {
                    Children::Leaf => {
                        self.0.children[parent] = Children::Loaded(vec![child]);
                    }
                    Children::Loaded(children) => children.push(child),
                    Children::Unloaded | Children::Loading => {
                        return Err("cannot create under an unloaded branch");
                    }
                }
                self.0.revision.advance();
                Ok(TreeChangeSet {
                    inserted: smallvec![child],
                    selection: TreeSelectionUpdate::Select(child),
                    ..TreeChangeSet::default()
                })
            }
            TreeEditCommand::Rename { .. }
            | TreeEditCommand::Move { .. }
            | TreeEditCommand::Detach { .. } => Err("unsupported test command"),
        }
    }
}

fn columns() -> TreeColumnSet<'static, TestTree> {
    TreeColumnSet::new([ColumnDef::tree("Name", ColumnWidth::fixed(12))]).expect("one tree column")
}

fn descending(_: &TestTree, left: usize, right: usize) -> Ordering {
    right.cmp(&left)
}

const fn matches_two_or_three(_: &TestTree, id: usize) -> bool {
    matches!(id, 2 | 3)
}

const fn matches_five(_: &TestTree, id: usize) -> bool {
    id == 5
}

#[test]
fn projection_supports_forests_and_hidden_roots() {
    let model = TestTree::forest();
    let query = TreeQuery::new();
    let mut state = TreeListViewState::new();
    assert!(state.expand_all(&model));
    assert!(state.ensure_projection(&model, &query));
    assert_eq!(state.visible_ids().collect::<Vec<_>>(), [0, 1, 3, 2, 4, 5]);
    assert_eq!(
        state.projection().get_by_id(3).map(ProjectedNode::level),
        Some(2)
    );

    let hidden = TreeQuery::new().with_root_visibility(TreeRootVisibility::Hidden);
    assert!(state.ensure_projection(&model, &hidden));
    assert_eq!(state.visible_ids().collect::<Vec<_>>(), [1, 3, 2, 5]);
    let first = state.projection().nodes()[0];
    assert_eq!(first.parent(), Some(0));
    assert_eq!(first.level(), 0);
}

#[test]
fn filtering_keeps_paths_and_can_force_expansion() {
    let model = TestTree::forest();
    let query = TreeQuery::new().with_filter(
        matches_two_or_three,
        TreeFilterConfig::enabled(),
        TreeRevision::INITIAL,
    );
    let mut state = TreeListViewState::new();
    assert!(state.ensure_projection(&model, &query));
    assert_eq!(state.visible_ids().collect::<Vec<_>>(), [0, 1, 3, 2]);
    assert_eq!(
        state.effective_expansion(0),
        Some(TreeExpansionState::ForcedByFilter)
    );
    assert_eq!(
        state
            .projection()
            .get_by_id(3)
            .map(ProjectedNode::match_state),
        Some(tui_treelistview::TreeMatchState::Direct)
    );
    assert_eq!(
        state
            .projection()
            .get_by_id(1)
            .map(ProjectedNode::match_state),
        Some(tui_treelistview::TreeMatchState::Ancestor)
    );

    let manual = TreeQuery::new().with_filter(
        matches_two_or_three,
        TreeFilterConfig::enabled_manual_expand(),
        TreeRevision::INITIAL,
    );
    let mut collapsed = TreeListViewState::new();
    assert!(collapsed.ensure_projection(&model, &manual));
    assert_eq!(collapsed.visible_ids().collect::<Vec<_>>(), [0]);
}

#[test]
fn filtering_can_be_disabled_without_replacing_its_policy() {
    let model = TestTree::forest();
    let mut query = TreeQuery::new().with_filter(
        matches_two_or_three,
        TreeFilterConfig::enabled(),
        TreeRevision::INITIAL,
    );
    let mut state = TreeListViewState::new();
    assert!(state.ensure_projection(&model, &query));
    assert_eq!(state.visible_ids().collect::<Vec<_>>(), [0, 1, 3, 2]);

    assert!(query.set_filter_config(TreeFilterConfig::Disabled));
    assert!(!query.set_filter_config(TreeFilterConfig::Disabled));
    assert_eq!(query.filter_config(), TreeFilterConfig::Disabled);
    assert!(state.ensure_projection(&model, &query));
    assert_eq!(state.visible_ids().collect::<Vec<_>>(), [0, 4]);
}

#[test]
fn replacing_a_filter_policy_rebuilds_even_at_the_same_data_revision() {
    let model = TestTree::forest();
    let first = TreeQuery::new().with_filter(
        matches_two_or_three,
        TreeFilterConfig::enabled(),
        TreeRevision::INITIAL,
    );
    let second = TreeQuery::new().with_filter(
        matches_five,
        TreeFilterConfig::enabled(),
        TreeRevision::INITIAL,
    );
    let mut state = TreeListViewState::new();
    assert!(state.ensure_projection(&model, &first));
    assert!(state.ensure_projection(&model, &second));
    assert_eq!(state.visible_ids().collect::<Vec<_>>(), [4, 5]);
}

#[test]
fn replacing_the_same_policy_type_invalidates_its_projection_stamp() {
    let model = TestTree::forest();
    let first = TreeQuery::new().with_filter(
        ExactMatch(3),
        TreeFilterConfig::enabled(),
        TreeRevision::INITIAL,
    );
    let second = TreeQuery::new().with_filter(
        ExactMatch(5),
        TreeFilterConfig::enabled(),
        TreeRevision::INITIAL,
    );
    let mut state = TreeListViewState::new();
    assert!(state.ensure_projection(&model, &first));
    assert_eq!(state.visible_ids().collect::<Vec<_>>(), [0, 1, 3]);
    assert!(state.ensure_projection(&model, &second));
    assert_eq!(state.visible_ids().collect::<Vec<_>>(), [4, 5]);

    assert!(state.expand_all(&model));
    let ascending =
        TreeQuery::new().with_sort(NumericOrder { descending: false }, TreeRevision::INITIAL);
    let descending =
        TreeQuery::new().with_sort(NumericOrder { descending: true }, TreeRevision::INITIAL);
    assert!(state.ensure_projection(&model, &ascending));
    assert_eq!(state.visible_ids().collect::<Vec<_>>(), [0, 1, 3, 2, 4, 5]);
    assert!(state.ensure_projection(&model, &descending));
    assert_eq!(state.visible_ids().collect::<Vec<_>>(), [4, 5, 0, 2, 1, 3]);
}

#[test]
fn selection_uses_stable_ids_across_sorting_and_model_changes() {
    let mut model = TestTree::forest();
    let query = TreeQuery::new();
    let mut state = TreeListViewState::new();
    assert!(state.expand_all(&model));
    assert!(state.select_by_id(&model, &query, 3));
    assert_eq!(state.selected_id(), Some(3));

    let sorted = TreeQuery::new().with_sort(descending, TreeRevision::INITIAL);
    assert!(state.ensure_projection(&model, &sorted));
    assert_eq!(state.selected_id(), Some(3));
    assert_eq!(state.visible_ids().collect::<Vec<_>>(), [4, 5, 0, 2, 1, 3]);

    model.remove(1, 3);
    assert!(state.ensure_projection(&model, &sorted));
    assert_eq!(state.selected_id(), Some(1));

    let clear = TreeQuery::new().with_selection_fallback(TreeSelectionFallback::Clear);
    assert!(state.select_by_id(&model, &clear, 2));
    model.remove(0, 2);
    assert!(state.ensure_projection(&model, &clear));
    assert_eq!(state.selected_id(), None);
}

#[test]
fn lazy_branches_emit_load_intents_and_loading_is_inert() {
    let mut model = TestTree {
        roots: vec![0],
        children: vec![Children::Unloaded],
        revision: TreeRevision::INITIAL,
    };
    let query = TreeQuery::new();
    let columns = columns();
    let mut state = TreeListViewState::new();
    assert!(state.select_by_id(&model, &query, 0));
    assert_eq!(
        state.handle_action(
            &model,
            &query,
            &columns,
            TreeAction::<()>::View(TreeViewAction::Expand),
        ),
        TreeEvent::Intent(TreeIntent::LoadChildren(0))
    );

    model.children[0] = Children::Loading;
    model.revision.advance();
    assert_eq!(
        state.handle_action(
            &model,
            &query,
            &columns,
            TreeAction::<()>::View(TreeViewAction::Expand),
        ),
        TreeEvent::Unchanged
    );
}

#[test]
fn right_and_left_follow_standard_tree_navigation() {
    let model = TestTree::forest();
    let query = TreeQuery::new();
    let columns = columns();
    let mut state = TreeListViewState::new();
    assert!(state.select_by_id(&model, &query, 0));

    assert_eq!(
        state.handle_action(
            &model,
            &query,
            &columns,
            TreeAction::<()>::View(TreeViewAction::ExpandOrSelectFirstChild),
        ),
        TreeEvent::Changed
    );
    assert_eq!(state.selected_id(), Some(0));
    assert_eq!(
        state.handle_action(
            &model,
            &query,
            &columns,
            TreeAction::<()>::View(TreeViewAction::ExpandOrSelectFirstChild),
        ),
        TreeEvent::Changed
    );
    assert_eq!(state.selected_id(), Some(1));
    assert_eq!(
        state.handle_action(
            &model,
            &query,
            &columns,
            TreeAction::<()>::View(TreeViewAction::CollapseOrSelectParent),
        ),
        TreeEvent::Changed
    );
    assert_eq!(state.selected_id(), Some(0));
}

#[test]
fn marks_are_aggregated_without_recursion() {
    let model = TestTree::forest();
    let mut state = TreeListViewState::new();
    assert!(state.set_marked(1, true));
    state.ensure_mark_states(&model);
    assert_eq!(state.mark_state(0), TreeMarkState::Partial);
    assert_eq!(state.mark_state(1), TreeMarkState::Marked);

    assert!(state.set_marked(2, true));
    state.ensure_mark_states(&model);
    assert_eq!(state.mark_state(0), TreeMarkState::Marked);
}

#[test]
fn projection_handles_a_very_deep_tree_iteratively() {
    const DEPTH: usize = 20_000;
    let mut children = Vec::with_capacity(DEPTH);
    for id in 0..DEPTH {
        if id + 1 == DEPTH {
            children.push(Children::Leaf);
        } else {
            children.push(Children::Loaded(vec![id + 1]));
        }
    }
    let model = TestTree {
        roots: vec![0],
        children,
        revision: TreeRevision::INITIAL,
    };
    let query = TreeQuery::new();
    let mut state = TreeListViewState::with_capacity(DEPTH);
    assert!(state.expand_all(&model));
    assert!(state.ensure_projection(&model, &query));
    assert_eq!(state.visible_len(), DEPTH);
    state.ensure_mark_states(&model);
    assert_eq!(state.mark_state(0), TreeMarkState::Unmarked);
}

#[test]
fn adapters_parse_invariants_once() {
    let children = vec![vec![1], vec![], vec![]];
    assert!(matches!(
        IndexedTree::new([0], &children, TreeRevision::INITIAL),
        Err(IndexedTreeError::MissingRoot(2))
    ));

    let roots = [0];
    let model =
        TreeModelRef::new(&roots, |_| TreeChildren::Leaf, TreeRevision::new(7)).with_size_hint(1);
    assert_eq!(model.roots().collect::<Vec<_>>(), [0]);
    assert_eq!(model.revision(), TreeRevision::new(7));
}

#[test]
fn edit_changes_reconcile_selection_marks_and_expansion() {
    let mut model = EditableTree(TestTree::forest());
    let query = TreeQuery::new();
    let mut state = TreeListViewState::new();
    assert!(state.select_by_id(&model, &query, 2));
    assert!(state.set_marked(2, true));
    assert!(state.set_expanded(2, Some(0), true));

    let changes = state
        .apply_edit(
            &mut model,
            &query,
            TreeEditCommand::Delete {
                nodes: smallvec![2],
            },
        )
        .expect("valid delete");
    assert_eq!(changes.removed.as_slice(), &[2]);
    assert_eq!(state.selected_id(), Some(0));
    assert!(!state.is_manually_marked(2));
    assert!(!state.expanded_paths().any(|(_, id)| id == 2));
}

#[test]
fn editing_expands_the_path_to_an_explicitly_selected_result() {
    let mut model = EditableTree(TestTree::forest());
    let query = TreeQuery::new();
    let mut state = TreeListViewState::new();
    assert!(state.select_by_id(&model, &query, 2));

    let changes = state
        .apply_edit(
            &mut model,
            &query,
            TreeEditCommand::CreateChild { parent: 2 },
        )
        .expect("valid insertion");
    let child = changes.inserted[0];
    assert_eq!(state.selected_id(), Some(child));
    assert!(state.visible_contains(child));
    assert!(state.node_is_expanded(2, Some(0)));
}

#[test]
fn snapshots_preserve_ids_and_both_scroll_offsets() {
    let snapshot = TreeListViewSnapshot {
        expanded: vec![(None, 0)],
        manual_marked: vec![2],
        selected: Some(2),
        selected_column: Some(1),
        offset: 9,
        horizontal_offset: 13,
        draw_lines: false,
    };
    let state = TreeListViewState::from_snapshot(snapshot.clone());
    assert_eq!(state.snapshot(), snapshot);

    #[cfg(feature = "serde")]
    {
        let json = serde_json::to_string(&snapshot).expect("serialize snapshot");
        let decoded: TreeListViewSnapshot<usize> =
            serde_json::from_str(&json).expect("deserialize snapshot");
        assert_eq!(decoded, snapshot);
    }
}
