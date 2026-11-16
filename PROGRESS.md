# Taşıma ilerlemesi

| Katman | Modül | Durum | Not |
|---|---|---|---|
| core | time, text, event, cancel, win_path (ortak) | ✅ | win_path: .NET Windows Path (GetFileName, GetExtension, GetDirectoryName, GetFullPath, Combine, IsPathRooted). cancel: CancellationToken/Source, OperationCanceled. | C# TimeSpan/TimeProvider/FakeClock/StepTimeProvider, bankacı yuvarlaması, OrdinalIgnoreCase |
| core | voice, audio, radio, wireless | ✅ | 60 test. Async yok (engelleyen çağrılar, işçi iş parçacığında çağrılmalı). `CancellationToken` ortak `cancel.rs`e taşındı. Bekleyen: VoicePermissionSettingTests (Settings) |
| core | shell, tray, visibility, quick_launch, network, power | ✅ | 189 test (ortak dahil). Bekleyen: QuickLaunch `set_error_log` → diagnostics CrashLog'a bağlanacak; `TrayIconVisibilityPolicy` HashMap alıyor → Settings `ValueDictionary` gelince uyarla; Dock/Notch/Notifications'a ait testler (EdgeRevealTracker, TrayGlyphs, TrayBalloonNotification) o modüllerle |
| core | appearance, media, animation, background | ✅ | 156 test. Settings gelince: `impl MotionToggles for MotionSettings`, `impl IconOverrideMap for ValueDictionary` (büyük/küçük harf duyarsız); Settings'e bağlı 16 test bekliyor (MotionToggles, AppIconOverride, FontScalePolicy, UserStyleOverrides, MusicCardStyleSetting) |
| core | jump_list, lyrics, diagnostics, uninstall, privacy | ✅ | 158 test. LyricsSession yüklemeyi iş parçacığında yapar (`LoadTask::wait` = await). Bekleyen: Uninstall takvim testi (Calendar) |
| core | calendar, clock, timers, units, shelf | ✅ | 135 test. clock'ta `Culture` (Invariant/en-US/en-GB/tr-TR/de-DE), `DayOfWeek`, `TimeZoneInfo` (yerleşik ~33 Windows bölgesi; kayıt defteri okuması Platform'a), .NET tarih biçim motoru. Rastgelelik `crate::random` üzerinden; Platform: `koy_platform::security::install_secure_random()` (BCryptGenRandom) — uygulama açılışında çağrılmalı. EventAgenda başlık sırası kültürsüz (ordinal). Bekleyen: Settings/Weather'a bağlı 5 test |
| core | **hub**: settings (+profiles), dock (+items), display, notch, search, clipboard, rules, shortcuts, tasks, weather, input | ✅ | Bütünleşik: `--features hub` 1943 test; çalışma alanı 2095 test yeşil. Settings JSON'u kendi `json_tree.rs` ile System.Text.Json'a bayt uyumlu yazar/okur (PascalCase, .NET kaçışları, CRLF). Açık: üç ayrı `Guid` (tray: .NET bayt düzeni, clipboard, tasks) birleştirilecek; `JsonListFile` tarih biçimi chrono RFC3339 (System.Text.Json'dan küçük fark); `EDGE_REVEAL_DEFAULT_DELAY_MS` settings'te elle eşlenmiş; `AppLanguage::set_system_culture` Platform açılışında çağrılmalı |
| core | notifications, system_stats, bluetooth | ✅ | 120 test. NotificationPoller: engelleyen okuma + TimeProvider tek atımlık zamanlayıcı (2 sn / hata sonrası 10 sn) |
| core | updates, start_menu | ✅ | 52 test. CHANGELOG'lar `include_str!` ile `../koy/` klasöründen gömülür |
| core | extensions (+store) | ✅ | 249 test. Elle yazılmış inflate (RFC 1951) ve ZIP okuyucu; Unicode Cf/Cn tablosu gömülü. `IExtensionStoreClient` uygulaması Platform'da |
| **core** | **TAMAM** | ✅ | **Çalışma alanı: 2516 test yeşil, uyarısız** |

| platform | net (WinHTTP), weather, lyrics, calendar, extensions, search, security, registry/locale/wmi yardımcıları, jump_list, start_menu, shelf, radio, wireless, network, power, diagnostics, bluetooth, display, audio, media, voice, clipboard, privacy, system_stats, notifications, shell (tamamı) | ✅ | **TAMAM**: 60 test yeşil, uyarısız. Sapmalar: PNG sıkıştırmasız zlib (yeni crate yok); FocusAssist olay bağlantısı kurucuda; WindowTracker başlık bildirimi `post` ile; WMI `System.Management` yerine doğrudan COM |

## Sıradaki adımlar (oturum yarıda kalırsa buradan devam)
1. ✅ Dalga 1 (yaprak modüller) ve ✅ merkez grubu (hub): bütünleşik, testler yeşil.
2. ✅ Dalga 3: notifications, system_stats, bluetooth, extensions, updates, start_menu.
3. Ortak temizlik: `Guid` tekleştir, `iso_time` (calendar + extensions) ve `to_string_fixed` (units + system_stats) kopyalarını birleştir, feature kapılarını kaldır (tek crate, kapısız), `cargo test` tamamı yeşil.
4. ✅ **Platform** (`koy-platform`): bütün C# Platform servisleri taşındı.
5. **App** (`koy-app`, Slint 1.18 winit + yazılım çizici; WPF ~43k satır). Karar: Slint (GPLv3 uyumlu).
   - ✅ Altyapı: `infra/` (dispatch: post/post_local/delay, message_window, native_window: HWND/stil/DWM/alt sınıf kancası,
     system_events, single_instance, ui_ref: Rc'ye Send jeton, snapshot: `koy --snapshot <görünüm> <png>`).
   - ✅ `shared_services.rs`, `update_status.rs`, kısayol / ekran köşesi / tepsi simgesi denetleyicileri.
   - ✅ Kaynaklar: `resources/` (strings: resx→tablo build.rs, theme_manager: Slint `Theme` global'i, pure callback +
     `rev` ile DynamicResource; palette.rs ve ui/theme.slint, ui/icons.slint `tools/gen_*.py` ile XAML'den üretilir),
     motion, material_state, window_frame, user_style_watcher. `ui/controls.slint` (Styles.xaml), `ui/notch_controls.slint`.
   - ✅ Menü: `features/menu` (SagAltMenuWindow, TrayIconManager).
   - ✅ Çentik alt panelleri: Status (VM + WeatherGlyph), SystemInfo, Focus/Tasks (ArcRing), Shelf (OLE sürükle-bırak:
     `infra/ole_dnd.rs`), ClipboardPanel, Calendar (SharedEvents + Google/Outlook eşitleme). Önizleme:
     `koy --snapshot <system|focus|timer|shelf|clipboard|calendar|calweek> <png>`.
   - ✅ Görünüm modelleri: Music/Lyrics (WIC kapak çözümü `infra/image_decode.rs`), Home + QuickLaunch (dosya seçici
     `infra/file_dialog.rs`), ControlCenter (radyo döşemeleri, ses, parlaklık, Wi-Fi sayfası, Bluetooth), Search (22 kaynak,
     `infra/clipboard_text.rs`), OSD, LiveBackgroundState, NotchViewModel. `ui/notch.slint`: çentik yüzeyi, kapalı / göz atma
     yerleşimi, açık çentik üst satırı (derleniyor, henüz görsel doğrulanmadı).
   - Çentik tamam (kod): `notch_shape.rs` (yol + SetWindowRgn), Slint görünümleri (Ana/müzik, Kontrol Merkezi + Wi-Fi /
     Bluetooth sayfaları, Arama, OSD/kart; önizlemelerle doğrulandı), `ui/notch_window.slint`, `notch_window.rs` (yaylar,
     bölge, fare / tekerlek / sürükle-bırak, etkinleştirme), `notch_window_binding.rs` (sayfa modellerine canlı bağlama),
     `notch_feature.rs` (ekran başına pencere, servisler, tam ekran / kenar, kısayollar, OSD olayları),
     `notch_feature_cards.rs` (güncelleme kartı, bildirim kuyruğu, sesli komut), `voice_reply_window.rs`,
     `notch_line_window.rs`. Geliştirici kipi: `koy.exe --notch [--quit-after N]`. Not: arayüz iş parçacığında OLE
     `main` başında başlatılır (winit sürükle-bırak ister). Gerçek masaüstünde görsel doğrulama ve ince ayar bekliyor.
   - Sıradaki: Dock (~8.5k satır), Settings/KoyWindow (~11.7k), Extensions, Rules, Wizard, `app.rs` (App.xaml.cs:
     özellikleri, kısayolları, tepsiyi ve güncelleme kartı → Ayarlar bağlantısını kurar) ve main.
   - Desen: C# ViewModel → Rust struct (`Rc`), görünüm durumu Slint bileşen özelliklerine / global'lere yazılır; olaylar
     arka plandan `dispatch::post` ile, Rc'ye `UiRef` ile ulaşılır. Pencereler `ThemeManager::install` ile temaya bağlanır.
   - Açık: otomatik güncelleme (C# Velopack) — paketleme kararıyla; UiScalePercent menüde `ui-scale` ile, diğer
     pencerelerde de aynı desen.
