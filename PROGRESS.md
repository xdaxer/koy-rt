# Taşıma ilerlemesi

| Katman | Modül | Durum | Not |
|---|---|---|---|
| core | time, text, event (ortak) | ✅ | C# TimeSpan/TimeProvider/FakeClock/StepTimeProvider, bankacı yuvarlaması, OrdinalIgnoreCase |
| core | voice, audio, radio, wireless | ✅ | 60 test. Async yok (engelleyen çağrılar, işçi iş parçacığında çağrılmalı). `CancellationToken` ortak `cancel.rs`e taşındı. Bekleyen: VoicePermissionSettingTests (Settings) |

## Sıradaki adımlar (oturum yarıda kalırsa buradan devam)
1. **Dalga 1 (yaprak modüller)**: appearance+media+animation+background, calendar+clock+timers+units+shelf,
   jump_list+lyrics+diagnostics+uninstall+privacy, shell+tray+visibility+quick_launch+network+power. Her grup bitince
   `cargo test -p koy-core --no-default-features --features <grup>` yeşil → commit.
2. **H1 (merkez iskeleti, tek ajan)**: `hub` feature'ı (settings, dock, display, notch, clipboard, search, rules,
   shortcuts, tasks, weather) birbirine döngüsel bağlı. Önce bütün tipler/imzalar `todo!()` gövdeyle, Settings tam.
   `cargo build --features hub` derlenmeli.
3. **H2 (paralel)**: dock | display+notch | search+clipboard | rules+shortcuts+tasks+weather gövde + testler.
4. **Dalga 3**: input, start_menu, notifications, system_stats, bluetooth, extensions, updates (hub'a bağlı).
5. Feature kapılarını kaldır, `cargo test` tamamı yeşil.
6. **Platform** (`koy-platform`, `windows` crate): Shell ✅ (taskbar + watchdog launcher), sonra diğer servisler.
7. **Arayüz**: karar bekliyor (öneri: Slint, yazılımla çizim). Sonra App katmanı.
| core | shell, tray, visibility, quick_launch, network, power | ✅ | 189 test (ortak dahil). Bekleyen: QuickLaunch `set_error_log` → diagnostics CrashLog'a bağlanacak; `TrayIconVisibilityPolicy` HashMap alıyor → Settings `ValueDictionary` gelince uyarla; Dock/Notch/Notifications'a ait testler (EdgeRevealTracker, TrayGlyphs, TrayBalloonNotification) o modüllerle |
| core | appearance, media, animation, background | ✅ | 156 test. Settings gelince: `impl MotionToggles for MotionSettings`, `impl IconOverrideMap for ValueDictionary` (büyük/küçük harf duyarsız); Settings'e bağlı 16 test bekliyor (MotionToggles, AppIconOverride, FontScalePolicy, UserStyleOverrides, MusicCardStyleSetting) |
