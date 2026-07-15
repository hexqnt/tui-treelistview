use crate::model::{TreeChildren, TreeModel};

type PostorderFrame<'a, Id> = (Id, Option<&'a [Id]>);

pub struct TreeWalkNode<'a, Id> {
    pub parent: Option<Id>,
    pub id: Id,
    pub children: TreeChildren<'a, Id>,
}

pub struct TreeWalk<'a, T: TreeModel> {
    model: &'a T,
    stack: Vec<(Option<T::Id>, T::Id)>,
}

impl<'a, T: TreeModel> TreeWalk<'a, T> {
    pub fn forest(model: &'a T) -> Self {
        let mut stack = Vec::with_capacity(model.size_hint().min(1024));
        stack.extend(model.roots().map(|id| (None, id)));
        stack.reverse();
        Self { model, stack }
    }

    pub fn subtree(model: &'a T, parent: Option<T::Id>, root: T::Id) -> Self {
        Self {
            model,
            stack: vec![(parent, root)],
        }
    }
}

impl<'a, T: TreeModel> Iterator for TreeWalk<'a, T> {
    type Item = TreeWalkNode<'a, T::Id>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let (parent, id) = self.stack.pop()?;
        let children = self.model.children(id);
        self.stack.extend(
            children
                .loaded_slice()
                .iter()
                .rev()
                .copied()
                .map(|child| (Some(id), child)),
        );
        Some(TreeWalkNode {
            parent,
            id,
            children,
        })
    }
}

pub struct TreePostorderNode<'a, Id> {
    pub id: Id,
    pub children: &'a [Id],
}

pub struct TreePostorder<'a, T: TreeModel> {
    model: &'a T,
    stack: Vec<PostorderFrame<'a, T::Id>>,
}

impl<'a, T: TreeModel> TreePostorder<'a, T> {
    pub fn forest(model: &'a T) -> Self {
        let mut stack = Vec::with_capacity(model.size_hint().min(1024));
        stack.extend(model.roots().map(|id| (id, None)));
        stack.reverse();
        Self { model, stack }
    }
}

impl<'a, T: TreeModel> Iterator for TreePostorder<'a, T> {
    type Item = TreePostorderNode<'a, T::Id>;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let (id, children) = self.stack.pop()?;
            if let Some(children) = children {
                return Some(TreePostorderNode { id, children });
            }
            let children = self.model.children(id).loaded_slice();
            self.stack.push((id, Some(children)));
            self.stack
                .extend(children.iter().rev().copied().map(|child| (child, None)));
        }
    }
}
