//! Karşılığı: `koy/src/Core/Settings/SettingsKeyMap.cs`.

use std::collections::HashMap;
use std::sync::OnceLock;

/// v3 (düz) ayar anahtarlarının v4'teki yeri: tek tablo. Yeni bir v3 alanı eklenmediği sürece bu tablo değişmez
/// (v3 artık donmuş şemadır); tutarlılığı testle korunur: v3 altın dosyasındaki her anahtar burada ya da
/// `dropped()` içinde olmak zorundadır.
pub struct SettingsKeyMap;

static V3_TO_V4: OnceLock<HashMap<String, (String, String)>> = OnceLock::new();

impl SettingsKeyMap {
    /// Düz anahtar → (bölüm, bölümdeki yeni ad).
    pub fn v3_to_v4() -> &'static HashMap<String, (String, String)> {
        V3_TO_V4.get_or_init(build)
    }

    /// v4'te karşılığı olmayan, bilerek atılan v3 anahtarları (MonitorTarget v2 kalıntısıydı; v2→v3 adımı tüketir).
    pub fn dropped() -> &'static [&'static str] {
        &["MonitorTarget"]
    }

    /// Bölüme taşınmayan, kökte kalan anahtarlar.
    pub fn top_level() -> &'static [&'static str] {
        &["Version"]
    }
}

fn build() -> HashMap<String, (String, String)> {
    let mut map: HashMap<String, (String, String)> = HashMap::new();

    // "DockShowTray" gibi düz anahtar → bölümdeki yeni ad ("ShowTray"); eşlemesi açık yazılmış anahtarlar "eski=yeni".
    let mut add = |section: &str, keys: &[&str]| {
        for key in keys {
            let mut parts = key.split('=');
            let old = parts.next().unwrap_or("");
            let new = parts.next().unwrap_or(old);
            map.insert(old.to_string(), (section.to_string(), new.to_string()));
        }
    };

    add("General", &[
        "Language", "FirstRunCompleted", "LastSeenVersion", "AutoCheckUpdates", "UpdateChannel",
        "TemperatureUnit", "WindSpeedUnit", "NetworkSpeedUnit", "DeveloperMode",
    ]);
    add("Appearance", &[
        "Theme", "AccentColor", "UseWindowsAccent", "CornerRadius", "AnimationSpeed",
        "RespectReduceMotion", "BatterySaverReduceMotion", "Material", "MaterialReadabilityGuard",
    ]);
    add("Dock", &[
        "DockEnabled=Enabled", "HideWindowsTaskbar", "DockVisibility=Visibility", "DockStyle=Style",
        "DockIconAlignment=IconAlignment", "DockProximity=Proximity", "DockHideInFullscreen=HideInFullscreen",
        "DockEdgeReveal=EdgeReveal", "DockHoverMagnify=HoverMagnify", "DockShowStartButton=ShowStartButton",
        "DockStartIcon=StartIcon", "DockStartIconPath=StartIconPath", "DockShowFolders=ShowFolders",
        "DockShowRecents=ShowRecents", "DockShowTray=ShowTray", "DockSmallTray=SmallTray", "DockShowBadges=ShowBadges",
        "DockThumbnailPreview=ThumbnailPreview", "DockThumbnailDelayMs=ThumbnailDelayMs", "DockDodgeDelayMs=DodgeDelayMs",
        "DockIconSize=IconSize", "DockIndicatorStyle=IndicatorStyle", "MiddleClickAction",
    ]);
    add("DockItems", &["DockPinnedOrder=PinnedOrder", "DockFolders=Folders"]);
    add("Notch", &[
        "NotchEnabled=Enabled", "NotchVisibility=Visibility", "NotchStyle=Style", "NotchOpenBehavior=OpenBehavior",
        "IdleDisplayMode=IdleDisplay", "WheelAction", "NotchHideInFullscreen=HideInFullscreen", "NotchEdgeLine=EdgeLine",
        "NotchEdgeLineColor=EdgeLineColor", "NotchEdgeLineThickness=EdgeLineThickness",
        "NotchFollowsMaterial=FollowsMaterial", "ShowVolumeOsd", "ShowBrightnessOsd", "ShowPrivacyIndicator",
        "NotchShowNotifications=ShowNotifications", "NotchNotificationsHideBody=NotificationsHideBody",
        "NotchCollapsedBackground=CollapsedBackground",
        "EnabledPanels", "ControlCenterLayout",
    ]);
    add("Music", &["MusicShowWaveform=ShowWaveform", "MusicCardStyle=CardStyle", "MusicLyricsEnabled=LyricsEnabled"]);
    add("Displays", &["Monitors", "NewScreenDefault"]);
    add("Shortcuts", &[
        "ShortcutToggleNotch=ToggleNotch", "ShortcutOpenShelf=OpenShelf", "ShortcutToggleDock=ToggleDock",
        "ShortcutOpenMenu=OpenMenu", "ShortcutPlayPause=PlayPause", "ShortcutOpenSearch=OpenSearch",
        "ShortcutOpenSearchClipboard=OpenSearchClipboard", "ShortcutVoiceCommand=VoiceCommand",
    ]);
    add("HotCorners", &[
        "HotCornerTopLeft=TopLeft", "HotCornerTopRight=TopRight", "HotCornerBottomLeft=BottomLeft",
        "HotCornerBottomRight=BottomRight", "HotCornerDwellMs=DwellMs", "HotCornerDisableInFullscreen=DisableInFullscreen",
    ]);
    add("Voice", &[
        "VoiceHarness=Harness", "VoiceClaudePermission=ClaudePermission", "VoiceModel=Model",
        "VoiceCustomCommand=CustomCommand", "VoiceWorkingDirectory=WorkingDirectory", "VoiceTimeoutSeconds=TimeoutSeconds",
    ]);
    add("Search", &[
        "SearchRetainPrior=RetainPrior", "SearchHistoryMode=HistoryMode", "SearchDisabledSources=DisabledSources",
        "SearchPinned=Pinned",
    ]);
    add("Focus", &["PomodoroWorkMinutes", "PomodoroShortBreakMinutes", "PomodoroLongBreakMinutes"]);
    add("Weather", &[
        "WeatherRefreshInterval=RefreshInterval", "WeatherLatitude=Latitude", "WeatherLongitude=Longitude",
        "WeatherCity=City", "WeatherLocationSource=LocationSource", "WeatherUseLocation=UseLocation",
        "WeatherLocationUpdatedUnix=LocationUpdatedUnix",
    ]);
    add("Clipboard", &["ClipboardHistoryEnabled=HistoryEnabled"]);
    add("QuickLaunch", &["QuickLaunchItems=Items"]);

    map
}
