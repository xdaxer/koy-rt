use std::collections::HashSet;

use super::dock_item_entry::DockItemEntry;
use super::dock_item_kind::DockItemKind;
use super::dock_item_spec::DockItemSpec;
use crate::dock::pinned_order::PinnedOrder;
use crate::text;

/// Dock öğe dizisinin saf kuralları: doğrulama, tekilleştirme, sıra ve PinnedOrder ile çift yönlü dönüşüm.
/// Çalıştırma ve çizim burada yok. Özel düğmenin komutu yalnız saklanır.
pub struct DockItemRules;

/// `Path.GetInvalidFileNameChars()` (Windows): `" < > |`, NUL ve 1-31 denetim karakterleri, `: * ? \ /`.
fn is_invalid_file_name_char(c: char) -> bool {
    (c as u32) < 32 || matches!(c, '"' | '<' | '>' | '|' | ':' | '*' | '?' | '\\' | '/')
}

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

impl DockItemRules {
    /// Dock'taki en çok öğe. Bellek ve çizim bütçesi için (AGENTS #1).
    pub const MAX_ITEM_COUNT: i32 = 32;

    /// Özel komut uzunluk sınırı; bunu aşan komutlu öğe atılır.
    pub const MAX_COMMAND_LENGTH: i32 = 2048;

    /// Tür metnini ayrıştırır (büyük/küçük harf duyarsız). Bilinmeyen metinde `None` döner.
    pub fn try_parse_kind(text: Option<&str>) -> Option<DockItemKind> {
        let text = text?;
        match text::to_lower_invariant(text::trim(text)).as_str() {
            "app" => Some(DockItemKind::App),
            "folder" => Some(DockItemKind::Folder),
            "separator" => Some(DockItemKind::Separator),
            "spacer" => Some(DockItemKind::Spacer),
            "clock" => Some(DockItemKind::Clock),
            "trash" => Some(DockItemKind::Trash),
            "custom" => Some(DockItemKind::Custom),
            _ => None,
        }
    }

    /// Ham kayıtları doğrular ve sırasını korur. Bilinmeyen tür, boş kimlik, dosya adı olamayan uygulama kimliği,
    /// boş ya da çok uzun özel komut atılır.
    /// Tekilleştirme: App, Folder ve Custom kendi türünde kimliğe göre (büyük/küçük harf duyarsız) tekildir;
    /// Clock ve Trash en fazla bir kez bulunur; Separator ve Spacer tekrarlanabilir.
    /// Sınır aşılınca kalan kayıtlar atılır (ilk gelenler kalır). `max_count` C# varsayılanı: `MAX_ITEM_COUNT`.
    ///
    /// C#'taki `entry is null` denetimi Rust'ta gerekmez (dizi boş referans taşıyamaz).
    pub fn normalize(entries: &[DockItemEntry], max_count: i32) -> Vec<DockItemSpec> {
        let mut result: Vec<DockItemSpec> = Vec::new();
        let mut seen_keys: HashSet<String> = HashSet::new();

        for entry in entries {
            if result.len() as i64 >= max_count as i64 {
                break;
            }
            let Some(kind) = Self::try_parse_kind(Some(&entry.kind)) else { continue };

            let mut id = text::trim(&entry.id).to_string();
            let mut command = text::trim(&entry.command).to_string();
            let key: String;

            match kind {
                DockItemKind::App | DockItemKind::Folder => {
                    if id.is_empty() {
                        continue;
                    }
                    // Uygulama kimliği .lnk dosya adıdır; "folder:x" ya da yol/geçersiz karakter PinnedOrder'a yazılınca
                    // başka bir şeye (klasöre) dönüşür ya da hiçbir kısayolla eşleşmez.
                    if kind == DockItemKind::App && (PinnedOrder::is_folder_id(&id) || id.chars().any(is_invalid_file_name_char)) {
                        continue;
                    }
                    command = String::new();
                    key = format!("{}:{}", kind, id);
                }

                DockItemKind::Custom => {
                    // Komut Core'da yalnız saklanır; boş ya da aşırı uzun komutlu düğme anlamsızdır.
                    if id.is_empty() || command.is_empty() || utf16_len(&command) > Self::MAX_COMMAND_LENGTH as usize {
                        continue;
                    }
                    key = format!("{}:{}", kind, id);
                }

                DockItemKind::Clock | DockItemKind::Trash => {
                    // Tekil öğeler: kimlik türün adına sabitlenir, böylece iki kez eklenemez.
                    id = kind.to_string();
                    command = String::new();
                    key = format!("{}:{}", kind, id);
                }

                _ => {
                    // Ayırıcı ve boşluk kimliksiz ve tekrarlanabilir; anahtar her seferinde benzersiz.
                    id = String::new();
                    command = String::new();
                    key = format!("{}:{}", kind, result.len());
                }
            }

            if !seen_keys.insert(text::to_upper_invariant(&key)) {
                continue;
            }

            result.push(DockItemSpec::new(kind, id, command));
        }

        result
    }

    /// Öğeyi `from_index` konumundan `to_index` konumuna taşır ve yeni bir liste döndürür.
    /// Geçersiz kaynak indeksinde liste değişmeden kopyalanır; hedef sınırların dışındaysa uca kenetlenir.
    /// (C# adı `Move`; Rust'ta `move` ayrılmış sözcük olduğundan `move_item`.)
    pub fn move_item(items: &[DockItemSpec], from_index: i32, to_index: i32) -> Vec<DockItemSpec> {
        let mut copy = items.to_vec();
        if from_index < 0 || from_index as usize >= copy.len() {
            return copy;
        }

        let target = text::clamp(to_index, 0, copy.len() as i32 - 1);
        if target == from_index {
            return copy;
        }

        let item = copy.remove(from_index as usize);
        copy.insert(target as usize, item);
        copy
    }

    /// Eski PinnedOrder listesini öğe kayıtlarına çevirir (sıra aynen korunur). "folder:" belirteçleri klasör olur.
    /// PinnedOrder ayarı değişmez; bu yalnız yeni listeye geçişte ya da karşılaştırmada kullanılır.
    pub fn from_pinned_order(pinned_order: &[String]) -> Vec<DockItemEntry> {
        let mut entries = Vec::new();
        for token in pinned_order {
            if text::is_null_or_white_space(Some(token)) {
                continue;
            }

            entries.push(if PinnedOrder::is_folder_id(token) {
                DockItemEntry::new("folder", PinnedOrder::extract_folder_id(token), "")
            } else {
                DockItemEntry::new("app", token.clone(), "")
            });
        }
        entries
    }

    /// Dock'un gösterecek öğe listesi. Kayıtlı `items` varsa o doğrulanıp kullanılır; hiç kayıt yoksa
    /// (ilk geçiş) bugünkü dock korunsun diye `pinned_order` öğelere çevrilir. Kayıtlar var ama hepsi
    /// geçersizse kullanıcının sırası sessizce yeniden türetilmez: sonuç boş döner.
    /// Üst sınır geçişte de uygulanır: 32'den fazla sabitli uygulaması olan kullanıcı fazlasını bu listede görmez
    /// (PinnedOrder ayarı yine dokunulmaz kalır). `max_count` C# varsayılanı: `MAX_ITEM_COUNT`.
    pub fn resolve(items: &[DockItemEntry], pinned_order: &[String], max_count: i32) -> Vec<DockItemSpec> {
        if !items.is_empty() {
            Self::normalize(items, max_count)
        } else {
            Self::normalize(&Self::from_pinned_order(pinned_order), max_count)
        }
    }

    /// Öğe listesinden PinnedOrder biçimindeki sırayı üretir: uygulamalar kısayol adı, klasörler "folder:" belirteciyle.
    /// Özel düğme, ayırıcı, boşluk, saat ve çöp kutusu PinnedOrder'a yazılmaz.
    pub fn to_pinned_order(items: &[DockItemSpec]) -> Vec<String> {
        let mut order = Vec::new();
        for item in items {
            if item.kind == DockItemKind::App {
                order.push(item.id.clone());
            } else if item.kind == DockItemKind::Folder {
                order.push(PinnedOrder::to_folder_token(&item.id));
            }
        }
        order
    }
}
