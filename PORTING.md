# Koy → Rust taşıma kuralları

Amaç: `koy/` (C# + WPF) projesini **algoritma mantığını olabildiğince koruyarak** Rust'a taşımak.
Bu dosya taşımada çalışan herkes (insan ya da ajan) için kuraldır. Kaynak projenin kuralları `koy/AGENTS.md`'de;
özellikle değişmez kurallar (bellek bütçesi, yalnız gerektiğinde çalışan servisler, görev çubuğu güvenliği) aynen geçerli.

## Yapı
```
koy-rt/
  Cargo.toml                 çalışma alanı
  crates/koy-core/           koy/src/Core karşılığı (saf mantık, Windows API ve arayüz yok)
    src/lib.rs               modül listesi (feature kapılı)
    src/time.rs              TimeSpan, TimeProvider, Timer, testing::{FakeClock, StepTimeProvider}
    src/text.rs              C# davranışlı yuvarlama, OrdinalIgnoreCase, Türkçe büyük/küçük harf
    src/event.rs             C# `event` karşılığı Event<T>
    src/<modül>/mod.rs       her C# klasörü bir modül (snake_case: JumpList → jump_list)
    src/<modül>/<dosya>.rs   her C# dosyası bir Rust dosyası (snake_case: NotchController.cs → notch_controller.rs)
    src/<modül>/tests/       o modülün testleri (koy/tests/Core.Tests'ten)
```
İleride: `crates/koy-platform` (koy/src/Platform, `windows` crate), `crates/koy-app` (arayüz), `crates/koy-watchdog`.

## Derleme ve test
- Araç zinciri `stable-x86_64-pc-windows-gnu`; `cargo` için `export PATH="$HOME/.cargo/bin:$PATH"`.
- Taşıma süresince her modül kendi cargo feature'ının arkasındadır (`Cargo.toml` → `[features]`).
  Yalnız kendi modülünü derle: `cargo test -p koy-core --no-default-features --features <modül>`.
  Başka modülün yarım hali seni durdurmaz.
- Paralel çalışırken her ajan kendi hedef klasörünü kullanır: `CARGO_TARGET_DIR=target/<ajan-adı>`.
- Modül başka bir Core modülüne bağlıysa feature'ı `Cargo.toml`'da o modülü de açar (örn. `media = ["appearance"]`).
- Bitti: `cargo test` yeşil, `cargo clippy` gerekmez ama derleme **uyarısız** olmalı (kullanılmayan kod uyarısı modülün
  herkese açık API'si `pub` olduğu için çıkmaz; çıkıyorsa gerçekten ölü koddur).

## Birebir taşıma kuralları
1. **Mantık aynı kalır.** Sabitler, eşikler, sıralama, kenar durumları, hata yolları C#'taki gibi. "Daha iyisi" yazılmaz;
   C#'ta bir tuhaflık görürsen aynen taşı ve dosyada `// C# ile aynı:` diye not düş, raporunda da belirt.
2. **Yorumlar Türkçe**, C#'taki *neden* anlatan yorumlar taşınır (XML doc → `///`).
3. **Adlar:** tipler PascalCase kalır (`NotchController`), metot/alan/özellik snake_case (`GetText` → `get_text`,
   `IsOpen` → `is_open()`), sabitler `SCREAMING_CASE`. Enum değerleri PascalCase kalır.
4. **Sayı tipleri aynen:** `int`→`i32`, `long`→`i64`, `double`→`f64`, `float`→`f32`, `byte`→`u8`, `uint`→`u32`,
   `nint`→`isize`. Tamsayı bölmesi ve `(int)x` kesmesi Rust'ta da sıfıra doğrudur (`as i32`).
5. **Yuvarlama:** `Math.Round(x)` = `crate::text::round(x)` (bankacı!). `Math.Round(x, n)` = `text::round_digits`.
   `MidpointRounding.AwayFromZero` = `f64::round`. Asla düz `f64::round` ile `Math.Round` taşınmaz.
   `Math.Clamp` = `text::clamp` (min > max panikler, C# istisnası gibi) ya da `f64::clamp`.
6. **Metin karşılaştırması:** `OrdinalIgnoreCase` → `text::eq_ignore_case` / `contains_ignore_case` / `starts_with_ignore_case`…
   `Ordinal` → düz `==`. `ToLowerInvariant` → `text::to_lower_invariant`. Türkçe kültür → `text::to_lower_tr`.
   `string.IsNullOrWhiteSpace` → `text::is_null_or_white_space`. Karakter konumu gerekiyorsa `chars()` ile çalış:
   C# indeksleri UTF-16'dır; testler Türkçe karakter içeriyorsa uzunluk/indeksin karakter sayısıyla tuttuğunu denetle.
7. **null:** `T?` → `Option<T>`. `string` alanı null olabiliyorsa `Option<String>`, olamıyorsa `String`.
8. **record / class:**
   - Değer taşıyan `record`/`record struct` → `#[derive(Debug, Clone, PartialEq)]` struct (gerekirse `Eq, Hash, Default`).
     `with { }` ifadeleri → `Struct { alan: x, ..eski.clone() }`.
   - Varsayılan değerli kayıtlar → elle `impl Default` (C#'taki varsayılan değerlerle birebir).
   - `interface` → `trait`. Platform servis arayüzleri trait olur; testlerdeki sahte servisler trait uygular.
   - `static class` → modül seviyesinde serbest fonksiyonlar ya da birimsiz struct'ta ilişkili fonksiyonlar
     (çağrı biçimi `DockLayoutMath::compute(..)` korunacaksa struct tercih edilir).
   - Kalıtım → trait + bileşim; `abstract class SearchSource` gibi tabanlar trait + ortak yardımcı fonksiyonlar.
9. **Olaylar:** `event Action<T>` → `crate::event::Event<T>` alanı. `EventHandler` (sender'lı) → `Event<T>` yeterli.
10. **Zaman:** `TimeSpan` → `crate::time::TimeSpan` (tick tabanlı, birebir). `TimeProvider` → `Arc<dyn TimeProvider>`
    (`crate::time::TimeProvider`). `ITimer` → `Box<dyn Timer>`; `Timeout.InfiniteTimeSpan` → `TimeSpan::INFINITE`.
    `DateTimeOffset` → `crate::time::DateTimeOffset` (chrono `DateTime<FixedOffset>`), `DateTime` → `chrono::NaiveDateTime`
    (Kind önemliyse not düş), `DateOnly` → `chrono::NaiveDate`, `TimeOnly` → `chrono::NaiveTime`.
    `DateTime.Now` gibi doğrudan saat çağrıları C#'taki gibi doğrudan kalır (`chrono::Local::now()`).
    Test saatleri: `crate::time::testing::{FakeClock, StepTimeProvider}` (C#'takilerle aynı davranış).
    Zamanlayıcı geri çağrısı `Send` olmalı: durumu kendini yeniden kuran bileşenlerde `Arc<Mutex<İç>>` + `Weak` kalıbı
    kullan, geri çağrı içinde kilidi tutarken dış olayı (`Event::invoke`) tetikleme — önce kilidi bırak.
11. **JSON:** System.Text.Json → `serde` + `serde_json`. Dosya biçimi **birebir** korunur (C# sürümünün yazdığı dosyayı
    Rust okuyabilmeli, tersi de). Özellik adları C#'ta PascalCase yazılıyorsa `#[serde(rename_all = "PascalCase")]`;
    `JsonPropertyName` → `#[serde(rename = "...")]`. Enum'lar `UseStringEnumConverter` ile metin olarak yazılır.
    Eksik alan varsayılanı alır (`#[serde(default)]`), bilinmeyen alan yok sayılır. `WriteIndented = true` →
    `serde_json::to_string_pretty` (girinti 2 boşluk, C# ile aynı).
12. **Regex:** `regex` crate. .NET'e özgü yapılar (lookbehind, `RegexOptions.IgnoreCase` kültür farkı) varsa elle yaz,
    not düş.
13. **Koleksiyonlar:** `List<T>` → `Vec<T>`, `Dictionary` → `HashMap` (sıra önemliyse — örn. JSON yazımı, sıralı
    yineleme — `Vec<(K,V)>` ya da `BTreeMap`/`indexmap` yerine düz vektör; C#'ta Dictionary ekleme sırasını korur!),
    `HashSet` → `HashSet`, `IReadOnlyList` → `&[T]` ya da `Vec<T>`. LINQ `OrderBy` **kararlıdır** → `sort_by` (kararlı)
    kullan, `sort_unstable` kullanma. `StringComparer.OrdinalIgnoreCase` ile sözlük → anahtarı büyük harfe çevirerek ya da
    karşılaştırıcılı arama ile.
14. **İstisnalar:** C#'ta yakalanıp sonuç dönen yollar `Result`/`Option` olur. Çağırana fırlayan `ArgumentException`
    gibi programcı hataları `panic!` (testler `Assert.Throws` ile bunu sınıyorsa `#[should_panic]`).
15. **Dosya sistemi:** Core'daki dosya okuma/yazma (`File.ReadAllText`, `Directory`) `std::fs` ile aynen. Yollar
    `std::path::PathBuf`. `Environment.GetFolderPath(AppData)` → `std::env::var("APPDATA")` (Windows hedefli).
16. **Yeni crate eklenmez.** Kullanılabilir: `serde`, `serde_json`, `chrono`, `regex`, `sha2`. Başka bir şey gerekirse
    (base64, zip…) önce küçük bir yardımcıyı kendin yaz; olmuyorsa ekleme, raporunda iste.
17. **unsafe yok** (Core'da gerekmez).

## Testler
- `koy/tests/Core.Tests` altındaki ilgili test dosyası `src/<modül>/tests/<dosya>.rs` olur ve modülün `mod.rs`'inde
  `#[cfg(test)] mod tests;` ile bağlanır (`tests/mod.rs` dosyaları listeler).
- Her `[Fact]` bir `#[test]`. `[Theory] + [InlineData]` → her satır için ayrı çağrı yapan tek test ya da tablo döngüsü
  (hata mesajında hangi satırın düştüğü görünmeli).
- Test adları snake_case: `Opens_after_hover_delay` → `opens_after_hover_delay`.
- Test **beklentileri değiştirilmez**. Bir test Rust'ta geçmiyorsa sebep taşıma hatasıdır; düzelt. Gerçekten
  taşınamıyorsa (WPF/Windows bağımlı) `#[ignore = "sebep"]` ile bırak ve raporla.
- Başka (henüz taşınmamış) modüllere ihtiyaç duyan testleri şimdilik taşıma; raporunda listele.

## İlerleme
`PROGRESS.md` modül modül durumu tutar. Paralel çalışmada onu, `Cargo.toml`'u ve `lib.rs`'i yalnız koordinatör günceller;
alt ajanlar gereken değişikliği raporunda yazar.
