//! Karşılığı: `koy/src/Core/Shelf/`.

mod i_shelf_service;

pub use i_shelf_service::{IShelfService, ShelfItem, ShelfItemKind};

#[cfg(test)]
mod tests;
