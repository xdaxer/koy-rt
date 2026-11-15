use super::ordered_pinned_apps_service::IReorderablePinnedApps;

/// Görev çubuğuna sabitlenmiş bir kısayolun saf modeli.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PinnedApp {
    pub title: String,
    pub shortcut_path: String,
    pub target_path: Option<String>,
    pub icon_data: Option<Vec<u8>>,
    pub app_user_model_id: Option<String>,
}

impl PinnedApp {
    /// C# `new PinnedApp(title, shortcutPath, targetPath, iconData, appUserModelId)` (son parametre isteğe bağlı: `None`).
    pub fn new(
        title: impl Into<String>,
        shortcut_path: impl Into<String>,
        target_path: Option<String>,
        icon_data: Option<Vec<u8>>,
        app_user_model_id: Option<String>,
    ) -> Self {
        PinnedApp {
            title: title.into(),
            shortcut_path: shortcut_path.into(),
            target_path,
            icon_data,
            app_user_model_id,
        }
    }
}

/// Kullanıcının görev çubuğuna sabitlediği uygulamaları okuma sözleşmesi.
pub trait IPinnedAppsService: Send + Sync {
    fn get_pinned_apps(&self) -> Vec<PinnedApp>;

    /// C#'taki `_pinnedAppsService is IReorderablePinnedApps` tür denetiminin karşılığı: sıralanabilir kaynaklar
    /// `Some(self)` döndürür, öbürleri varsayılan `None`'u bırakır.
    fn as_reorderable(&self) -> Option<&dyn IReorderablePinnedApps> {
        None
    }
}
