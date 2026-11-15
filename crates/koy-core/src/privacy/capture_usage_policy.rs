//! Karşılığı: `Privacy/CaptureUsagePolicy.cs`.

use crate::text;

/// Kamera ve mikrofon gizlilik göstergesi durumu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrivacyState {
    pub is_camera_in_use: bool,
    pub is_microphone_in_use: bool,
    pub active_camera_app: Option<String>,
    pub active_microphone_app: Option<String>,
}

impl PrivacyState {
    /// `PrivacyState.Empty`.
    pub const EMPTY: PrivacyState = PrivacyState {
        is_camera_in_use: false,
        is_microphone_in_use: false,
        active_camera_app: None,
        active_microphone_app: None,
    };

    pub fn new(
        is_camera_in_use: bool,
        is_microphone_in_use: bool,
        active_camera_app: Option<String>,
        active_microphone_app: Option<String>,
    ) -> Self {
        PrivacyState { is_camera_in_use, is_microphone_in_use, active_camera_app, active_microphone_app }
    }

    pub fn any_in_use(&self) -> bool {
        self.is_camera_in_use || self.is_microphone_in_use
    }
}

impl Default for PrivacyState {
    fn default() -> Self {
        Self::EMPTY
    }
}

/// Windows ConsentStore kayıtlarına göre kamera ve mikrofon kullanımını değerlendiren saf mantık.
pub struct CaptureUsagePolicy;

impl CaptureUsagePolicy {
    /// Verilen başlangıç ve bitiş zamanlarına (FILETIME) göre aygıtın şu an aktif kullanımda olup olmadığını belirler.
    /// Başlangıç zamanı > 0 ve (bitiş zamanı 0 veya başlangıç zamanı > bitiş zamanı) ise kullanım aktiftir.
    pub fn is_actively_using(start_file_time: i64, stop_file_time: i64) -> bool {
        if start_file_time <= 0 {
            return false;
        }

        stop_file_time == 0 || start_file_time > stop_file_time
    }

    /// Aktif duruma göre kullanıcıya gösterilecek ToolTip metnini üretir. Kalıplar ve "bilinmeyen uygulama" adı
    /// arayüz katmanından gelir (Core resx'e erişemez).
    ///
    /// Kalıplar .NET bileşik biçim dizesidir (`{0}`, `{1}`); bkz. `format_composite`.
    pub fn format_tool_tip(
        state: &PrivacyState,
        format_cam_and_mic: &str,
        format_cam: &str,
        format_mic: &str,
        unknown_app: &str,
    ) -> String {
        let cam = state.active_camera_app.as_deref().unwrap_or(unknown_app);
        let mic = state.active_microphone_app.as_deref().unwrap_or(unknown_app);

        if state.is_camera_in_use && state.is_microphone_in_use {
            return format_composite(format_cam_and_mic, &[cam, mic]);
        }

        if state.is_camera_in_use {
            return format_composite(format_cam, &[cam]);
        }

        if state.is_microphone_in_use {
            return format_composite(format_mic, &[mic]);
        }

        String::new()
    }

    /// ConsentStore alt anahtar adından okunabilir uygulama adını ayıklar.
    /// Paket adı (UWP) veya NonPackaged yol (# ile ayrılmış) desteklenir. Ad yoksa `None` döner.
    pub fn extract_app_name_from_key(key_name: Option<&str>) -> Option<String> {
        if text::is_null_or_white_space(key_name) {
            return None;
        }

        let mut clean = text::trim(key_name?);

        // NonPackaged formatı: "C:#Program Files#obs-studio#bin#64bit#obs64.exe"
        if let Some(hash_idx) = clean.rfind('#') {
            if hash_idx < clean.len() - 1 {
                return Some(strip_exe(&clean[hash_idx + 1..]).to_string());
            }
        }

        // Standart dosya yolu: "C:\path\to\app.exe"
        let slash_idx = match (clean.rfind('\\'), clean.rfind('/')) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        };
        if let Some(slash_idx) = slash_idx {
            if slash_idx < clean.len() - 1 {
                return Some(strip_exe(&clean[slash_idx + 1..]).to_string());
            }
        }

        // UWP paket adı: "Microsoft.WindowsCamera_8wekyb3d8bbwe!App" veya "5319275A.WhatsAppDesktop_cv1g1gvanyjgm"
        if let Some(exclam_idx) = clean.find('!') {
            if exclam_idx > 0 {
                clean = &clean[..exclam_idx];
            }
        }

        if let Some(under_idx) = clean.find('_') {
            if under_idx > 0 {
                clean = &clean[..under_idx];
                if let Some(dot_idx) = clean.rfind('.') {
                    if dot_idx < clean.len() - 1 {
                        clean = &clean[dot_idx + 1..];
                    }
                }
            }
        }

        Some(clean.to_string())
    }
}

/// Sonu `.exe` ise (büyük/küçük harf duyarsız) uzantıyı atar (`fileName[..^4]`).
fn strip_exe(file_name: &str) -> &str {
    if text::ends_with_ignore_case(file_name, ".exe") { &file_name[..file_name.len() - 4] } else { file_name }
}

/// `string.Format(CultureInfo.CurrentCulture, format, args)` için dize argümanlı alt küme: `{n}`, `{n,hizalama}`,
/// `{n:biçim}` (dize argümanlarında biçim yok sayılır) ve `{{` / `}}` kaçışları. .NET'te `FormatException` fırlatan
/// durumlar (bozuk kalıp, argüman sırası dışı) burada panik olur.
fn format_composite(format: &str, args: &[&str]) -> String {
    let chars: Vec<char> = format.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '{' => {
                if chars.get(i + 1) == Some(&'{') {
                    out.push('{');
                    i += 2;
                    continue;
                }
                let close = chars[i + 1..].iter().position(|&x| x == '}').expect("FormatException: kapanmayan '{'") + i + 1;
                let spec: String = chars[i + 1..close].iter().collect();
                let (index_and_align, _format) = match spec.split_once(':') {
                    Some((a, f)) => (a.to_string(), Some(f.to_string())),
                    None => (spec.clone(), None),
                };
                let (index_text, align_text) = match index_and_align.split_once(',') {
                    Some((a, w)) => (a.to_string(), Some(w.to_string())),
                    None => (index_and_align.clone(), None),
                };
                let index: usize = index_text.trim().parse().expect("FormatException: bozuk bileşik biçim dizesi");
                let value = args.get(index).expect("FormatException: argüman dizini aralık dışı");
                let mut text = (*value).to_string();
                if let Some(w) = align_text {
                    let width: i32 = w.trim().parse().expect("FormatException: bozuk hizalama");
                    let pad = (width.unsigned_abs() as usize).saturating_sub(text.encode_utf16().count());
                    if width < 0 {
                        text.push_str(&" ".repeat(pad));
                    } else {
                        text = format!("{}{}", " ".repeat(pad), text);
                    }
                }
                out.push_str(&text);
                i = close + 1;
            }
            '}' => {
                if chars.get(i + 1) == Some(&'}') {
                    out.push('}');
                    i += 2;
                } else {
                    panic!("FormatException: eşleşmeyen '}}'");
                }
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}
