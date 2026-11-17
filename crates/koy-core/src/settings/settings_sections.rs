//! Karşılığı: `koy/src/Core/Settings/SettingsSections.cs`.
//!
//! Ayar bölümleri (settings.json v4). Hepsi değer eşitlikli kayıt: eşitlik (`PartialEq`) ve değer kopyası
//! (`Struct { alan: x, ..eski.clone() }`) ücretsiz gelir. Her alan `section!` makrosunda tanımlanır; C# özellik adı =
//! JSON adı. JSON adları dosyaya yazılır: yayımlandıktan sonra bir özelliğin ADI değiştirilmez (değişirse sürüm
//! artırılır ve göç yazılır).

use super::setting_value::section;
use super::{AppLanguage, UpdateChannel, ValueDictionary, ValueList};
use crate::appearance::{AnimationSpeed, AppTheme, FontScalePolicy, IconPack, MaterialLevel};
use crate::clock::{ClockHourFormat, WeekStartSetting};
use crate::display::{MonitorPolicy, MonitorSettings, NewScreenMode};
use crate::dock::{
    DockBadgeCorner, DockBadgeStyle, DockColorSource, DockEdge, DockFolder, DockGroupingMode, DockIconAlignment,
    DockIndicatorStyle, DockItemEntry, DockLabelMode, DockLayoutLimits, DockLeftClickAction, DockProximityMode,
    DockRevealTrigger, DockStartIcon, DockStyleMode, DockWindowMonitorScope, MiddleClickAction,
};
use crate::input::{CornerAction, CornerActions};
use crate::media::MusicCardStyle;
use crate::notch::{NotchCollapsedBackground, 
    HomeCard, HomeCardLayout, IdleDisplayMode, LiveBackgroundLimits, LiveBackgroundPresets, LiveBackgroundSourceKind,
    LiveBackgroundTarget, NotchEdgeLine, NotchEdgeLineColor, NotchOpenBehavior, NotchOpenPolicy, NotchStyleMode,
    NotchTabBarStyle, NotchVisualizerColorSource, NotchVisualizerStyle, WheelAction,
};
use crate::search::SearchHistoryMode;
use crate::tray::{TrayIconVisibility, TrayIconVisibilityPolicy};
use crate::units::{NetworkSpeedUnit, TemperatureUnit, WindSpeedUnit};
use crate::visibility::VisibilityMode;
use crate::voice::{VoiceClaudePermission, VoiceHarnessType};
use crate::weather::{WeatherLocationSource, WeatherRefreshInterval};

/// `EdgeRevealTracker.DefaultDelay.TotalMilliseconds` (300 ms). Dock modülünün `static readonly TimeSpan`ına
/// bağlanmak yerine değer burada tutulur; ikisi birlikte değişmelidir.
const EDGE_REVEAL_DEFAULT_DELAY_MS: i32 = 300;

section! {
    /// Genel: dil, ilk açılış, sürüm bilgisi ve güncelleme.
    GeneralSettings {
        language: String = AppLanguage::system_default() => "Language",
        /// İlk açılış sihirbazının tamamlanıp tamamlanmadığını belirtir.
        first_run_completed: bool = false => "FirstRunCompleted",
        /// Kullanıcının gördüğü en son Koy sürümü (güncelleme sonrası 'Neler Geldi' ekranı için).
        last_seen_version: String = String::new() => "LastSeenVersion",
        auto_check_updates: bool = true => "AutoCheckUpdates",
        update_channel: UpdateChannel = UpdateChannel::Stable => "UpdateChannel",
        /// Sıcaklık birimi (hava durumu). Değer Core'da dönüştürülür, birim adı App'te yazılır (bkz. UnitConversion).
        temperature_unit: TemperatureUnit = TemperatureUnit::Celsius => "TemperatureUnit",
        /// Rüzgâr hızı birimi (hava durumu).
        wind_speed_unit: WindSpeedUnit = WindSpeedUnit::KilometersPerHour => "WindSpeedUnit",
        /// Ağ hızı birimi: bayt ya da bit.
        network_speed_unit: NetworkSpeedUnit = NetworkSpeedUnit::Bytes => "NetworkSpeedUnit",
        /// Gelişmiş ayarlar sekmesini sol menüde gösterir.
        developer_mode: bool = false => "DeveloperMode",
    }
}

section! {
    /// Görünüm: tema, vurgu, köşe, hareket ve malzeme.
    AppearanceSettings {
        theme: AppTheme = AppTheme::Dark => "Theme",
        accent_color: Option<String> = None => "AccentColor",
        /// Açıksa vurgu rengi Windows vurgu renginden gelir, AccentColor yok sayılır.
        use_windows_accent: bool = false => "UseWindowsAccent",
        /// Koy penceresinin köşe seçimi (bkz. CornerSetting): 6 Keskin, 12 Standart, 17 Yuvarlak. Eski varsayılan 18 Standart sayılır.
        corner_radius: f64 = 12.0 => "CornerRadius",
        /// Arayüz simge kaynağı: yerleşik ya da kullanıcı klasörü (bkz. IconPack).
        icon_pack: IconPack = IconPack::Builtin => "IconPack",
        /// Simge paketi klasörü (IconPack.Folder iken). Bu makineye özgüdür: profil dışa aktarımına yazılmaz.
        icon_pack_folder: String = String::new() => "IconPackFolder",
        /// Uygulama başına simge değişimi (anahtar: kısayol adı, değer: doğrulanmış simge yolu). Bkz. AppIconOverrides.
        app_icon_overrides: ValueDictionary<String> = ValueDictionary::new() => "AppIconOverrides",
        animation_speed: AnimationSpeed = AnimationSpeed::Normal => "AnimationSpeed",
        respect_reduce_motion: bool = true => "RespectReduceMotion",
        battery_saver_reduce_motion: bool = true => "BatterySaverReduceMotion",
        material: MaterialLevel = MaterialLevel::Glass => "Material",
        /// Animasyon başına açma/kapama. Kapalı bir animasyonun son durumu yaysız, anında uygulanır.
        motion: MotionSettings = MotionSettings::default() => "Motion",
        /// Okunabilirlik koruması: açık duvar kağıdında saydam malzeme kendiliğinden daha opak seviyeye çıkar.
        material_readability_guard: bool = true => "MaterialReadabilityGuard",
        /// Arayüz yazı tipi adı. Boşsa ya da yüklü değilse Koy varsayılanı (bkz. FontScalePolicy).
        font_family: String = String::new() => "FontFamily",
        /// Saat ve sayılar için ayrı yazı tipi. Boşsa arayüz yazı tipi kullanılır.
        clock_font_family: String = String::new() => "ClockFontFamily",
        /// Taban yazı boyu (DIP), 10-18 aralığında sınırlanır.
        base_font_size: f64 = FontScalePolicy::DEFAULT_BASE_FONT_SIZE => "BaseFontSize",
        /// Yazı kalınlığı (100-900). Okunurluk için 100'lük adımlarda tutulur.
        font_weight: i32 = FontScalePolicy::DEFAULT_FONT_WEIGHT => "FontWeight",
        /// Koy'un kendi ölçeği (yüzde, 80-150), monitör DPI'sinin üstüne uygulanır.
        ui_scale_percent: i32 = FontScalePolicy::DEFAULT_UI_SCALE => "UiScalePercent",
        /// Açıksa %AppData%\Koy\user-theme.json izinli tema anahtarlarını ezer (bkz. UserStyleParser). Varsayılan kapalı:
        /// dosya olsa bile okunmaz.
        user_style_overrides_enabled: bool = false => "UserStyleOverridesEnabled",
    }
}

section! {
    /// Animasyon başına açma/kapama (issue #79). Azaltılmış hareket ve pil tasarrufu hepsini ezer.
    MotionSettings {
        notch_open_close: bool = true => "NotchOpenClose",
        dock_magnify: bool = true => "DockMagnify",
        folder_panel: bool = true => "FolderPanel",
        osd: bool = true => "Osd",
        badge_bounce: bool = true => "BadgeBounce",
        tab_transitions: bool = true => "TabTransitions",
    }
}

impl crate::appearance::MotionToggles for MotionSettings {
    fn notch_open_close(&self) -> bool {
        self.notch_open_close
    }

    fn dock_magnify(&self) -> bool {
        self.dock_magnify
    }

    fn folder_panel(&self) -> bool {
        self.folder_panel
    }

    fn osd(&self) -> bool {
        self.osd
    }

    fn badge_bounce(&self) -> bool {
        self.badge_bounce
    }

    fn tab_transitions(&self) -> bool {
        self.tab_transitions
    }
}

section! {
    /// Dock ayarları. Dock'un kendi yazdığı veriler (sıra, klasörler) `DockItemsSettings` bölümündedir.
    DockSettings {
        /// Kapalıysa Windows görev çubuğuna hiç dokunulmaz.
        enabled: bool = true => "Enabled",
        hide_windows_taskbar: bool = true => "HideWindowsTaskbar",
        /// Dock'un ekran kenarı. Varsayılan alt (bugünkü davranış). Dikey kenarlarda çizim henüz bağlanmadı (bkz. issue #73).
        edge: DockEdge = DockEdge::Bottom => "Edge",
        /// Kenara paralel uzunluk yüzdesi (%10-%100). Varsayılan %100. Aralık dışı değerler kenetlenir (`DockLayoutLimits`).
        length_percent: i32 = DockLayoutLimits::DEFAULT_LENGTH_PERCENT => "LengthPercent",
        /// Dock'un ekran kenarından uzaklığı (DIP). Varsayılan 0.
        edge_offset_dip: f64 = DockLayoutLimits::DEFAULT_EDGE_OFFSET_DIP => "EdgeOffsetDip",
        /// Dock içi dolgu (DIP). Varsayılan 0.
        inner_padding_dip: f64 = DockLayoutLimits::DEFAULT_INNER_PADDING_DIP => "InnerPaddingDip",
        /// Simgeler arası boşluk (DIP, yuva başına). Varsayılan 2.
        icon_spacing_dip: f64 = DockLayoutLimits::DEFAULT_ICON_SPACING_DIP => "IconSpacingDip",
        /// Eski görünürlük kipi (Fixed / Smart / Peek). Ayarlar'da ARAYÜZÜ YOK, Proximity yerine geçmedi: yakınlık modu
        /// "Yer ayır" dışındayken VisibilityController.IsVisible ile birlikte kararı daraltır ve kenar izlemeyi açar.
        /// Varsayılan Fixed'ta etkisizdir; yalnız ayar dosyası elle düzenlenirse görünür bir fark yaratır. Uyumluluk için silinmez.
        visibility: VisibilityMode = VisibilityMode::Fixed => "Visibility",
        /// Dock yerleşim stili: Yüzen kapsül (FloatingCapsule) veya Tam Genişlik (FullWidth).
        style: DockStyleMode = DockStyleMode::FloatingCapsule => "Style",
        /// Tam genişlik dock'ta ikonların yeri: Left, Center (varsayılan), Right. Kapsülde etkisiz.
        icon_alignment: DockIconAlignment = DockIconAlignment::Center => "IconAlignment",
        /// Pencere yaklaşınca dock davranışı: ReserveSpace (Yer ayır - varsayılan), DockDown (Aşağı yapış), Dodge (Çekil), AlwaysHidden (Hep gizli).
        proximity: DockProximityMode = DockProximityMode::ReserveSpace => "Proximity",
        /// Tam ekran bir uygulama veya oyun devredeyken dock'u otomatik gizle.
        hide_in_fullscreen: bool = true => "HideInFullscreen",
        /// Büyütülmüş pencere dock'u gizlediğinde fare alt kenara (~300 ms, fare tuşu basılı değilken) gelince dock çıkar.
        /// Kapalıysa dock büyütülmüş pencerede gizli kalır. Gerçek tam ekranda hiçbir zaman çıkmaz.
        edge_reveal: bool = true => "EdgeReveal",
        hover_magnify: bool = true => "HoverMagnify",
        show_start_button: bool = true => "ShowStartButton",
        /// Başlat düğmesinin simgesi: Koy (varsayılan), Windows logosu ya da Custom (`StartIconPath`).
        start_icon: DockStartIcon = DockStartIcon::Koy => "StartIcon",
        /// Özel Başlat simgesinin dosya yolu (PNG, JPG, ICO). Boşsa ya da dosya yoksa Koy simgesi gösterilir.
        start_icon_path: String = String::new() => "StartIconPath",
        show_folders: bool = true => "ShowFolders",
        /// Dock'ta son kullanılan dosyaların yığın düğmesi (Windows'un Son Kullanılanlar klasörü, en çok 10 öğe).
        show_recents: bool = true => "ShowRecents",
        /// Dock'ta sistem tepsisi düğmesi. Açıkken Koy uygulamaların tepsi simgelerini kendisi toplar.
        show_tray: bool = true => "ShowTray",
        /// Küçük tepsi: tepsi düğmesi tam boy simge yerine dar bir ok olarak Koy menüsü düğmesinin sağına geçer.
        small_tray: bool = true => "SmallTray",
        show_badges: bool = true => "ShowBadges",
        thumbnail_preview: bool = true => "ThumbnailPreview",
        thumbnail_delay_ms: i32 = 400 => "ThumbnailDelayMs",
        dodge_delay_ms: i32 = 150 => "DodgeDelayMs",
        icon_size: f64 = 28.0 => "IconSize",
        indicator_style: DockIndicatorStyle = DockIndicatorStyle::Dot => "IndicatorStyle",
        /// Gösterge rengi: Windows vurgu rengi (varsayılan), nötr ya da `IndicatorCustomColor`.
        indicator_color: DockColorSource = DockColorSource::Accent => "IndicatorColor",
        /// Özel gösterge rengi (#RRGGBB). Geçersizse vurgu rengi kullanılır.
        indicator_custom_color: String = String::new() => "IndicatorCustomColor",
        /// Nokta kipinde her açık pencere için ayrı bir nokta (en çok üç).
        indicator_one_per_window: bool = false => "IndicatorOnePerWindow",
        /// Rozet rengi (bildirim sayısı vb.).
        badge_color: DockColorSource = DockColorSource::Accent => "BadgeColor",
        /// Özel rozet rengi (#RRGGBB). Geçersizse vurgu rengi kullanılır.
        badge_custom_color: String = String::new() => "BadgeCustomColor",
        badge_corner: DockBadgeCorner = DockBadgeCorner::TopRight => "BadgeCorner",
        badge_style: DockBadgeStyle = DockBadgeStyle::Number => "BadgeStyle",
        /// Gizli dock'un çıkma tetikleyicisi: kenarda beklemek (varsayılan, bugünkü davranış), kenara dokunmak ya da yalnız kısayol.
        reveal_trigger: DockRevealTrigger = DockRevealTrigger::EdgeHold => "RevealTrigger",
        /// Kenarda bekleme süresi (ms, EdgeHold iken). Varsayılan bugünkü kenar gecikmesi (`EdgeRevealTracker.DefaultDelay`).
        reveal_delay_ms: i32 = EDGE_REVEAL_DEFAULT_DELAY_MS => "RevealDelayMs",
        /// Dock gizliyken alt kenarda ince bir çizgi görünsün mü.
        hidden_edge_line: bool = false => "HiddenEdgeLine",
        /// Dock simgesine orta tık: yeni pencere (varsayılan) ya da kapat.
        middle_click_action: MiddleClickAction = MiddleClickAction::NewWindow => "MiddleClickAction",
        /// Aynı uygulamanın pencereleri tek simgede: her zaman (varsayılan, bugünkü davranış), hiç ya da yalnız yer kalmayınca.
        grouping_mode: DockGroupingMode = DockGroupingMode::Always => "GroupingMode",
        /// Dock'ta gösterilen pencereler: tüm monitörler (varsayılan) ya da yalnız dock'un monitörü.
        window_monitor_scope: DockWindowMonitorScope = DockWindowMonitorScope::AllMonitors => "WindowMonitorScope",
        /// Dock simgesine sol tık: etkinse küçült, değilse öne getir (varsayılan, bugünkü davranış) ya da başka bir pencere eylemi.
        left_click_action: DockLeftClickAction = DockLeftClickAction::MinimizeIfActive => "LeftClickAction",
        /// Dock simgelerinin altındaki uygulama adı etiketi: hiç (varsayılan), her zaman ya da üzerine gelince.
        label_mode: DockLabelMode = DockLabelMode::Never => "LabelMode",
        /// Yeni bir tepsi simgesinin varsayılan yeri (uygulamanın kendi kaydı yoksa). Varsayılan taşma menüsü.
        new_tray_icon_visibility: TrayIconVisibility = TrayIconVisibilityPolicy::DEFAULT_NEW_ICON => "NewTrayIconVisibility",
        /// Uygulama başına tepsi simgesi yeri. Anahtar küçük harfli exe adı; kaydı olmayan simge `NewTrayIconVisibility`'i alır.
        tray_app_visibility: ValueDictionary<TrayIconVisibility> = ValueDictionary::new() => "TrayAppVisibility",
    }
}

section! {
    /// Dock'un KENDİ yazdığı veriler (sürükleyerek sıralama, klasörler). Ayrı bölüm olmasının sebebi: bunlar değişince dock
    /// kendini yeniden kurmamalı (`DockSettingsDiff` yalnız `DockSettings`'e bakar).
    DockItemsSettings {
        /// Dock'taki sabitli uygulamaların sırası (kısayol dosya adları, örn. "Obsidian.lnk"). Boşsa ilk açılışta Windows'un
        /// sırası tohum alınır; sonra dock'ta sürükleyerek değiştirilir. Yeni sabitlenen sona eklenir, kaldırılan düşer.
        pinned_order: ValueList<String> = ValueList::new() => "PinnedOrder",
        /// Dock üzerindeki klasör tanımları. Her klasör kısa bir kimlik (Id), görünen ad (Name)
        /// ve kısayol adlarından (.lnk) oluşan uygulama listesi (Items) içerir.
        folders: ValueList<DockFolder> = ValueList::new() => "Folders",
        /// Dock öğe dizisi (ayırıcı, boşluk, saat, çöp kutusu, özel düğme, uygulama, klasör). Yükleme anında en çok 32 kayıt kalır;
        /// kayıtlar `DockItemRules.Resolve` ile doğrulanır. Boşsa dock PinnedOrder'dan türetilir (kullanıcı sırasını
        /// kaybetmez). Henüz hiçbir yerde çizilmez.
        items: ValueList<DockItemEntry> = ValueList::new() => "Items",
    }
}

section! {
    /// Çentik ayarları.
    NotchSettings {
        enabled: bool = true => "Enabled",
        visibility: VisibilityMode = VisibilityMode::Fixed => "Visibility",
        /// Çentik biçimi: Pinned (varsayılan, ekran kenarına bitişik, kulaklı) ya da Floating (kulaksız kapsül, üstten 9 DIP boşluk).
        style: NotchStyleMode = NotchStyleMode::Pinned => "Style",
        /// Çentiğin açılması: üzerine gelince (varsayılan) ya da tıklayınca.
        open_behavior: NotchOpenBehavior = NotchOpenBehavior::Hover => "OpenBehavior",
        /// Üzerine gelince açılma gecikmesi (ms, 0–2000). Tıklayınca açılmada kullanılmaz.
        open_delay_ms: i32 = 0 => "OpenDelayMs",
        /// İmleç çentikten çıkınca kapanma gecikmesi (ms, 0–2000). Varsayılan bugünkü 400 ms.
        close_delay_ms: i32 = NotchOpenPolicy::DEFAULT_CLOSE_DELAY_MS => "CloseDelayMs",
        /// Sekme çubuğu: simge ve ad (varsayılan, bugünkü görünüm), yalnız simge ya da gizli. Varsayılan arayüz bağlanırken
        /// IconOnly'den buna çekildi; aksi halde güncellemeyle herkesin sekme adları kaybolurdu (issue #78).
        tab_bar_style: NotchTabBarStyle = NotchTabBarStyle::IconAndLabel => "TabBarStyle",
        /// Sekmelerin kullanıcı sırası (panel anahtarları). Boşsa ya da eksikse varsayılan sıra kullanılır; görünürlük
        /// yine `EnabledPanels` ile belirlenir. Bkz. `NotchPanels.NormalizeOrder`.
        panel_order: ValueList<String> = ValueList::new() => "PanelOrder",
        /// Ana sayfada gösterilen kartlar ve sırası. Boş liste: hiçbir kart yok. Bkz. `HomeCardLayout.Normalize`.
        home_cards: ValueList<HomeCard> = ValueList::from_items(HomeCardLayout::default().iter().copied()) => "HomeCards",
        /// Çentik boştayken (kapalı, müzik çalmıyor) ne gösterilir.
        idle_display: IdleDisplayMode = IdleDisplayMode::ClockAndBattery => "IdleDisplay",
        /// Çentik üstünde tekerlek: panel değiştir (varsayılan) ya da ses.
        wheel_action: WheelAction = WheelAction::SwitchPanel => "WheelAction",
        /// Tam ekran uygulama ya da oyun varken çentik gizlensin.
        hide_in_fullscreen: bool = true => "HideInFullscreen",
        /// Çentiğin iki yanında ekranın üst kenarı boyunca ince çizgi; çentik bu çizginin içinden açılır.
        edge_line: bool = true => "EdgeLine",
        /// Üst kenar çizgisinin rengi: çentik zemini (varsayılan, siyah) ya da vurgu rengi.
        edge_line_color: NotchEdgeLineColor = NotchEdgeLineColor::NotchSurface => "EdgeLineColor",
        /// Üst kenar çizgisinin kalınlığı (DIP, 1–20; bkz. `NotchEdgeLine.ClampThickness`).
        edge_line_thickness: i32 = NotchEdgeLine::DEFAULT_THICKNESS => "EdgeLineThickness",
        /// Açık çentik ve kartlar malzemeyi izlesin; kapalıysa açık çentik hep opak kalır.
        follows_material: bool = true => "FollowsMaterial",
        /// Ses değişince çentikte 1,5 sn ses göstergesi çıksın.
        show_volume_osd: bool = true => "ShowVolumeOsd",
        /// Parlaklık değişince çentikte 1,5 sn parlaklık göstergesi çıksın.
        show_brightness_osd: bool = true => "ShowBrightnessOsd",
        /// Kamera ve mikrofon kullanımında çentikte gizlilik noktaları görünsün.
        show_privacy_indicator: bool = true => "ShowPrivacyIndicator",
        /// Windows bildirimleri çentikte kart olarak görünsün. Yalnız açıkken ve izin verilmişse bildirim listesi yoklanır
        /// (paketsiz süreçte olay yok); kapalıyken hiçbir zamanlayıcı çalışmaz.
        show_notifications: bool = true => "ShowNotifications",
        /// Bildirim kartında yalnız uygulama adı ve başlık görünsün, gövde gizlensin (gizlilik).
        notifications_hide_body: bool = false => "NotificationsHideBody",
        enabled_panels: ValueList<String> = ValueList::from(["home", "focus", "calendar", "shelf", "clipboard", "system", "controlcenter"]) => "EnabledPanels",
        control_center_layout: ValueList<String> = ValueList::from(["wifi", "bluetooth", "donotdisturb"]) => "ControlCenterLayout",
        /// Boştayken çentikteki saat: biçim (12/24 saat, Auto kültürü izler). Kararlar `ClockFormatPolicy`'de.
        clock_hour_format: ClockHourFormat = ClockHourFormat::Auto => "ClockHourFormat",
        /// Saat saniyeyi göstersin.
        clock_show_seconds: bool = false => "ClockShowSeconds",
        /// Saatin yanında tarih görünsün.
        clock_show_date: bool = false => "ClockShowDate",
        /// Takvimde hafta numarası görünsün.
        clock_show_week_number: bool = false => "ClockShowWeekNumber",
        /// Takvimde haftanın ilk günü (System: kültürün ilk günü).
        clock_first_day_of_week: WeekStartSetting = WeekStartSetting::System => "ClockFirstDayOfWeek",
        /// Boşsa yerleşik tarih biçimi kullanılır; doluysa `ClockFormatPolicy.IsValidCustomDateFormat` geçmelidir.
        clock_custom_date_format: String = String::new() => "ClockCustomDateFormat",
        /// İkinci saatin saat dilimi kimliği (Windows kimliği, örn. "Tokyo Standard Time"). Boşsa ikinci saat kapalı.
        clock_second_time_zone_id: String = String::new() => "ClockSecondTimeZoneId",
        /// Canlı arka plan (issue #67): nereye uygulanacağı. Varsayılan None = özellik kapalı, hiçbir şey yüklenmez ve çizilmez.
        /// Yalnız çentik açıkken oynar, kapanınca durur ve bellek bırakılır (bkz. `LiveBackgroundPlaybackPolicy`).
        live_background_target: LiveBackgroundTarget = LiveBackgroundTarget::None => "LiveBackgroundTarget",
        /// Canlı arka plan kaynağı: yerleşik ambiyans (varsayılan) ya da yerel dosya.
        live_background_source: LiveBackgroundSourceKind = LiveBackgroundSourceKind::BuiltInAmbient => "LiveBackgroundSource",
        /// Yerleşik ambiyans önayar kimliği (bkz. `LiveBackgroundPresets`); bilinmeyen kimlik varsayılana düşer.
        live_background_preset: String = LiveBackgroundPresets::DEFAULT_ID.to_string() => "LiveBackgroundPreset",
        /// Yerel dosya yolu (.gif, .mp4, .webm, .webp; yalnız yerel sürücü). Geçerliliği `LiveBackgroundFileValidator` denetler.
        live_background_file_path: String = String::new() => "LiveBackgroundFilePath",
        /// Okunabilirlik için koyu perdenin yüzdesi (30–90, bkz. `LiveBackgroundLimits.ClampScrimPercent`).
        live_background_scrim_percent: i32 = LiveBackgroundLimits::DEFAULT_SCRIM_PERCENT => "LiveBackgroundScrimPercent",
        /// Kapalı çentiğin arka planı: Varsayılan (#0A0A0B), Albüm rengi (çalarken kapaktan), Ambiyans (seçili önayardan statik renk).
        collapsed_background: NotchCollapsedBackground = NotchCollapsedBackground::Default => "CollapsedBackground",
    }
}

section! {
    /// Müzik kartı.
    MusicSettings {
        /// Müzik çalarken çentikte dalga simgesi görünsün.
        show_waveform: bool = true => "ShowWaveform",
        /// Müzik kartı stili: Neutral (düz), Tinted (albüm rengi, varsayılan), Artwork (kapak arka plan).
        card_style: MusicCardStyle = MusicCardStyle::Tinted => "CardStyle",
        /// Ana sayfadaki müzik kartında eşzamanlı sözler. Varsayılan kapalı: açıkken çalan parçanın (tarayıcı videosu dahil)
        /// adı LRCLIB'e gider, bu yüzden kullanıcı kartındaki düğmeyle ya da ayardan kendisi açar. İstek yalnız Ana
        /// görünürken, parça başına bir kez.
        lyrics_enabled: bool = false => "LyricsEnabled",
        /// Çentik görselleştiricisinin biçimi: çubuklar (varsayılan), dalga ya da noktalar.
        visualizer_style: NotchVisualizerStyle = NotchVisualizerStyle::Bars => "VisualizerStyle",
        /// Görselleştirici rengi: vurgu (varsayılan) ya da albüm kapağı.
        visualizer_color_source: NotchVisualizerColorSource = NotchVisualizerColorSource::Accent => "VisualizerColorSource",
        /// Görselleştiricideki çubuk sayısı (`NotchVisualizer.MinBarCount`–`MaxBarCount`).
        visualizer_bar_count: i32 = 5 => "VisualizerBarCount",
    }
}

section! {
    /// Ekranlar: monitör başına ayar ve yeni ekran varsayılanı.
    DisplaysSettings {
        /// Monitör başına çentik/dock ayarı; anahtar kararlı monitör kimliği. Bağlı olmayan ekranların kaydı da saklanır
        /// (ekran geri takılınca ayarı döner). Sözlük yerinde değiştirilmez, `MonitorPolicy.Update` yenisini üretir.
        monitors: ValueDictionary<MonitorSettings> = ValueDictionary::new() => "Monitors",
        /// Daha önce hiç görülmemiş yeni bir ekran bağlanınca çentik/dock durumu. Varsayılan AÇIK
        /// (`MonitorPolicy.DefaultNewScreenMode`): yeni ekran ayrı kurulum istemez.
        new_screen_default: NewScreenMode = MonitorPolicy::DEFAULT_NEW_SCREEN_MODE => "NewScreenDefault",
    }
}

section! {
    /// Genel kısayollar, "Ctrl+Alt+N" biçiminde metin; boş metin = atanmamış.
    /// Varsayılanlar Windows'un Win+... kısayollarıyla çakışmasın diye Ctrl+Alt+harf (bkz. ShortcutBindings).
    ShortcutsSettings {
        toggle_notch: String = "Ctrl+Alt+N".to_string() => "ToggleNotch",
        open_shelf: String = "Ctrl+Alt+R".to_string() => "OpenShelf",
        toggle_dock: String = "Ctrl+Alt+D".to_string() => "ToggleDock",
        open_menu: String = "Ctrl+Alt+K".to_string() => "OpenMenu",
        play_pause: String = "Ctrl+Alt+Space".to_string() => "PlayPause",
        open_search: String = "Win+Shift+Q".to_string() => "OpenSearch",
        open_search_clipboard: String = "Alt+Shift+F2".to_string() => "OpenSearchClipboard",
        voice_command: String = "Ctrl+Alt+V".to_string() => "VoiceCommand",
        open_koy_window: String = String::new() => "OpenKoyWindow",
    }
}

section! {
    /// Ekran köşeleri (sıcak köşeler). Varsayılan: hiçbir köşeye eylem bağlı değil; hepsi None iken köşe takibi hiç çalışmaz.
    /// Eylem listesi Koy'un var olan yeteneklerine bağlıdır (bkz. CornerActions).
    HotCornersSettings {
        top_left: CornerAction = CornerAction::None => "TopLeft",
        top_right: CornerAction = CornerAction::None => "TopRight",
        bottom_left: CornerAction = CornerAction::None => "BottomLeft",
        bottom_right: CornerAction = CornerAction::None => "BottomRight",
        /// İmleç köşede bu kadar (ms) kesintisiz kalınca eylem çalışır; sınırlar için bkz. `CornerActions.ClampDwell`.
        dwell_ms: i32 = CornerActions::DEFAULT_DWELL_MS => "DwellMs",
        /// Tam ekran uygulama (oyun, sunum, video) öndeyken köşeler tetiklenmez.
        disable_in_fullscreen: bool = true => "DisableInFullscreen",
    }
}

section! {
    /// Sesli komut.
    VoiceSettings {
        harness: VoiceHarnessType = VoiceHarnessType::Claude => "Harness",
        /// Varsayılan salt okunur: dikte yanlış duyulsa da onaysız dosya değişikliği ya da komut çalışmaz.
        /// Otomatik ve Düzenleme kullanıcının açık seçimidir (proje sahibi kararı, 2026-11-11).
        claude_permission: VoiceClaudePermission = VoiceClaudePermission::Default => "ClaudePermission",
        /// Claude'un modeli (takma ad ya da tam ad); "default" = aracın kendi varsayılanı.
        model: String = "default".to_string() => "Model",
        custom_command: String = String::new() => "CustomCommand",
        working_directory: String = String::new() => "WorkingDirectory",
        timeout_seconds: i32 = 60 => "TimeoutSeconds",
    }
}

section! {
    /// Arama paneli.
    SearchSettings {
        /// Arama paneli yeniden açılınca son sorgu geri gelsin.
        retain_prior: bool = true => "RetainPrior",
        /// Varsayılan kapalı: geçmiş düz metin dosyasına yazılır, açmak kullanıcının kararı. Ayarda açık değer varsa korunur.
        history_mode: SearchHistoryMode = SearchHistoryMode::Disabled => "HistoryMode",
        /// Kapatılan arama kaynaklarının kimlikleri.
        disabled_sources: ValueList<String> = ValueList::new() => "DisabledSources",
        /// Arama paneli odak gidince ve sonuç çalışınca kapanmasın.
        pinned: bool = false => "Pinned",
    }
}

section! {
    /// Odak (Pomodoro) süreleri.
    FocusSettings {
        /// Pomodoro çalışma süresi (dakika).
        pomodoro_work_minutes: i32 = 25 => "PomodoroWorkMinutes",
        /// Pomodoro kısa mola süresi (dakika).
        pomodoro_short_break_minutes: i32 = 5 => "PomodoroShortBreakMinutes",
        /// Pomodoro uzun mola süresi (dakika).
        pomodoro_long_break_minutes: i32 = 15 => "PomodoroLongBreakMinutes",
    }
}

section! {
    /// Hava durumu.
    WeatherSettings {
        refresh_interval: WeatherRefreshInterval = WeatherRefreshInterval::Every30Minutes => "RefreshInterval",
        /// Hava durumu koordinatı. Yalnız `LocationSource` "Yok" değilse geçerlidir; eski sürümlerde burada sabit İstanbul
        /// vardı, kaynak "Yok" olduğu için artık okunmaz.
        latitude: f64 = 0.0 => "Latitude",
        longitude: f64 = 0.0 => "Longitude",
        /// Seçilen şehrin adı; cihaz konumunda boş.
        city: String = String::new() => "City",
        /// Koordinatın kaynağı: Yok (hava durumu gösterilmez), Cihaz konumu, Şehir araması.
        location_source: WeatherLocationSource = WeatherLocationSource::None => "LocationSource",
        /// Kullanıcı hava durumu için cihaz konumunu kullanmayı kabul etti (sihirbaz ya da ayarlar).
        use_location: bool = false => "UseLocation",
        /// Koordinatın son güncellendiği an (Unix saniyesi, UTC); günde en fazla bir yenileme için.
        location_updated_unix: i64 = 0 => "LocationUpdatedUnix",
    }
}

section! {
    /// Pano geçmişi.
    ClipboardSettings {
        /// Pano geçmişi (yalnız metin). Kapalıyken pano dinleyicisi hiç kurulmaz.
        history_enabled: bool = true => "HistoryEnabled",
    }
}

section! {
    /// Hızlı başlat.
    QuickLaunchSettings {
        /// Hızlı başlat: uygulama / dosya / klasör yolları, sıralı, en çok 8 (bkz. QuickLaunchService).
        items: ValueList<String> = ValueList::new() => "Items",
    }
}
