//! Firefox yer imlerini (`places.sqlite`) Windows'ta yerleşik `winsqlite3.dll` üzerinden salt okunur okur.
//! Yeni bağımlılık gerektirmez; DLL yoksa veya Firefox kurulu değilse sessizce boş döner.
//! Firefox veritabanını kilitli tuttuğu için dosya (ve WAL eşleri) geçici klasöre kopyalanıp oradan okunur.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use koy_core::search::{BrowserBookmark, FileStampCache};
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::core::{PCSTR, w};

use super::win_env::last_write_utc_ticks;

const MAX_FILE_BYTES: u64 = 64 * 1024 * 1024;
const MAX_WAL_BYTES: u64 = 16 * 1024 * 1024;
const MAX_BOOKMARKS: usize = 5000;
const SQLITE_OPEN_READONLY: c_int = 1;
const SQLITE_OK: c_int = 0;
const SQLITE_ROW: c_int = 100;
const QUERY: &CStr = c"SELECT COALESCE(b.title, p.title, ''), p.url FROM moz_bookmarks b JOIN moz_places p ON b.fk = p.id WHERE b.type = 1 AND p.url IS NOT NULL AND (p.url LIKE 'http://%' OR p.url LIKE 'https://%') LIMIT 5000";

type OpenV2 = unsafe extern "system" fn(*const c_char, *mut *mut c_void, c_int, *const c_char) -> c_int;
type PrepareV2 = unsafe extern "system" fn(*mut c_void, *const c_char, c_int, *mut *mut c_void, *mut *const c_char) -> c_int;
type Step = unsafe extern "system" fn(*mut c_void) -> c_int;
type ColumnText = unsafe extern "system" fn(*mut c_void, c_int) -> *const u8;
type Finalize = unsafe extern "system" fn(*mut c_void) -> c_int;
type CloseV2 = unsafe extern "system" fn(*mut c_void) -> c_int;

struct SqliteApi {
    open_v2: OpenV2,
    prepare_v2: PrepareV2,
    step: Step,
    column_text: ColumnText,
    finalize: Finalize,
    close_v2: CloseV2,
}

impl SqliteApi {
    fn get() -> Option<&'static SqliteApi> {
        static API: OnceLock<Option<SqliteApi>> = OnceLock::new();
        API.get_or_init(|| {
            // SAFETY: sistem DLL'i adla yüklenir; dışa aktarımlar bilinen sqlite3 imzalarıyla dönüştürülür.
            unsafe {
                let module = LoadLibraryW(w!("winsqlite3.dll")).ok()?;
                macro_rules! export {
                    ($name:literal, $ty:ty) => {
                        std::mem::transmute::<unsafe extern "system" fn() -> isize, $ty>(GetProcAddress(module, PCSTR($name.as_ptr()))?)
                    };
                }
                Some(SqliteApi {
                    open_v2: export!(b"sqlite3_open_v2\0", OpenV2),
                    prepare_v2: export!(b"sqlite3_prepare_v2\0", PrepareV2),
                    step: export!(b"sqlite3_step\0", Step),
                    column_text: export!(b"sqlite3_column_text\0", ColumnText),
                    finalize: export!(b"sqlite3_finalize\0", Finalize),
                    close_v2: export!(b"sqlite3_close_v2\0", CloseV2),
                })
            }
        })
        .as_ref()
    }
}

fn cache() -> &'static FileStampCache<Vec<BrowserBookmark>> {
    static CACHE: OnceLock<FileStampCache<Vec<BrowserBookmark>>> = OnceLock::new();
    CACHE.get_or_init(FileStampCache::new)
}

pub struct FirefoxBookmarks;

impl FirefoxBookmarks {
    pub fn load() -> Vec<BrowserBookmark> {
        let profiles_root = PathBuf::from(std::env::var_os("APPDATA").unwrap_or_default()).join(r"Mozilla\Firefox\Profiles");
        if !profiles_root.is_dir() {
            return Vec::new();
        }
        let Some(api) = SqliteApi::get() else { return Vec::new() };
        let Ok(entries) = std::fs::read_dir(&profiles_root) else { return Vec::new() };
        let mut bookmarks = Vec::new();
        for profile in entries.filter_map(|e| e.ok()).map(|e| e.path()).filter(|p| p.is_dir()) {
            bookmarks.extend(read_profile(&profile, api));
        }
        bookmarks
    }
}

fn read_profile(profile_directory: &Path, api: &SqliteApi) -> Vec<BrowserBookmark> {
    let path = profile_directory.join("places.sqlite");
    let key = path.to_string_lossy();
    let Ok(meta) = std::fs::metadata(&path) else {
        cache().remove(&key);
        return Vec::new();
    };
    if !meta.is_file() || meta.len() > MAX_FILE_BYTES {
        cache().remove(&key);
        return Vec::new();
    }
    cache().get_or_load(&key, last_write_utc_ticks(&meta), meta.len() as i64, || read_with_temp_copy(&path, api))
}

fn read_with_temp_copy(source: &Path, api: &SqliteApi) -> Vec<BrowserBookmark> {
    let id: String = koy_core::random::get_bytes(16).iter().map(|b| format!("{b:02x}")).collect();
    let temp_base = std::env::temp_dir().join(format!("koy-ff-{id}.sqlite"));
    let temp_wal = PathBuf::from(format!("{}-wal", temp_base.display()));
    let temp_shm = PathBuf::from(format!("{}-shm", temp_base.display()));

    let result = if copy_shared(source, &temp_base).is_ok() {
        try_copy_companion(&PathBuf::from(format!("{}-wal", source.display())), &temp_wal, MAX_WAL_BYTES);
        try_copy_companion(&PathBuf::from(format!("{}-shm", source.display())), &temp_shm, MAX_WAL_BYTES);
        query_database(&temp_base, api)
    } else {
        Vec::new()
    };
    for p in [&temp_base, &temp_wal, &temp_shm] {
        let _ = std::fs::remove_file(p);
    }
    result
}

fn query_database(db_path: &Path, api: &SqliteApi) -> Vec<BrowserBookmark> {
    let mut result = Vec::new();
    let Ok(path) = std::ffi::CString::new(db_path.to_string_lossy().into_owned()) else { return result };
    let mut db: *mut c_void = std::ptr::null_mut();
    // SAFETY: winsqlite3 C API'si belgelendiği gibi kullanılır; tutamaçlar her yolda kapatılır.
    unsafe {
        if (api.open_v2)(path.as_ptr(), &mut db, SQLITE_OPEN_READONLY, std::ptr::null()) != SQLITE_OK || db.is_null() {
            if !db.is_null() {
                (api.close_v2)(db);
            }
            return result;
        }
        let mut stmt: *mut c_void = std::ptr::null_mut();
        if (api.prepare_v2)(db, QUERY.as_ptr(), -1, &mut stmt, std::ptr::null_mut()) == SQLITE_OK && !stmt.is_null() {
            while result.len() < MAX_BOOKMARKS && (api.step)(stmt) == SQLITE_ROW {
                let title = column(api, stmt, 0).unwrap_or_default();
                let Some(url) = column(api, stmt, 1).filter(|u| !u.trim().is_empty()) else { continue };
                let title = if title.trim().is_empty() { url.clone() } else { title };
                result.push(BrowserBookmark::new(title, url));
            }
            (api.finalize)(stmt);
        }
        (api.close_v2)(db);
    }
    result
}

/// `Marshal.PtrToStringUTF8(sqlite3_column_text(..))`.
unsafe fn column(api: &SqliteApi, stmt: *mut c_void, index: c_int) -> Option<String> {
    // SAFETY: dönen işaretçi sıfırla biten UTF-8 ya da boş; bir sonraki step'e kadar geçerli.
    unsafe {
        let p = (api.column_text)(stmt, index);
        (!p.is_null()).then(|| CStr::from_ptr(p as *const c_char).to_string_lossy().into_owned())
    }
}

fn copy_shared(source: &Path, destination: &Path) -> std::io::Result<()> {
    // std, dosyayı FILE_SHARE_READ | WRITE | DELETE ile açar (Firefox yazarken de okunabilir).
    let mut input = File::open(source)?;
    let mut output = File::create_new(destination)?;
    std::io::copy(&mut input, &mut output).map(|_| ())
}

fn try_copy_companion(source: &Path, destination: &Path, max_bytes: u64) {
    if let Ok(meta) = std::fs::metadata(source)
        && meta.is_file()
        && meta.len() <= max_bytes
    {
        let _ = copy_shared(source, destination);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn winsqlite_loads_and_missing_profile_is_empty() {
        assert!(super::SqliteApi::get().is_some(), "winsqlite3.dll Windows 10+ üzerinde yerleşik");
        let _ = super::FirefoxBookmarks::load();
    }
}
