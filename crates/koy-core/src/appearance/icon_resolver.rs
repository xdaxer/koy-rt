use super::{AppIconOverrides, IconOverrideMap, IconPack};

/// Simgeyi hangi kaynağın verdiği.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IconSourceKind {
    Default,
    Pack,
    AppOverride,
}

/// Çözümlenen simge: kaynak türü ve (yerleşik simgede `None`) doğrulanmış dosya yolu ya da adı.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct IconChoice {
    pub kind: IconSourceKind,
    pub value: Option<String>,
}

impl IconChoice {
    pub fn new(kind: IconSourceKind, value: Option<String>) -> Self {
        IconChoice { kind, value }
    }
}

/// Hangi simgenin kazandığı: önce uygulamanın kendi değişimi, sonra simge paketi, en son yerleşik simge.
/// Dosya okumaz; paketin o anahtar için sunduğu dosya çağıran tarafından verilir.
pub struct IconResolver;

impl IconResolver {
    /// `app_key`: kısayol adı (dock öğesi); `None` ise uygulama değişimi denenmez. `app_overrides`: uygulama başına
    /// değişimler (`AppIconOverrides`). `pack`: seçili simge paketi. `pack_icon`: paketin bu anahtar için sunduğu dosya
    /// adı (yalnız ad, yol değil); paket yoksa ya da dosya yoksa `None`.
    pub fn resolve(
        app_key: Option<&str>,
        app_overrides: Option<&dyn IconOverrideMap>,
        pack: IconPack,
        pack_icon: Option<&str>,
    ) -> IconChoice {
        if let Some(overridden) = AppIconOverrides::lookup(app_overrides, app_key) {
            return IconChoice::new(IconSourceKind::AppOverride, Some(overridden));
        }

        if pack == IconPack::Folder {
            if let Some(pack_file) = AppIconOverrides::try_validate_pack_file(pack_icon) {
                return IconChoice::new(IconSourceKind::Pack, Some(pack_file));
            }
        }

        IconChoice::new(IconSourceKind::Default, None)
    }
}
