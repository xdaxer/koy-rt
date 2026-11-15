use std::collections::HashSet;

use super::dock_folder::{new_short_id, DockFolder};
use super::pinned_order::PinnedOrder;
use crate::text;

/// Dock klasörleri üzerinde saf durum geçişleri ve kuralları yöneten etki alanı motoru.
/// Tüm metotlar saf çalışır; giriş koleksiyonlarını değiştirmeden yeni değişmez durumlar üretir.
pub struct DockFolders;

fn contains_ignore_case(list: &[String], value: &str) -> bool {
    list.iter().any(|x| text::eq_ignore_case(x, value))
}

fn find_folder<'a>(folders: &'a [DockFolder], id: &str) -> Option<&'a DockFolder> {
    folders.iter().find(|f| text::eq_ignore_case(&f.id, id))
}

impl DockFolders {
    /// Bir uygulamanın diğerinin üzerine bırakılmasıyla yeni bir klasör oluşturur.
    /// Hedef zaten bir klasör ise `add_to_folder` metoduna yönlendirir.
    pub fn create_from_drop(
        order: &[String],
        folders: &[DockFolder],
        dragged: &str,
        target: &str,
        default_name: &str,
    ) -> (Vec<String>, Vec<DockFolder>) {
        if text::eq_ignore_case(dragged, target) {
            return (order.to_vec(), folders.to_vec());
        }

        // Klasörler başka bir öğenin içine sürüklenemez (iç içe klasör yok)
        if PinnedOrder::is_folder_id(dragged) {
            return (order.to_vec(), folders.to_vec());
        }

        // Hedef zaten bir klasör ise klasöre ekleme yap
        if PinnedOrder::is_folder_id(target) {
            return Self::add_to_folder(order, folders, dragged, &PinnedOrder::extract_folder_id(target));
        }

        // Yeni benzersiz klasör kimliği üret
        let folder_id = new_short_id(8);
        let folder_token = PinnedOrder::to_folder_token(&folder_id);

        let mut new_order: Vec<String> = order.to_vec();

        // Sürüklenen öğeyi ana listeden çıkar
        new_order.retain(|x| !text::eq_ignore_case(x, dragged));

        // Hedefin yerine klasör belirtecini yerleştir
        match new_order.iter().position(|x| text::eq_ignore_case(x, target)) {
            Some(i) => new_order[i] = folder_token,
            None => new_order.push(folder_token),
        }

        let new_folder = DockFolder::new(folder_id, default_name, vec![target.to_string(), dragged.to_string()]);
        let mut new_folders = folders.to_vec();
        new_folders.push(new_folder);

        (new_order, new_folders)
    }

    /// Sürüklenen uygulamayı mevcut bir klasöre ekler ve ana dock sırasından kaldırır.
    pub fn add_to_folder(order: &[String], folders: &[DockFolder], dragged: &str, folder_id: &str) -> (Vec<String>, Vec<DockFolder>) {
        if PinnedOrder::is_folder_id(dragged) {
            return (order.to_vec(), folders.to_vec());
        }

        let f_id = PinnedOrder::extract_folder_id(folder_id);
        let Some(target_folder) = find_folder(folders, &f_id) else {
            return (order.to_vec(), folders.to_vec());
        };

        let mut new_order: Vec<String> = order.to_vec();
        new_order.retain(|x| !text::eq_ignore_case(x, dragged));

        if contains_ignore_case(&target_folder.items, dragged) {
            return (new_order, folders.to_vec());
        }

        let new_folders: Vec<DockFolder> = folders
            .iter()
            .map(|f| {
                if !text::eq_ignore_case(&f.id, &f_id) {
                    return f.clone();
                }

                let mut items = f.items.clone();
                items.push(dragged.to_string());
                DockFolder { items, ..f.clone() }
            })
            .collect();

        (new_order, new_folders)
    }

    /// Bir uygulamayı klasörden çıkarıp dock sırasındaki belirtilen konuma yerleştirir.
    /// Klasörde 1 öğe kalırsa klasör otomatik dağılır (kalan öğe klasörün sırasına geçer).
    /// Klasörde 0 öğe kalırsa klasör silinir.
    pub fn remove_from_folder(
        order: &[String],
        folders: &[DockFolder],
        folder_id: &str,
        item: &str,
        insert_index: i32,
    ) -> (Vec<String>, Vec<DockFolder>) {
        let f_id = PinnedOrder::extract_folder_id(folder_id);
        let target_folder = match find_folder(folders, &f_id) {
            Some(t) if contains_ignore_case(&t.items, item) => t,
            _ => return (order.to_vec(), folders.to_vec()),
        };

        let mut new_order: Vec<String> = order.to_vec();
        let mut new_folders: Vec<DockFolder> = Vec::new();
        let remaining_items: Vec<String> = target_folder.items.iter().filter(|x| !text::eq_ignore_case(x, item)).cloned().collect();
        let folder_token = PinnedOrder::to_folder_token(&target_folder.id);

        if remaining_items.len() >= 2 {
            for f in folders {
                new_folders.push(if text::eq_ignore_case(&f.id, &f_id) {
                    DockFolder { items: remaining_items.clone(), ..f.clone() }
                } else {
                    f.clone()
                });
            }
        } else if remaining_items.len() == 1 {
            // Tek öğe kaldı: klasör dağılır, kalan tek öğe klasörün yuvasına geçer
            for f in folders {
                if !text::eq_ignore_case(&f.id, &f_id) {
                    new_folders.push(f.clone());
                }
            }

            let single_item = remaining_items[0].clone();
            match new_order.iter().position(|x| text::eq_ignore_case(x, &folder_token)) {
                Some(i) => new_order[i] = single_item,
                None => new_order.push(single_item),
            }
        } else {
            // 0 öğe kaldı: klasör silinir
            for f in folders {
                if !text::eq_ignore_case(&f.id, &f_id) {
                    new_folders.push(f.clone());
                }
            }

            new_order.retain(|x| !text::eq_ignore_case(x, &folder_token));
        }

        // Çıkarılan öğeyi dock sırasına ekle
        new_order.retain(|x| !text::eq_ignore_case(x, item));
        let clamped_index = text::clamp(insert_index, 0, new_order.len() as i32) as usize;
        new_order.insert(clamped_index, item.to_string());

        (new_order, new_folders)
    }

    /// Klasörü dağıtır; içindeki tüm uygulamalar klasörün dock'taki sırasına ardışık olarak dizilir.
    pub fn dissolve(order: &[String], folders: &[DockFolder], folder_id: &str) -> (Vec<String>, Vec<DockFolder>) {
        let f_id = PinnedOrder::extract_folder_id(folder_id);
        let Some(target_folder) = find_folder(folders, &f_id) else {
            return (order.to_vec(), folders.to_vec());
        };

        let folder_token = PinnedOrder::to_folder_token(&f_id);
        let mut new_order: Vec<String> = Vec::new();
        let mut replaced = false;

        for id in order {
            if text::eq_ignore_case(id, &folder_token) {
                new_order.extend(target_folder.items.iter().cloned());
                replaced = true;
            } else {
                new_order.push(id.clone());
            }
        }

        if !replaced {
            new_order.extend(target_folder.items.iter().cloned());
        }

        let new_folders: Vec<DockFolder> = folders.iter().filter(|f| !text::eq_ignore_case(&f.id, &f_id)).cloned().collect();
        (new_order, new_folders)
    }

    /// Klasörün adını günceller. Boş veya salt boşluklu adlar yok sayılır ve eski ad korunur.
    pub fn rename(folders: &[DockFolder], folder_id: &str, new_name: &str) -> Vec<DockFolder> {
        let f_id = PinnedOrder::extract_folder_id(folder_id);
        let trimmed = text::trim(new_name);
        if trimmed.is_empty() {
            return folders.to_vec();
        }

        folders
            .iter()
            .map(|f| if text::eq_ignore_case(&f.id, &f_id) { DockFolder { name: trimmed.to_string(), ..f.clone() } } else { f.clone() })
            .collect()
    }

    /// Klasör içindeki öğelerin sırasını indekslere göre değiştirir.
    pub fn reorder_in_folder(folders: &[DockFolder], folder_id: &str, old_index: i32, new_index: i32) -> Vec<DockFolder> {
        let f_id = PinnedOrder::extract_folder_id(folder_id);
        folders
            .iter()
            .map(|f| {
                if !text::eq_ignore_case(&f.id, &f_id) {
                    return f.clone();
                }

                let mut items = f.items.clone();
                if old_index < 0 || old_index as usize >= items.len() {
                    return f.clone();
                }

                let item = items.remove(old_index as usize);
                let target = text::clamp(new_index, 0, items.len() as i32) as usize;
                items.insert(target, item);
                DockFolder { items, ..f.clone() }
            })
            .collect()
    }

    /// Klasör içindeki öğelerin sırasını öğe adına göre değiştirir (C# `ReorderInFolder(folders, folderId, string item, newIndex)`).
    pub fn reorder_in_folder_by_item(folders: &[DockFolder], folder_id: &str, item: &str, new_index: i32) -> Vec<DockFolder> {
        let f_id = PinnedOrder::extract_folder_id(folder_id);
        folders
            .iter()
            .map(|f| {
                if !text::eq_ignore_case(&f.id, &f_id) {
                    return f.clone();
                }

                let mut items = f.items.clone();
                let Some(from) = items.iter().position(|x| text::eq_ignore_case(x, item)) else {
                    return f.clone();
                };

                items.remove(from);
                let target = text::clamp(new_index, 0, items.len() as i32) as usize;
                items.insert(target, item.to_string());
                DockFolder { items, ..f.clone() }
            })
            .collect()
    }

    /// Çakışmayan bir sonraki varsayılan klasör adını üretir ("Klasör", "Klasör 2", "Klasör 3"...).
    pub fn next_default_name(folders: &[DockFolder], base_name: &str) -> String {
        let existing: HashSet<String> = folders.iter().map(|f| text::to_upper_invariant(&f.name)).collect();
        let has = |s: &str| existing.contains(&text::to_upper_invariant(s));
        if !has(base_name) {
            return base_name.to_string();
        }

        for i in 2..1000 {
            let candidate = format!("{base_name} {i}");
            if !has(&candidate) {
                return candidate;
            }
        }

        format!("{} {}", base_name, new_short_id(4))
    }

    /// Klasörleri ve dock sırasını mevcut kısayol dosyalarına göre eşitler.
    pub fn reconcile(order: &[String], folders: &[DockFolder], current_available_lnk_ids: &[String]) -> (Vec<String>, Vec<DockFolder>) {
        PinnedOrder::reconcile_with_folders(order, current_available_lnk_ids, folders)
    }
}
