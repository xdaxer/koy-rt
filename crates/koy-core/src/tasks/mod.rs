//! Karşılığı: `koy/src/Core/Tasks/`.
//!
//! Odak görev listesi: kayıt (`TaskItem`), dosya destekli servis (`TaskService`, `tasks.json`) ve kapsam/saat
//! yardımcıları (`TaskViews`). C# `System.Guid` için küçük bir karşılık `guid` dosyasındadır.

pub mod guid;
pub mod i_task_service;
pub mod task_item;
pub mod task_service;
pub mod task_views;

pub use guid::*;
pub use i_task_service::*;
pub use task_item::*;
pub use task_service::*;
pub use task_views::*;

#[cfg(test)]
mod tests;
