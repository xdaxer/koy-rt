//! Karşılığı: `koy/src/Core/Extensions/`.
//!
//! Eklenti bildirimi (manifest) doğrulaması, kurulu eklenti listesi ve `store/` altındaki eklenti mağazası
//! (indeks ayrıştırma, paket denetimi/kurulumu, kurulu kayıt, uygulama planı). Veri paketleri dışında kod çalıştıran
//! hiçbir tür yoktur. `Store/` alt klasörü `store` alt modülüdür; tipleri burada da yeniden dışa açılır.

mod extension_manifest;
mod extension_manifest_validator;
mod installed_extension_list;
pub mod store;
pub(crate) mod unicode_category;

pub use extension_manifest::*;
pub use extension_manifest_validator::*;
pub use installed_extension_list::*;
pub use store::*;

#[cfg(test)]
mod tests;
