//! Karşılığı: `koy/src/Core/Uninstall/`.

pub mod uninstall_cleaner;
pub mod uninstall_plan;

pub use uninstall_cleaner::{UninstallCleanResult, UninstallCleaner};
pub use uninstall_plan::{UninstallAvailability, UninstallPlan};

#[cfg(test)]
mod tests;
