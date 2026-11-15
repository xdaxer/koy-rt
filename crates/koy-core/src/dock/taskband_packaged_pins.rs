use crate::text;

/// Windows Taskband `Favorites` ikili değerinden görev çubuğuna sabitlenmiş Mağaza (UWP / MSIX) uygulamalarının
/// AppUserModelID (AUMID) kimliklerini çıkarır. Windows 11'de bu uygulamalar `TaskBar` klasöründe `.lnk`
/// dosyası oluşturmaz; yalnız ikili `Favorites` kaydında yaşarlar.
pub struct TaskbandPackagedPins;

/// `char.IsControl`: Cc kategorisi (C0 ve C1 denetim karakterleri).
fn is_control_unit(u: u16) -> bool {
    u < 0x20 || (0x7F..=0x9F).contains(&u)
}

/// İkili değeri `alignment` bayt kaydırmayla UTF-16 (little endian) olarak çözer (`Encoding.Unicode.GetString`:
/// eşsiz vekil birimler U+FFFD olur) ve denetim karakteri ile vekil birimlerde bölünen koşuları döndürür:
/// `(koşunun UTF-16 birim konumu, koşu karakterleri)`. Koşular vekil içermediğinden her `char` bir UTF-16 birimidir.
pub(crate) fn utf16_runs(favorites: &[u8], alignment: usize) -> Vec<(usize, Vec<char>)> {
    let length = (favorites.len() - alignment) & !1usize;
    let mut units: Vec<u16> = Vec::with_capacity(length / 2);
    let mut i = alignment;
    while i + 1 < alignment + length {
        units.push(u16::from_le_bytes([favorites[i], favorites[i + 1]]));
        i += 2;
    }

    // Eşsiz vekil birimler yerine U+FFFD (.NET'in varsayılan yedek karakteri).
    let mut k = 0;
    while k < units.len() {
        let u = units[k];
        if (0xD800..0xDC00).contains(&u) {
            if k + 1 < units.len() && (0xDC00..0xE000).contains(&units[k + 1]) {
                k += 2;
                continue;
            }
            units[k] = 0xFFFD;
        } else if (0xDC00..0xE000).contains(&u) {
            units[k] = 0xFFFD;
        }
        k += 1;
    }

    let mut runs = Vec::new();
    let mut run_start: Option<usize> = None;
    let mut current: Vec<char> = Vec::new();
    for idx in 0..=units.len() {
        let in_run = idx < units.len() && !is_control_unit(units[idx]) && !(0xD800..0xE000).contains(&units[idx]);
        if in_run {
            if run_start.is_none() {
                run_start = Some(idx);
            }
            current.push(char::from_u32(units[idx] as u32).unwrap_or('\u{FFFD}'));
            continue;
        }

        if let Some(start) = run_start.take() {
            runs.push((start, std::mem::take(&mut current)));
        }
    }
    runs
}

impl TaskbandPackagedPins {
    pub const APPS_FOLDER_PREFIX: &'static str = "shell:AppsFolder\\";

    /// AUMID için kabuk yolu üretir: `shell:AppsFolder\<AUMID>`.
    pub fn apps_folder_path(aumid: &str) -> String {
        format!("{}{}", Self::APPS_FOLDER_PREFIX, aumid)
    }

    /// Yolun `shell:AppsFolder\` önekiyle başlayıp başlamadığını döndürür.
    pub fn is_apps_folder_path(path: Option<&str>) -> bool {
        match path {
            Some(p) if !p.is_empty() => text::starts_with_ignore_case(p, Self::APPS_FOLDER_PREFIX),
            _ => false,
        }
    }

    /// Taskband `Favorites` ikili verisindeki paketli uygulama AUMID'lerini (`PaketAdı_Yayıncı!Uygulama`)
    /// ilk görülme sırasıyla ve tekrarsız döndürür. Boş, bozuk ya da eşleşmesiz veride boş liste döner.
    pub fn extract_aumids(favorites: Option<&[u8]>) -> Vec<String> {
        let Some(favorites) = favorites else { return Vec::new() };
        if favorites.len() < 8 {
            return Vec::new();
        }

        // Dictionary(OrdinalIgnoreCase) + ekleme sırası: anahtar yazımı ilk görülenindir.
        let mut positions: Vec<(String, usize)> = Vec::new();

        for alignment in 0..2usize {
            let length = (favorites.len() - alignment) & !1usize;
            if length < 8 {
                continue;
            }

            for (run_start, run) in utf16_runs(favorites, alignment) {
                if Self::is_packaged_aumid_chars(&run) {
                    let aumid: String = run.iter().collect();
                    let byte_position = run_start * 2 + alignment;
                    match positions.iter_mut().find(|(k, _)| text::eq_ignore_case(k, &aumid)) {
                        Some(existing) => {
                            if byte_position < existing.1 {
                                existing.1 = byte_position;
                            }
                        }
                        None => positions.push((aumid, byte_position)),
                    }
                }
            }
        }

        // OrderBy kararlı.
        positions.sort_by_key(|(_, p)| *p);
        positions.into_iter().map(|(k, _)| k).collect()
    }

    /// Metnin paketli Windows uygulaması AUMID biçiminde (`PaketAdı_13KarakterBase32!UygulamaId`) olup olmadığını denetler.
    pub fn is_packaged_aumid(value: &str) -> bool {
        let chars: Vec<char> = value.chars().collect();
        Self::is_packaged_aumid_chars(&chars)
    }

    /// `is_packaged_aumid`'in `ReadOnlySpan<char>` karşılığı (PinnedOrder koşuları için).
    pub fn is_packaged_aumid_chars(value: &[char]) -> bool {
        let Some(bang) = value.iter().position(|&c| c == '!') else { return false };
        if bang == 0 || bang >= value.len() - 1 {
            return false;
        }

        let family = &value[..bang];
        let app_id = &value[bang + 1..];

        if app_id.len() > 64 {
            return false;
        }

        for &c in app_id {
            if !c.is_ascii_alphanumeric() && !matches!(c, '.' | '_' | '-') {
                return false;
            }
        }

        let Some(underscore) = family.iter().rposition(|&c| c == '_') else { return false };
        if underscore == 0 {
            return false;
        }

        let package_name = &family[..underscore];
        let publisher_id = &family[underscore + 1..];

        // Windows paket aile adında yayıncı kimliği her zaman 13 karakterlik Crockford Base32 karmasıdır.
        if publisher_id.len() != 13 {
            return false;
        }

        if !publisher_id.iter().all(|&c| Self::is_crockford_base32(c)) {
            return false;
        }

        if package_name.is_empty() || package_name.len() > 50 {
            return false;
        }

        if matches!(package_name[0], '.' | '-') || matches!(package_name[package_name.len() - 1], '.' | '-') {
            return false;
        }

        let mut has_alnum = false;
        for &c in package_name {
            if c.is_ascii_alphanumeric() {
                has_alnum = true;
            } else if !matches!(c, '.' | '-') {
                return false;
            }
        }

        has_alnum
    }

    fn is_crockford_base32(c: char) -> bool {
        if c.is_ascii_digit() {
            return true;
        }

        // (char)(c | 0x20)
        let lower = char::from_u32((c as u32) | 0x20).unwrap_or('\0');
        lower.is_ascii_lowercase() && !matches!(lower, 'i' | 'l' | 'o' | 'u')
    }
}
