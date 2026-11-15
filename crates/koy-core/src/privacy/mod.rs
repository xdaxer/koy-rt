//! Karşılığı: `koy/src/Core/Privacy/`.

pub mod capture_usage_policy;
pub mod i_privacy_monitor_service;

pub use capture_usage_policy::{CaptureUsagePolicy, PrivacyState};
pub use i_privacy_monitor_service::IPrivacyMonitorService;

#[cfg(test)]
mod tests;
