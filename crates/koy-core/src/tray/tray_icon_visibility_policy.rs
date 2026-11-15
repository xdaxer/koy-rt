use std::collections::HashMap;

use super::tray_icon_visibility::TrayIconVisibility;
use crate::text;

/// Tepsi simgesi görünürlüğü: uygulama başına kayıt varsa o, yoksa yeni simge varsayılanı. Anahtar küçük harfli exe adıdır.
pub struct TrayIconVisibilityPolicy;

impl TrayIconVisibilityPolicy {
    /// Yeni tepsi simgeleri varsayılan olarak taşma menüsüne girer.
    pub const DEFAULT_NEW_ICON: TrayIconVisibility = TrayIconVisibility::Overflow;

    /// Exe adından kayıt anahtarı: boşluk kırpılır, küçük harfe çevrilir (kültürden bağımsız).
    pub fn app_key(exe_name: Option<&str>) -> String {
        text::to_lower_invariant(text::trim(exe_name.unwrap_or("")))
    }

    /// Simgenin etkin görünürlüğü. `per_app` anahtarları `app_key` biçimindedir (küçük harf).
    pub fn resolve(
        exe_name: Option<&str>,
        per_app: &HashMap<String, TrayIconVisibility>,
        new_icon_default: TrayIconVisibility,
    ) -> TrayIconVisibility {
        let key = Self::app_key(exe_name);
        if !key.is_empty()
            && let Some(visibility) = per_app.get(&key)
        {
            return *visibility;
        }

        new_icon_default
    }

    /// Ayarlar listesindeki uygulamalar: bu oturumda tepside görülen exe adları ile kaydı olanların birleşimi,
    /// anahtar biçiminde (küçük harf), tekrarsız ve sıralı. Boş adlar atlanır.
    pub fn settings_apps<'a>(
        seen_exe_names: impl IntoIterator<Item = Option<&'a str>>,
        saved_keys: impl IntoIterator<Item = &'a str>,
    ) -> Vec<String> {
        let mut result: Vec<String> = Vec::new();
        let keys = seen_exe_names.into_iter().map(Self::app_key).chain(saved_keys.into_iter().map(|k| Self::app_key(Some(k))));
        for k in keys {
            // `Distinct(StringComparer.Ordinal)`: ilk görülen kalır.
            if !k.is_empty() && !result.contains(&k) {
                result.push(k);
            }
        }
        // `Order(StringComparer.Ordinal)`: UTF-16 kod birimi sırası (kararlı).
        result.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
        result
    }
}
