//! Karşılığı: `JumpList/JumpListModels.cs`.

/// Atlama listesi bölümü türü. Sıra, menüdeki sıra değildir; menü sırası `JumpListLayout` içindedir.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JumpListSectionKind {
    /// Uygulamanın kendi adlandırdığı kategori (ör. VS Code'un son klasörleri).
    Custom,
    Frequent,
    Recent,
    Tasks,
}

/// Bir girdi tıklanınca çalıştırılacak şey; yalnız dizelerden oluşur (WinRT / Win32 tipi yok), bu yüzden önbelleğe
/// alınabilir ve COM nesnesi taşımaz.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct JumpListLaunch {
    /// Çalıştırılacak yol, URL ya da kabuk ayrıştırma adı.
    pub target: String,
    /// Görev kısayolunun komut satırı (belge girdilerinde boş).
    pub arguments: Option<String>,
    /// Görev kısayolunun çalışma klasörü.
    pub working_directory: Option<String>,
    /// Hedef bir belge / klasör / URL (son ve sık kullanılanlar); görev kısayolu değil.
    pub is_document: bool,
}

impl JumpListLaunch {
    /// `new JumpListLaunch(target)`: yalnız hedef, görev olmayan varsayılanlarla.
    pub fn new(target: impl Into<String>) -> Self {
        JumpListLaunch { target: target.into(), ..Default::default() }
    }
}

/// Menüde bir satır: başlık ve çalıştırma bilgisi. `launch` boşsa satır bir ayırıcıdır.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JumpListEntry {
    pub title: String,
    pub launch: Option<JumpListLaunch>,
}

impl JumpListEntry {
    pub fn new(title: impl Into<String>, launch: Option<JumpListLaunch>) -> Self {
        JumpListEntry { title: title.into(), launch }
    }

    /// `JumpListEntry.Separator`.
    pub fn separator() -> Self {
        JumpListEntry { title: String::new(), launch: None }
    }

    pub fn is_separator(&self) -> bool {
        self.launch.is_none()
    }
}

/// Menüde başlıklı bir bölüm. `name` yalnız `JumpListSectionKind::Custom` için doludur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JumpListSection {
    pub kind: JumpListSectionKind,
    pub name: Option<String>,
    pub entries: Vec<JumpListEntry>,
}

impl JumpListSection {
    pub fn new(kind: JumpListSectionKind, name: Option<String>, entries: Vec<JumpListEntry>) -> Self {
        JumpListSection { kind, name, entries }
    }
}
