use std::cmp::Ordering;

use super::app_icon_overrides::{path_get_extension, path_get_file_name_without_extension};
use super::AppIconOverrides;
use crate::text;

/// Simge paketi klasöründeki dosyalardan bir uygulamanın simgesini seçer. Dosya adı (uzantısız) uygulama anahtarının
/// uzantısız hâliyle eşleşir: "Firefox.lnk" için "Firefox.png". Dosya okumaz; ad listesini çağıran verir.
pub struct IconPackFiles;

// Aynı ad birden çok uzantıyla varsa sıra: PNG, ICO, SVG.
const EXTENSION_ORDER: [&str; 3] = [".png", ".ico", ".svg"];
const SVG_RANK: usize = 2;

impl IconPackFiles {
    /// Paket klasörü, tek simge dosyası yolu gibi sınanır: yalnız sürücü harfli mutlak yol; ağ (UNC), aygıt yolu,
    /// nokta bileşeni ve yasak karakter yok. Klasörün var olduğuna bakmaz.
    pub fn is_safe_folder(folder: Option<&str>) -> bool {
        if text::is_null_or_white_space(folder) {
            return false;
        }
        let probe = format!("{}\\x.png", folder.unwrap_or_default().trim().trim_end_matches(['\\', '/']));
        AppIconOverrides::try_validate_source(Some(&probe)).is_some()
    }

    /// `file_names`: paket klasöründeki dosya adları (yol değil). `app_key`: uygulama anahtarı (`AppIconKey`).
    /// `allow_svg`: çizici SVG gösteremiyorsa false: SVG dosyaları yok sayılır.
    /// Seçilen dosya adı; eşleşme yoksa ya da ad geçersizse `None`.
    pub fn find<I, S>(file_names: I, app_key: Option<&str>, allow_svg: bool) -> Option<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        if !AppIconOverrides::is_valid_key(app_key) {
            return None;
        }

        let stem = stem_of(app_key.unwrap_or_default().trim());
        if stem.is_empty() {
            return None;
        }

        let mut best: Option<String> = None;
        let mut best_rank = usize::MAX;
        for name in file_names {
            let Some(valid) = AppIconOverrides::try_validate_pack_file(Some(name.as_ref())) else {
                continue;
            };

            let extension = path_get_extension(&valid);
            let Some(rank) = EXTENSION_ORDER.iter().position(|e| text::eq_ignore_case(e, extension)) else {
                continue;
            };
            if !allow_svg && rank == SVG_RANK {
                continue;
            }
            if !text::eq_ignore_case(path_get_file_name_without_extension(&valid), stem) {
                continue;
            }

            // Eşit sırada ada göre sabit seçim: klasör sırası sonucu değiştirmesin.
            let better = rank < best_rank
                || (rank == best_rank && compare_ordinal(&valid, best.as_deref().unwrap_or_default()) == Ordering::Less);
            if better {
                best = Some(valid);
                best_rank = rank;
            }
        }

        best
    }
}

/// `string.CompareOrdinal`: UTF-16 kod birimi sırası.
fn compare_ordinal(a: &str, b: &str) -> Ordering {
    a.encode_utf16().cmp(b.encode_utf16())
}

// AUMID'de uzantı yoktur ("Microsoft.WindowsTerminal_8wekyb3d8bbwe!App"); yalnız bilinen kısayol/exe uzantısı atılır.
fn stem_of(key: &str) -> &str {
    let ext = path_get_extension(key);
    if text::eq_ignore_case(ext, ".lnk") || text::eq_ignore_case(ext, ".exe") || text::eq_ignore_case(ext, ".url") {
        path_get_file_name_without_extension(key)
    } else {
        key
    }
}
