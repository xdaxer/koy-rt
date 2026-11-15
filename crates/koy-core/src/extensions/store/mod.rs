//! Karşılığı: `koy/src/Core/Extensions/Store/`.
//!
//! Eklenti mağazası çekirdeği (docs/EXTENSION-STORE.md): indeks modeli ve ayrıştırıcı, paket giriş denetimi (zip slip,
//! sınırlar, tür uzantıları), sürüm karşılaştırma, mağaza listesi durumu, kurulu kayıt (`extensions.json`), paket
//! açma/kurma, uygulama planı ve mağaza istemcisi sözleşmesi.
//!
//! C# `System.IO.Compression.ZipArchive` yerine elle yazılmış küçük bir zip okuyucusu (`zip_reader`) ve RFC 1951 açıcı
//! (`inflate`) kullanılır; yeni crate eklenmez.

mod extension_apply_plan;
mod extension_install_error;
mod extension_kind;
mod extension_package_installer;
mod extension_paths;
mod extension_version;
mod i_extension_store_client;
pub mod inflate;
mod installed_registry;
mod iso_time;
pub(crate) mod json_document;
mod localized_text;
mod package_entry_policy;
mod package_manifest_parser;
mod store_catalog;
pub(crate) mod store_field_rules;
mod store_index_parser;
mod store_models;
mod store_url_policy;
pub mod uri;
pub mod zip_reader;

pub use extension_apply_plan::*;
pub use extension_install_error::*;
pub use extension_kind::*;
pub use extension_package_installer::*;
pub use extension_paths::*;
pub use extension_version::*;
pub use i_extension_store_client::*;
pub use installed_registry::*;
pub use localized_text::*;
pub use package_entry_policy::*;
pub use package_manifest_parser::*;
pub use store_catalog::*;
pub use store_index_parser::*;
pub use store_models::*;
pub use store_url_policy::*;

#[cfg(test)]
mod tests;
