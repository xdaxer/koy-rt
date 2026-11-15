//! Karşılığı: `JumpList/JumpListLimits.cs`.

/// Atlama listesi verisi güvenilmeyen ikili veridir (her uygulama kendi dosyasını yazar). Okuyucu ve menü bu üst
/// sınırların dışına çıkmaz; aşan girdi bozuk sayılır.
pub struct JumpListLimits;

impl JumpListLimits {
    /// `customDestinations-ms` dosyası için en büyük boyut. Gerçek dosyalar onlarca KB'dir.
    pub const MAX_FILE_BYTES: i32 = 2 * 1024 * 1024;

    pub const MAX_CATEGORIES: i32 = 16;
    pub const MAX_ITEMS_PER_CATEGORY: i32 = 64;
    pub const MAX_TOTAL_ITEMS: i32 = 256;

    /// Tek bir ShellLink akışı için en büyük boyut.
    pub const MAX_ITEM_BYTES: i32 = 64 * 1024;

    /// Kategori adı için en çok UTF-16 karakter (bunu aşan dosya bozuk sayılır).
    pub const MAX_CATEGORY_NAME_CHARS: i32 = 128;

    /// Menüde görünen başlık en çok bu kadar karakterdir; fazlası "…" ile kesilir.
    pub const MAX_TITLE_CHARS: i32 = 120;

    pub const MAX_TARGET_CHARS: i32 = 4096;
    pub const MAX_ARGUMENTS_CHARS: i32 = 8192;

    // Menü ekranı taşmasın: bölüm başına ve toplam sınır (ayırıcılar sayılmaz).
    pub const MAX_CUSTOM_SECTIONS: i32 = 3;
    pub const MAX_PER_CUSTOM_SECTION: i32 = 6;
    pub const MAX_FREQUENT: i32 = 3;
    pub const MAX_RECENT: i32 = 5;
    pub const MAX_TASKS: i32 = 6;

    /// Bütün bölümlerdeki toplam girdi sınırı; aşılırsa önce sık, sonra son kullanılanlar kırpılır.
    pub const MAX_TOTAL_ENTRIES: i32 = 14;

    /// Toplam sınır yüzünden kırpılırken bir bölümde en az bu kadar girdi kalır.
    pub const MIN_PER_SECTION_WHEN_TRIMMING: i32 = 2;
}
