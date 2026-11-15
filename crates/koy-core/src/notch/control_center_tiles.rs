use crate::text;

/// Kontrol Merkezi döşemelerinin sırası (`AppSettings.ControlCenterLayout`). Kayıtlı liste eski, eksik ya da
/// bozuk olabilir; [`ControlCenterTiles::resolve`] her zaman bilinen üç döşemeyi tam olarak bir kez içeren bir sıra üretir.
pub struct ControlCenterTiles;

impl ControlCenterTiles {
    pub const WI_FI: &'static str = "wifi";
    pub const BLUETOOTH: &'static str = "bluetooth";
    pub const DO_NOT_DISTURB: &'static str = "donotdisturb";

    /// `DefaultOrder`.
    pub fn default_order() -> Vec<String> {
        vec![Self::WI_FI.to_string(), Self::BLUETOOTH.to_string(), Self::DO_NOT_DISTURB.to_string()]
    }

    /// Kayıtlı sırayı geçerli sıraya çevirir: büyük/küçük harf yok sayılır, bilinmeyenler ve tekrarlar atılır,
    /// listede olmayan döşemeler varsayılan sıralarıyla sona eklenir. Liste `None`/boşsa varsayılan sıra döner.
    pub fn resolve(saved: Option<&[String]>) -> Vec<String> {
        let default_order = Self::default_order();
        let mut result: Vec<String> = Vec::with_capacity(default_order.len());
        if let Some(saved) = saved {
            for raw in saved {
                let id = text::to_lower_invariant(text::trim(raw));
                if default_order.contains(&id) && !result.contains(&id) {
                    result.push(id);
                }
            }
        }

        for id in &default_order {
            if !result.contains(id) {
                result.push(id.clone());
            }
        }

        result
    }

    /// `id` döşemesini `to_index` konumuna taşır (aralık dışı değer uçlara sıkıştırılır).
    /// Bilinmeyen id için sıra olduğu gibi döner. Girdi değiştirilmez. (C# adı `Move`; Rust'ta anahtar sözcük.)
    pub fn r#move(order: &[String], id: &str, to_index: i32) -> Vec<String> {
        let mut result = order.to_vec();
        let Some(from) = result.iter().position(|x| x == id) else {
            return result;
        };

        result.remove(from);
        let at = text::clamp(to_index, 0, result.len() as i32) as usize;
        result.insert(at, id.to_string());
        result
    }

    /// Sürüklenen döşemenin merkezi `center_x` konumundayken hangi yuvaya düşeceği.
    /// Yuvalar soldan sağa eşit genişlikte (`slot_width`) ve aralarında `gap` boşlukla dizilir.
    pub fn slot_at(center_x: f64, slot_width: f64, gap: f64, count: i32) -> i32 {
        if count <= 1 || slot_width <= 0.0 {
            return 0;
        }

        let index = (center_x / (slot_width + gap)).floor() as i32;
        text::clamp(index, 0, count - 1)
    }

    /// İki sıra aynı mı (kaydetmeye değer bir değişiklik var mı).
    pub fn same_order(a: &[String], b: &[String]) -> bool {
        a == b
    }
}
