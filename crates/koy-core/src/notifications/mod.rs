//! Karşılığı: `koy/src/Core/Notifications/`.
//!
//! Windows bildirimlerinin yoklanması, farkı, kuyruğu ve çentik kartına çevrilmesi (saf mantık).

pub mod icon_blank_check;
pub mod notification_diff;
pub mod notification_item;
pub mod notification_poller;
pub mod notification_presentation;
pub mod notification_queue;

pub use icon_blank_check::*;
pub use notification_diff::*;
pub use notification_item::*;
pub use notification_poller::*;
pub use notification_presentation::*;
pub use notification_queue::*;

#[cfg(test)]
mod tests;
