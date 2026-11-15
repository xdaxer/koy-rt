//! Karşılığı: `koy/src/Core/Settings/Profiles/`.
//!
//! Ayar profilleri: dışa/içe aktarma metni (`SettingsProfileCodec`), kayıtlı profil listesi (`ProfileLibrary`) ve dosyası
//! (`ProfileStore`). `imported_profile.rs` C#'taki `ImportedProfile.cs`, `ProfileSection.cs` ve `SavedProfile.cs` kayıtlarını taşır.

mod imported_profile;
mod profile_library;
mod profile_store;
mod settings_profile_codec;

pub use imported_profile::*;
pub use profile_library::*;
pub use profile_store::*;
pub use settings_profile_codec::*;
