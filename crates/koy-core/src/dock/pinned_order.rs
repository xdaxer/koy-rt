use std::collections::HashSet;

use super::dock_folder::DockFolder;
use super::i_pinned_apps_service::PinnedApp;
use super::taskband_packaged_pins::{utf16_runs, TaskbandPackagedPins};
use crate::text;
use crate::win_path;

/// Büyük/küçük harf duyarsız (OrdinalIgnoreCase) küme: anahtarlar büyük harfe çevrilerek tutulur.
struct IgnoreCaseSet(HashSet<String>);

impl IgnoreCaseSet {
    fn new() -> Self {
        IgnoreCaseSet(HashSet::new())
    }

    fn from_iter<'a>(it: impl IntoIterator<Item = &'a String>) -> Self {
        let mut s = Self::new();
        for x in it {
            s.add(x);
        }
        s
    }

    /// `HashSet.Add`: yeni eklendiyse true.
    fn add(&mut self, s: &str) -> bool {
        self.0.insert(text::to_upper_invariant(s))
    }

    fn contains(&self, s: &str) -> bool {
        self.0.contains(&text::to_upper_invariant(s))
    }
}

/// Sabitli uygulamaların sırası (saf mantık). Sıra Koy'un ayarlarında (`AppSettings.DockPinnedOrder`) tutulur;
/// kimlik, görev çubuğu klasöründeki .lnk dosya adıdır (örn. "Obsidian.lnk"). Windows'un kendi sırası yalnız ilk
/// açılışta tohum olarak okunur.
pub struct PinnedOrder;

impl PinnedOrder {
    pub const FOLDER_PREFIX: &'static str = "folder:";

    /// Belirtilen kimliğin bir klasör belirteci olup olmadığını döndürür.
    pub fn is_folder_id(id: &str) -> bool {
        text::starts_with_ignore_case(id, Self::FOLDER_PREFIX)
    }

    /// Klasör kimliğini dock sıralama belirtecine dönüştürür ("folder:<Id>").
    pub fn to_folder_token(folder_id: &str) -> String {
        format!("{}{}", Self::FOLDER_PREFIX, folder_id)
    }

    /// Klasör belirtecinden klasör kimliğini ayıklar ("folder:abc" -> "abc").
    pub fn extract_folder_id(folder_token: &str) -> String {
        if text::starts_with_ignore_case(folder_token, Self::FOLDER_PREFIX) {
            // Önek ASCII: ilk 7 karakter = ilk 7 bayt.
            folder_token.chars().skip(Self::FOLDER_PREFIX.len()).collect()
        } else {
            folder_token.to_string()
        }
    }

    /// Sabitli uygulamanın sıralama kimliği: kısayol dosyasının adı (yoldan bağımsız, kullanıcıdan bağımsız).
    pub fn id_of(shortcut_path: &str) -> String {
        win_path::get_file_name(shortcut_path).to_string()
    }

    /// Kayıtlı sırayı uygular. Kayıtta olanlar kayıt sırasında, kayıtta olmayan yeni uygulamalar sona (verilen sırayla) gelir.
    pub fn apply(apps: &[PinnedApp], order: &[String]) -> Vec<PinnedApp> {
        if apps.len() <= 1 || order.is_empty() {
            return apps.to_vec();
        }

        // rank.TryAdd(id, rank.Count): ilk görülen yazım sırayı belirler.
        let mut ranks: Vec<&String> = Vec::new();
        for id in order {
            if !ranks.iter().any(|r| text::eq_ignore_case(r, id)) {
                ranks.push(id);
            }
        }

        // Kararlı sıralama: aynı sıradakiler (kayıtta olmayanlar) gelen sırayı korur.
        let mut keyed: Vec<(usize, usize, &PinnedApp)> = apps
            .iter()
            .enumerate()
            .map(|(index, app)| {
                let id = Self::id_of(&app.shortcut_path);
                let rank = ranks.iter().position(|r| text::eq_ignore_case(r, &id)).unwrap_or(usize::MAX);
                (rank, index, app)
            })
            .collect();
        keyed.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
        keyed.into_iter().map(|(_, _, app)| app.clone()).collect()
    }

    /// Kayıtlı sırayı mevcut kimliklerle eşitler: kaldırılanlar listeden düşer, yeni sabitlenenler sona eklenir.
    /// Klasör belirteçleri ('folder:<Id>') tanınır ve korunur.
    /// Mevcut kimlik listesi boşsa (klasör okunamadı) kayıt bozulmasın diye aynen döner.
    pub fn reconcile(saved: &[String], current_ids: &[String]) -> Vec<String> {
        if current_ids.is_empty() {
            return saved.to_vec();
        }

        let present = IgnoreCaseSet::from_iter(current_ids);
        let mut result = Vec::new();
        let mut seen = IgnoreCaseSet::new();

        for id in saved {
            if Self::is_folder_id(id) {
                if seen.add(id) {
                    result.push(id.clone());
                }
            } else if present.contains(id) && seen.add(id) {
                result.push(id.clone());
            }
        }

        for id in current_ids {
            if seen.add(id) {
                result.push(id.clone());
            }
        }

        result
    }

    /// Klasör farkındalıklı sıra eşitlemesi:
    /// - Klasör içindeki uygulamalar ana dock sırasında ayrıca görünmez.
    /// - Diskten silinmiş kısayollar klasörlerden ayıklanır.
    /// - Klasörde 1 öğe kalırsa klasör dağıtılır ve kalan öğe klasörün sırasına yerleştirilir.
    /// - Klasörde 0 öğe kalırsa klasör silinir ve belirteci dock sırasından kaldırılır.
    /// - Kalan aktif klasörler (>= 2 öğe) ve güncellenmiş dock sırası döndürülür.
    pub fn reconcile_with_folders(saved: &[String], current_ids: &[String], folders: &[DockFolder]) -> (Vec<String>, Vec<DockFolder>) {
        if current_ids.is_empty() {
            return (saved.to_vec(), folders.to_vec());
        }

        let available = IgnoreCaseSet::from_iter(current_ids);

        // 1. Klasörlerdeki silinmiş kısayolları temizle. Dictionary(OrdinalIgnoreCase): aynı Id ikinci kez gelirse değer
        //    yerinde değişir, sıra ilk eklenene göre kalır.
        let mut pruned: Vec<(DockFolder, Vec<String>)> = Vec::new();
        for f in folders {
            let mut valid_items: Vec<String> = Vec::new();
            let mut distinct = IgnoreCaseSet::new();
            for item in &f.items {
                if available.contains(item) && distinct.add(item) {
                    valid_items.push(item.clone());
                }
            }
            match pruned.iter_mut().find(|(pf, _)| text::eq_ignore_case(&pf.id, &f.id)) {
                Some(entry) => *entry = (f.clone(), valid_items),
                None => pruned.push((f.clone(), valid_items)),
            }
        }

        // Aktif klasörler: en az 2 öğesi olanlar
        let active_folders: Vec<DockFolder> = pruned
            .iter()
            .filter(|(_, valid)| valid.len() >= 2)
            .map(|(folder, valid)| DockFolder { items: valid.clone(), ..folder.clone() })
            .collect();

        // Aktif klasörlerin içindeki uygulamalar dock kök düzeyinde görünmemeli
        let mut apps_in_folders = IgnoreCaseSet::new();
        for f in &active_folders {
            for item in &f.items {
                apps_in_folders.add(item);
            }
        }

        let mut result_order: Vec<String> = Vec::new();
        let mut seen = IgnoreCaseSet::new();

        // 2. Kayıtlı sırayı tara
        for id in saved {
            if Self::is_folder_id(id) {
                let folder_id = Self::extract_folder_id(id);
                if let Some((folder, valid)) = pruned.iter().find(|(pf, _)| text::eq_ignore_case(&pf.id, &folder_id)) {
                    if valid.len() >= 2 {
                        let token = Self::to_folder_token(&folder.id);
                        if seen.add(&token) {
                            result_order.push(token);
                        }
                    } else if valid.len() == 1 {
                        // 1 öğe kaldı: klasör dağılır, kalan tek öğe klasörün yuvasına geçer
                        let single_item = &valid[0];
                        if !apps_in_folders.contains(single_item) && seen.add(single_item) {
                            result_order.push(single_item.clone());
                        }
                    }
                    // 0 öğe kaldı: klasör silinir (belirteç eklenmez)
                }
            } else {
                // Kök düzeyindeki kısayol (herhangi bir aktif klasörde yoksa ve diskte mevcutsa)
                if !apps_in_folders.contains(id) && available.contains(id) && seen.add(id) {
                    result_order.push(id.clone());
                }
            }
        }

        // 3. Sıraya henüz girmemiş aktif klasörler (yeni oluşturulmuş)
        for f in &active_folders {
            let token = Self::to_folder_token(&f.id);
            if seen.add(&token) {
                result_order.push(token);
            }
        }

        // 4. Henüz görülmemiş ve klasörlerde olmayan diskteki yeni kısayolları sona ekle
        for id in current_ids {
            if !apps_in_folders.contains(id) && seen.add(id) {
                result_order.push(id.clone());
            }
        }

        (result_order, active_folders)
    }

    /// Bir öğeyi yeni dizine taşır (dizin sınırlara kırpılır). Kimlik listede yoksa sıra değişmeden döner.
    /// (C# adı `Move`; Rust'ta `move` ayrılmış sözcük olduğundan `move_item`.)
    pub fn move_item(order: &[String], id: &str, new_index: i32) -> Vec<String> {
        let mut result = order.to_vec();
        let Some(from) = result.iter().position(|x| text::eq_ignore_case(x, id)) else {
            return result;
        };

        let item = result.remove(from);
        let index = text::clamp(new_index, 0, result.len() as i32) as usize;
        result.insert(index, item);
        result
    }

    /// Windows'un Taskband `Favorites` ikili değerinden ilk sıra tohumunu çıkarır. Biçim belgesizdir: bayt yapısı
    /// çözülmez, yalnız bilinen .lnk adlarının değerde geçtiği yerlere bakılır (UTF-16, iki hizalama). Bulunanlar
    /// değerdeki sırayla, bulunamayanlar verilen (klasör) sırasıyla arkadan gelir. Değer boş, bozuk ya da hiçbir adı
    /// içermiyorsa sonuç klasör sırasıdır; hiçbir koşulda istisna fırlatmaz.
    pub fn seed_from_taskband(favorites: Option<&[u8]>, folder_order_ids: &[String]) -> Vec<String> {
        let fallback = folder_order_ids.to_vec();
        let Some(favorites) = favorites else { return fallback };
        if favorites.len() < 8 || folder_order_ids.is_empty() {
            return fallback;
        }

        let mut positions: Vec<(String, usize)> = Vec::new();

        for alignment in 0..2usize {
            let length = (favorites.len() - alignment) & !1usize;
            if length < 8 {
                continue;
            }

            // Metin, denetim karakterlerinde bölünen koşulara ayrılır. Ad bir koşunun SONUNDA olmalı ve önünde
            // en çok 2 artık karakter bulunabilir (ikili uzunluk baytları yazdırılabilir görünür: "UObsidian.lnk").
            // Böylece "Code.lnk", "Visual Studio Code.lnk" koşusunda ya da "A.lnk", "Baska.lnk" içinde yanlış eşleşmez.
            for (run_start, run) in utf16_runs(favorites, alignment) {
                Self::match_run(&run, run_start, alignment, folder_order_ids, &mut positions);
            }
        }

        if positions.is_empty() {
            return fallback;
        }

        let position_of = |id: &str| positions.iter().find(|(k, _)| text::eq_ignore_case(k, id)).map(|(_, p)| *p);

        let mut found: Vec<&String> = folder_order_ids.iter().filter(|id| position_of(id).is_some()).collect();
        // OrderBy kararlı.
        found.sort_by_key(|id| position_of(id).unwrap_or(0));
        let rest = folder_order_ids.iter().filter(|id| position_of(id).is_none());

        found.into_iter().cloned().chain(rest.cloned()).collect()
    }

    /// Adın önündeki artık ikili veri mi (gerçek bir dosya adının başı değil mi)? En çok bir karakter (uzunluk baytı)
    /// her şey olabilir; ondan öncesi okunabilir harf/rakam içermemeli ("Baska.lnk" içindeki "A.lnk" reddedilir).
    fn is_binary_noise(prefix: &[char]) -> bool {
        if prefix.len() <= 1 {
            return true;
        }

        for &c in &prefix[..prefix.len() - 1] {
            if c.is_ascii_alphanumeric() || c == ' ' {
                return false;
            }
        }

        true
    }

    fn match_run(run: &[char], run_start: usize, alignment: usize, ids: &[String], positions: &mut Vec<(String, usize)>) {
        let run_text: String = run.iter().collect();
        let is_lnk = text::ends_with_ignore_case(&run_text, ".lnk");
        let is_packaged = !is_lnk && TaskbandPackagedPins::is_packaged_aumid_chars(run);
        if !is_lnk && !is_packaged {
            return;
        }

        let mut best: Option<(&String, usize)> = None; // (id, UTF-16 uzunluğu)
        for id in ids {
            let id_len = id.encode_utf16().count();
            if id.is_empty() || id_len > run.len() {
                continue;
            }

            let tail: String = run[run.len() - id_len..].iter().collect();
            if text::eq_ignore_case(&tail, id)
                && Self::is_binary_noise(&run[..run.len() - id_len])
                && best.is_none_or(|(_, best_len)| id_len > best_len)
            {
                best = Some((id, id_len));
            }
        }

        let Some((best_id, best_len)) = best else { return };

        let byte_position = (run_start + run.len() - best_len) * 2 + alignment;
        match positions.iter_mut().find(|(k, _)| text::eq_ignore_case(k, best_id)) {
            Some(existing) => {
                if byte_position < existing.1 {
                    existing.1 = byte_position;
                }
            }
            None => positions.push((best_id.clone(), byte_position)),
        }
    }
}
