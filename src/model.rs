use std::hash::Hash;

/// Минимальный контракт дерева для виджета.
///
/// Ожидается настоящее дерево (не DAG):
/// - без циклов (обход DFS используется напрямую);
/// - у каждого узла ровно один родитель;
/// - идентификаторы стабильны между кадрами (для выделения/развёртки).
pub trait TreeModel {
    type Id: Copy + Eq + Hash;

    /// Корневой узел дерева (или `None`, если дерево пустое).
    fn root(&self) -> Option<Self::Id>;
    /// Дети узла в детерминированном порядке.
    fn children(&self, id: Self::Id) -> &[Self::Id];
    /// Проверка существования узла в модели.
    fn contains(&self, id: Self::Id) -> bool;
    /// Примерная оценка размера (не обязательно точная).
    fn size_hint(&self) -> usize {
        0
    }
}

/// Фильтр видимости узлов (используется для построения сокращённого списка).
pub trait TreeFilter<T: TreeModel> {
    fn is_match(&self, model: &T, id: T::Id) -> bool;
}

impl<T, F> TreeFilter<T> for F
where
    T: TreeModel,
    F: Fn(&T, T::Id) -> bool,
{
    fn is_match(&self, model: &T, id: T::Id) -> bool {
        self(model, id)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct TreeFilterConfig {
    pub enabled: bool,
    pub auto_expand: bool,
}

impl TreeFilterConfig {
    pub const fn disabled() -> Self {
        Self {
            enabled: false,
            auto_expand: false,
        }
    }

    pub const fn enabled() -> Self {
        Self {
            enabled: true,
            auto_expand: true,
        }
    }
}

impl Default for TreeFilterConfig {
    fn default() -> Self {
        Self::disabled()
    }
}

#[derive(Clone, Copy, Debug)]
pub struct NoFilter;

impl<T: TreeModel> TreeFilter<T> for NoFilter {
    fn is_match(&self, _model: &T, _id: T::Id) -> bool {
        true
    }
}
