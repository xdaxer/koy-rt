# koy-rt

[Koy](https://github.com/koyhq/koy)'un Rust ile yeniden yazımı. Koy, Windows 10 ve 11 için hafif bir **çentik** (ekranın
üst ortasında yay animasyonlu ada) ve görev çubuğunun yerine geçen bir **dock**'tur. Özgün sürüm C# + WPF ile yazılmıştır.

> **Durum: erken aşama, henüz çalışan bir uygulama değil.** Şu an çekirdek mantık (Core) ve Windows servisleri (Platform)
> taşınmış ve testleriyle doğrulanmış durumda. Arayüz (App) henüz taşınmadı. Kullanmak için
> [Koy'un C# sürümünü](https://github.com/koyhq/koy/releases) indirin.

## Amaç

Algoritma mantığını **olabildiğince birebir korumak**. Sabitler, eşikler, sıralama kuralları ve kenar durumları C#
sürümüyle aynıdır. C#'taki testler de Rust'a taşınır ve aynı beklentilerle geçmek zorundadır. Bir C# davranışı
tuhaf görünse bile "düzeltilmez"; aynen taşınır ve kodda `// C# ile aynı:` notuyla işaretlenir.

C# sürümünün kuralları burada da geçerlidir:

- Boşta 100–150 MB bellek bütçesi.
- Tarayıcı motoru yok.
- Servisler yalnız gerektiğinde çalışır.
- Koy nasıl kapanırsa kapansın Windows görev çubuğu geri gelir.

## İlerleme

| Katman | C# karşılığı | Durum |
|---|---|---|
| `koy-core` | `src/Core` (saf mantık, ~30 bin satır) | Tamamı taşındı, testleriyle: 2516 test |
| `koy-platform` | `src/Platform` (Win32/WinRT servisleri) | Tamam: bütün Win32/WinRT servisleri (60 test) |
| `koy-watchdog` | `src/Watchdog` | Taşındı |
| arayüz | `src/App` (WPF) | Başlamadı; arayüz çatısı seçiliyor (aday: [Slint](https://slint.dev), yazılımla çizim) |

Modül modül ayrıntı ve sıradaki adımlar için [`PROGRESS.md`](PROGRESS.md) dosyasına bakın.

## Yapı

```
crates/
  koy-core/       Saf mantık: ayarlar, dock ve çentik kararları, arama, zamanlayıcılar, takvim, sözler…
                  Windows API'si ve arayüz kodu içermez. Her C# klasörü bir modüldür (Dock/ → dock/).
  koy-platform/   Core sözleşmelerinin Windows uygulamaları (`windows` crate).
  koy-watchdog/   Koy ölürse görev çubuğunu geri getiren küçük gözcü süreç.
```

`koy-core` içindeki ortak yardımcılar, C# davranışını birebir verecek şekilde yazılmıştır:

- `time`: tick tabanlı `TimeSpan`, `TimeProvider` ve test saatleri.
- `text`: bankacı yuvarlaması (`Math.Round`), `OrdinalIgnoreCase` ve Türkçe büyük/küçük harf.
- `win_path`: .NET'in Windows `Path` kuralları.
- `cancel`, `event`, `random`.

## Derleme

Gereken:

- Rust 1.85 veya üstü (2024 sürümü).
- Windows.
- `koy` deposunun bu deponun **yanında** bulunması. Bazı testler C# sürümünün kaynak dosyalarını ve değişiklik
  günlüklerini okur:

```
klasör/
  koy/       https://github.com/koyhq/koy
  koy-rt/    bu depo
```

```powershell
git clone https://github.com/koyhq/koy
git clone https://github.com/xdaxer/koy-rt
cd koy-rt
cargo test --workspace
```

**MSVC araç zinciri** (Visual Studio Build Tools) ile ek bir ayar gerekmez.

**GNU araç zinciri** (`x86_64-pc-windows-gnu`) kullanıyorsanız:

1. MinGW'den `dlltool` gerekir (örneğin [WinLibs](https://winlibs.com)).
2. `.cargo/config.example.toml` dosyasını `.cargo/config.toml` olarak kopyalayın.
3. İçindeki yolu kendi `dlltool.exe` dosyanıza göre düzeltin.

Çekirdek modüller taşıma süresince cargo feature'larıyla ayrılmıştır; varsayılan `all` feature'ı hepsini açar.
Tek bir modülü derlemek için:

```powershell
cargo test -p koy-core --no-default-features --features calendar
```

## Katkı

Taşıma kuralları [`PORTING.md`](PORTING.md) dosyasında. Özetle:

- Mantık değiştirilmez.
- Testlerin beklentileri değiştirilmez.
- Yorumlar Türkçe yazılır.
- Yeni bağımlılık eklenmeden önce konuşulur.

## Lisans

[GPL-3.0](LICENSE), özgün Koy ile aynı lisans.
