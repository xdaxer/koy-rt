use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use chrono::{NaiveDate, NaiveDateTime};

use super::{UserStyleDiagnostic, UserStyleDiagnosticCode, UserStyleParseResult, UserStyleParser};

/// Dosyanın değişip değişmediğini anlamak için parmak izi (var mı, boyut, son yazım). İçerik okunmaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct UserStyleFileStamp {
    pub exists: bool,
    pub length: i64,
    pub last_write_utc: NaiveDateTime,
}

impl Default for UserStyleFileStamp {
    /// `default(UserStyleFileStamp)`: yok, 0 bayt, `DateTime` varsayılanı (0001-01-01).
    fn default() -> Self {
        UserStyleFileStamp {
            exists: false,
            length: 0,
            last_write_utc: NaiveDate::from_ymd_opt(1, 1, 1).unwrap().and_hms_opt(0, 0, 0).unwrap(),
        }
    }
}

/// %AppData%\Koy\user-theme.json okuma. Dosya yoksa sonuç boştur ve tanı yoktur (özellik açık ama dosya henüz yazılmamış
/// olabilir). Boyut sınırı dosya belleğe alınmadan, uzunluğa bakılarak uygulanır. Okuma hataları tanıya çevrilir, fırlatmaz.
pub struct UserStyleFile;

impl UserStyleFile {
    pub const FILE_NAME: &'static str = "user-theme.json";

    /// `%AppData%\Koy` (`Environment.GetFolderPath(ApplicationData)` → `APPDATA`).
    pub fn default_folder() -> PathBuf {
        let app_data = std::env::var("APPDATA").unwrap_or_default();
        PathBuf::from(app_data).join("Koy")
    }

    pub fn default_path() -> PathBuf {
        Self::default_folder().join(Self::FILE_NAME)
    }

    pub fn get_stamp(path: &Path) -> UserStyleFileStamp {
        // `new FileInfo(path)` boş/geçersiz yolda ArgumentException atar; C# yakalayıp `default` döner.
        if !is_valid_path_argument(path) {
            return UserStyleFileStamp::default();
        }
        match fs::metadata(path) {
            Ok(meta) if meta.is_file() => {
                let last_write = meta
                    .modified()
                    .map(|t| chrono::DateTime::<chrono::Utc>::from(t).naive_utc())
                    .unwrap_or(UserStyleFileStamp::default().last_write_utc);
                UserStyleFileStamp { exists: true, length: meta.len() as i64, last_write_utc: last_write }
            }
            _ => UserStyleFileStamp::default(),
        }
    }

    pub fn load(path: &Path) -> UserStyleParseResult {
        if !is_valid_path_argument(path) {
            return Self::failure(UserStyleDiagnosticCode::FileUnreadable, "The file could not be read.");
        }

        let max_bytes = UserStyleParser::MAX_BYTES as u64;
        let info = match fs::metadata(path) {
            Ok(meta) if meta.is_file() => meta,
            _ => return UserStyleParseResult::empty(),
        };

        if info.len() > max_bytes {
            return Self::too_large();
        }

        // Okuma sırasında büyüyen dosya için ikinci sınır: en çok MaxBytes + 1 bayt alınır.
        let mut buffer: Vec<u8> = Vec::new();
        let read = File::open(path).and_then(|file| file.take(max_bytes + 1).read_to_end(&mut buffer));
        if read.is_err() {
            return Self::failure(UserStyleDiagnosticCode::FileUnreadable, "The file could not be read.");
        }

        if buffer.len() as u64 > max_bytes {
            return Self::too_large();
        }

        // Geçersiz UTF-8 dizileri U+FFFD olur; JSON ayrıştırıcısı bozuk içeriği zaten reddeder.
        let text = String::from_utf8_lossy(&buffer);
        UserStyleParser::parse(text.trim_start_matches('\u{FEFF}'))
    }

    fn too_large() -> UserStyleParseResult {
        Self::failure(
            UserStyleDiagnosticCode::FileTooLarge,
            &format!("File is larger than {} KB.", UserStyleParser::MAX_BYTES / 1024),
        )
    }

    fn failure(code: UserStyleDiagnosticCode, reason: &str) -> UserStyleParseResult {
        UserStyleParseResult::new(
            UserStyleParseResult::empty().valid_entries,
            vec![UserStyleDiagnostic::new(UserStyleParser::FILE_KEY, reason, code)],
            0,
        )
    }
}

/// `FileInfo` kurucusunun `ArgumentException` attığı yollar: boş, yalnız boşluk ya da NUL içeren.
fn is_valid_path_argument(path: &Path) -> bool {
    let s = path.to_string_lossy();
    !(s.trim().is_empty() || s.contains('\0'))
}
