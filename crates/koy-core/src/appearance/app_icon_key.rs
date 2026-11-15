use super::AppIconOverrides;
use crate::text;

/// Uygulama başına simge değişiminin anahtarı: varsa AppUserModelId (paketli uygulamalarda kısayol değişse de aynı kalır),
/// yoksa dock öğesinin kısayol/yürütülebilir dosya adı. Geçerli anahtar çıkmazsa `None`.
pub struct AppIconKey;

impl AppIconKey {
    /// `app_user_model_id`: dock öğesinin AUMID'i; yoksa `None`. `item_id`: dock öğesi kimliği (kısayol dosya adı ya da yolu).
    pub fn r#for(app_user_model_id: Option<&str>, item_id: Option<&str>) -> Option<String> {
        Self::all(app_user_model_id, item_id).into_iter().next()
    }

    /// Öğenin bütün geçerli anahtarları, tercih sırasıyla (AUMID, kısayol adı). Sabitli bir uygulamanın AUMID'i yalnız
    /// çalışırken bilinebilir; bu yüzden kaydederken ve silerken hepsi, okurken ilk eşleşen kullanılır ve değişim
    /// uygulama kapalıyken de kaybolmaz.
    pub fn all(app_user_model_id: Option<&str>, item_id: Option<&str>) -> Vec<String> {
        let mut keys: Vec<String> = Vec::with_capacity(2);
        if AppIconOverrides::is_valid_key(app_user_model_id) {
            keys.push(app_user_model_id.unwrap_or_default().trim().to_string());
        }

        if let Some(name) = shortcut_name(item_id) {
            if !keys.iter().any(|k| text::eq_ignore_case(k, &name)) {
                keys.push(name);
            }
        }

        keys
    }
}

fn shortcut_name(item_id: Option<&str>) -> Option<String> {
    if text::is_null_or_white_space(item_id) {
        return None;
    }

    // Yol gelirse yalnız dosya adı alınır (PinnedOrder.IdOf ile aynı kural, her iki ayırıcıyla).
    let trimmed = item_id.unwrap_or_default().trim();
    let name = match trimmed.rfind(['\\', '/']) {
        Some(cut) => &trimmed[cut + 1..],
        None => trimmed,
    };
    if AppIconOverrides::is_valid_key(Some(name)) { Some(name.to_string()) } else { None }
}
