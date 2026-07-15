use ratatui::layout::Rect;
use ratatui::prelude::Buffer;
use ratatui::widgets::StatefulWidget;
use smallvec::smallvec;

use tui_treelistview::{
    ColumnDef, ColumnWidth, TreeChangeSet, TreeChildren, TreeColumnSet, TreeEditCommand,
    TreeEditor, TreeInsertPosition, TreeLabelPrefix, TreeLabelProvider, TreeListView,
    TreeListViewState, TreeListViewStyle, TreeModel, TreeQuery, TreeRevision, TreeSelectionUpdate,
};

struct Model {
    root: usize,
    children: Vec<Vec<usize>>,
    parents: Vec<Option<usize>>,
    names: Vec<String>,
    revision: TreeRevision,
}

impl TreeModel for Model {
    type Id = usize;

    fn roots(&self) -> impl Iterator<Item = Self::Id> + '_ {
        std::iter::once(self.root)
    }

    fn children(&self, id: Self::Id) -> TreeChildren<'_, Self::Id> {
        TreeChildren::loaded(&self.children[id])
    }

    fn revision(&self) -> TreeRevision {
        self.revision
    }
}

impl TreeEditor for Model {
    type Error = &'static str;

    fn apply(
        &mut self,
        command: TreeEditCommand<Self::Id>,
    ) -> Result<TreeChangeSet<Self::Id>, Self::Error> {
        let TreeEditCommand::Move {
            nodes,
            parent,
            position,
        } = command
        else {
            return Err("this minimal model only implements move");
        };
        let Some(&node) = nodes.first() else {
            return Err("move set is empty");
        };
        let old_parent = self.parents[node].ok_or("cannot move root")?;
        self.children[old_parent].retain(|child| *child != node);
        let siblings = &mut self.children[parent];
        let index = position
            .index_in(siblings)
            .ok_or("insertion anchor is missing")?;
        siblings.insert(index, node);
        self.parents[node] = Some(parent);
        self.revision.advance();
        Ok(TreeChangeSet {
            moved: smallvec![node],
            selection: TreeSelectionUpdate::Select(node),
            ..TreeChangeSet::default()
        })
    }
}

struct Label;

impl TreeLabelProvider<Model> for Label {
    fn label_parts<'a>(&'a self, model: &'a Model, id: usize) -> TreeLabelPrefix<'a> {
        TreeLabelPrefix::borrowed(&model.names[id])
    }
}

fn main() {
    let mut model = Model {
        root: 0,
        children: vec![vec![1, 2, 3], vec![], vec![], vec![]],
        parents: vec![None, Some(0), Some(0), Some(0)],
        names: vec!["root".into(), "alpha".into(), "beta".into(), "gamma".into()],
        revision: TreeRevision::INITIAL,
    };
    let query = TreeQuery::new();
    let label = Label;
    let columns = TreeColumnSet::new([ColumnDef::tree(
        "Name",
        ColumnWidth::flexible(8, 24).expect("valid static column width"),
    )])
    .expect("one tree column");
    let mut state = TreeListViewState::new();
    state.set_expanded(0, None, true);
    state.select_by_id(&model, &query, 2);
    state
        .apply_edit(
            &mut model,
            &query,
            TreeEditCommand::Move {
                nodes: smallvec![2],
                parent: 0,
                position: TreeInsertPosition::Before(1),
            },
        )
        .expect("valid move");

    let area = Rect::new(0, 0, 40, 8);
    let mut buffer = Buffer::empty(area);
    TreeListView::new(
        &model,
        &query,
        &label,
        &columns,
        TreeListViewStyle::default(),
    )
    .render(area, &mut buffer, &mut state);
}
