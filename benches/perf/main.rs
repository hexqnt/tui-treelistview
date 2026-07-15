use criterion::{criterion_group, criterion_main};

mod adapters;
mod fixture;
mod projection;
mod render;
mod state;

criterion_group!(
    benches,
    projection::rebuild,
    projection::cache,
    projection::id_width,
    projection::filter,
    projection::sort,
    state::marks_cache,
    state::marks_rebuild,
    state::interaction,
    adapters::indexed_tree,
    render::balanced,
    render::deep,
    render::horizontal,
    render::end_to_end,
);
criterion_main!(benches);
