//! Karşılığı: `koy/src/Core/Network/`.

mod i_network_service;
mod network_summary;

pub use i_network_service::*;
pub use network_summary::*;

#[cfg(test)]
mod tests;
