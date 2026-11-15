use super::{DisplayMonitor, MonitorSettings, NewScreenMode};
use crate::settings::ValueDictionary;
use crate::text;

/// Bir monitör için çözümlenmiş sonuç: çentik ve dock orada açık mı.
#[derive(Debug, Clone, PartialEq)]
pub struct MonitorAssignment {
    /// Şu an bağlı monitör.
    pub monitor: DisplayMonitor,
    /// Bu monitörde çentik açık.
    pub notch_enabled: bool,
    /// Bu monitörde dock açık.
    pub dock_enabled: bool,
    /// Ayar sözlüğünde bu monitör için kayıt var (false: yeni ekran, varsayılan uygulandı).
    pub is_known: bool,
}

impl MonitorAssignment {
    pub fn new(monitor: DisplayMonitor, notch_enabled: bool, dock_enabled: bool, is_known: bool) -> Self {
        MonitorAssignment { monitor, notch_enabled, dock_enabled, is_known }
    }
}

/// "Hangi monitörde ne açık" kararı ve ayar sözlüğü düzenlemeleri. Saf mantık: monitör listesi ve ayarlar girer, karar çıkar.
pub struct MonitorPolicy;

impl MonitorPolicy {
    /// Yeni takılan (kaydı olmayan) ekranın varsayılanı: çentik ve dock AÇIK. Ayarlar ekran başına baştan yapılmaz;
    /// genel ayarlar (dock biçimi, çentik görünürlüğü…) her ekrana aynen uygulanır. Bir ekranı kapatmak isteyen
    /// Ekranlar sayfasından o ekranı seçip kapatır; bu yalnız o ekranın kaydını yazar, öbürlerine dokunmaz.
    pub const DEFAULT_NEW_SCREEN_MODE: NewScreenMode = NewScreenMode::NotchAndDock;

    /// Yeni ekran varsayılanının monitör ayarı karşılığı.
    pub fn default_for(mode: NewScreenMode) -> MonitorSettings {
        match mode {
            NewScreenMode::NotchAndDock => MonitorSettings { notch_enabled: true, dock_enabled: true, name: None },
            NewScreenMode::NotchOnly => MonitorSettings { notch_enabled: true, dock_enabled: false, name: None },
            NewScreenMode::None => MonitorSettings { notch_enabled: false, dock_enabled: false, name: None },
        }
    }

    /// Kayıtlı anahtar bu monitöre ait mi? Kararlı kimlik (`DisplayMonitor::id`) ya da eski sürümlerin
    /// yazdığı Windows aygıt adı (`\\.\DISPLAY2`, `DisplayMonitor::device_name`) eşleşir.
    /// Çözünürlük, konum ya da sıra değişince kimlik değişmez; aygıt adı değişebilir, o yüzden yalnız eski kayıtlar için yedek.
    pub fn key_matches(key: &str, monitor: &DisplayMonitor) -> bool {
        text::eq_ignore_case(key, &monitor.id)
            || (!monitor.device_name.is_empty() && text::eq_ignore_case(key, &monitor.device_name))
    }

    /// Monitör için kayıtlı anahtarı döner; önce tam kimlik, sonra eski aygıt adı. Yoksa `None`.
    pub fn find_key(saved: Option<&ValueDictionary<MonitorSettings>>, monitor: &DisplayMonitor) -> Option<String> {
        let saved = saved?;
        if saved.len() == 0 {
            return None;
        }

        for key in saved.keys() {
            if text::eq_ignore_case(key, &monitor.id) {
                return Some(key.to_string());
            }
        }

        for key in saved.keys() {
            if Self::key_matches(key, monitor) {
                return Some(key.to_string());
            }
        }

        None
    }

    /// Her bağlı monitör için çentik/dock durumunu çözümler. Kaydı olmayan ekrana yeni ekran varsayılanı uygulanır.
    pub fn resolve(
        saved: Option<&ValueDictionary<MonitorSettings>>,
        new_screen_default: NewScreenMode,
        monitors: &[DisplayMonitor],
    ) -> Vec<MonitorAssignment> {
        let mut result = Vec::with_capacity(monitors.len());
        let fallback = Self::default_for(new_screen_default);

        for monitor in monitors {
            match Self::find_key(saved, monitor) {
                Some(key) => {
                    let s = saved.and_then(|d| d.get(&key)).expect("anahtar sözlükte var");
                    result.push(MonitorAssignment::new(monitor.clone(), s.notch_enabled, s.dock_enabled, true));
                }
                None => result.push(MonitorAssignment::new(monitor.clone(), fallback.notch_enabled, fallback.dock_enabled, false)),
            }
        }

        result
    }

    /// Çentiğin açık olduğu monitörler.
    pub fn notch_monitors(assignments: &[MonitorAssignment]) -> Vec<DisplayMonitor> {
        assignments.iter().filter(|a| a.notch_enabled).map(|a| a.monitor.clone()).collect()
    }

    /// Dock'un açık olduğu monitörler.
    pub fn dock_monitors(assignments: &[MonitorAssignment]) -> Vec<DisplayMonitor> {
        assignments.iter().filter(|a| a.dock_enabled).map(|a| a.monitor.clone()).collect()
    }

    /// Bir monitörün ayarını değiştirilmiş yeni sözlük döner (eskisine dokunmaz). Anahtar kararlı kimliğe çevrilir:
    /// eski aygıt adıyla kayıtlı ayar varsa devralınır ve eski anahtar silinir. Bağlı olmayan monitörlerin kayıtları korunur.
    pub fn update(
        saved: Option<&ValueDictionary<MonitorSettings>>,
        new_screen_default: NewScreenMode,
        monitor: &DisplayMonitor,
        change: impl FnOnce(MonitorSettings) -> MonitorSettings,
    ) -> ValueDictionary<MonitorSettings> {
        let mut copy: ValueDictionary<MonitorSettings> = match saved {
            None => ValueDictionary::new(),
            Some(s) => s.clone(),
        };

        let old_key = Self::find_key(Some(&copy), monitor);
        let current = match &old_key {
            Some(k) => copy.get(k).expect("anahtar sözlükte var").clone(),
            None => Self::default_for(new_screen_default),
        };
        if let Some(k) = &old_key {
            copy.remove(k);
        }

        let updated = change(current);
        copy.insert(&monitor.id, MonitorSettings { name: Some(monitor.name.clone()), ..updated });
        copy
    }
}
