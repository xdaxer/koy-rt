use super::dock_proximity_mode::{DockProximityMode, DockStyleMode};

/// Yakınlık modunun dock stiline göre geçerli karşılığı. Windows görev çubuğunun iki hâli vardır (hep görünür /
/// otomatik gizle); tam genişlikte yalnız bunlar sunulur. "Aşağı yapış", "Çekil" ve "Hep havada" yüzen kapsüle özgüdür.
/// Ayar dosyasındaki değer değişmez; eşleme çalışma anında uygulanır, kapsüle dönünce eski seçim geçerli olur.
pub struct DockProximityMapping;

impl DockProximityMapping {
    /// Verilen stilde bu mod seçilebilir mi (ayarlar sayfası yalnız bunları gösterir).
    pub fn is_available(style: DockStyleMode, mode: DockProximityMode) -> bool {
        style != DockStyleMode::FullWidth || matches!(mode, DockProximityMode::ReserveSpace | DockProximityMode::AlwaysHidden)
    }

    /// Stile göre uygulanacak mod. Tam genişlikte: Aşağı yapış → Yer ayır (çubuk hep görünür, yer ayrılır);
    /// Çekil → Hep gizli (otomatik gizle: fare alt kenara gelince çıkar);
    /// Hep havada → Yer ayır (havada yüzmek tam genişlikte anlamsız, çubuk hep görünür). Kapsülde değişmez.
    pub fn effective(style: DockStyleMode, mode: DockProximityMode) -> DockProximityMode {
        if style != DockStyleMode::FullWidth {
            return mode;
        }

        match mode {
            DockProximityMode::DockDown => DockProximityMode::ReserveSpace,
            DockProximityMode::Dodge => DockProximityMode::AlwaysHidden,
            DockProximityMode::AlwaysFloating => DockProximityMode::ReserveSpace,
            _ => mode,
        }
    }
}
