use super::dock_enums::DockLeftClickAction;

/// Sol tıkın sonucu: hangi pencere ne yapılacak. Saf karar; uygulaması `DockController`'dadır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DockClickKind {
    #[default]
    None,
    Activate,
    Minimize,

    /// Pencere önizlemesini aç. Arayüz açamıyorsa (önizleme kapalı) öne getirme olarak uygulanır.
    ShowThumbnails,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DockClickPlan {
    pub kind: DockClickKind,
    pub handle: isize,
}

impl DockClickPlan {
    pub const fn new(kind: DockClickKind, handle: isize) -> Self {
        DockClickPlan { kind, handle }
    }
}

pub struct DockClickPolicy;

impl DockClickPolicy {
    /// Çalışan bir simgeye sol tık kararı. Çalışmayan simge başlatılır (burada değil).
    ///
    /// * `handles`: Simgenin pencereleri (girdi sırası).
    /// * `foreground`: Şu an ön plandaki pencere (0 = yok).
    /// * `is_active`: Simgenin pencerelerinden biri ön planda mı.
    pub fn decide_left_click(action: DockLeftClickAction, handles: &[isize], foreground: isize, is_active: bool) -> DockClickPlan {
        if handles.is_empty() {
            return DockClickPlan::new(DockClickKind::None, 0);
        }

        match action {
            DockLeftClickAction::BringToFront => DockClickPlan::new(DockClickKind::Activate, handles[0]),

            DockLeftClickAction::CycleWindows => {
                if !is_active {
                    return DockClickPlan::new(DockClickKind::Activate, handles[0]);
                }
                let current = Self::index_of(handles, foreground);
                // Ön plan pencere listede yoksa (isActive ile çelişir) baştan başlanır.
                let next = if current < 0 { 0 } else { (current as usize + 1) % handles.len() };
                DockClickPlan::new(DockClickKind::Activate, handles[next])
            }

            DockLeftClickAction::ShowThumbnails => DockClickPlan::new(DockClickKind::ShowThumbnails, handles[0]),

            // MinimizeIfActive ve tanımsız (dosyadan gelen) değerler: bugünkü dock davranışı.
            DockLeftClickAction::MinimizeIfActive => {
                if is_active {
                    DockClickPlan::new(DockClickKind::Minimize, handles[0])
                } else {
                    DockClickPlan::new(DockClickKind::Activate, handles[0])
                }
            }
        }
    }

    fn index_of(handles: &[isize], handle: isize) -> i32 {
        for (i, h) in handles.iter().enumerate() {
            if *h == handle {
                return i as i32;
            }
        }
        -1
    }
}
