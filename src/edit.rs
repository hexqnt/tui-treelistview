use crate::model::TreeModel;

/// Опциональный edit-интерфейс для базовых операций над деревом.
pub trait TreeEdit: TreeModel {
    fn is_root(&self, id: Self::Id) -> bool;
    fn move_child_up(&mut self, parent: Self::Id, child: Self::Id) -> bool;
    fn move_child_down(&mut self, parent: Self::Id, child: Self::Id) -> bool;
    fn remove_child(&mut self, parent: Self::Id, child: Self::Id);
    fn delete_node(&mut self, id: Self::Id);
    fn add_child(&mut self, parent: Self::Id, child: Self::Id);
}
