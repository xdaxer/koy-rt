use crate::time::TimeSpan;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TrayPopupDismissAction {
    None,

    /// Tepsi açılır penceresi (animasyonla) kapanmalı.
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Phase {
    #[default]
    Idle,
    WaitingForMenu,
    MenuOpen,
}

/// Dock'taki tepsi açılır penceresinin ne zaman kapanacağı (saf mantık; Windows'un tepsi taşma penceresi gibi).
/// Pencere odak kaybıyla değil, bilinçli olaylarla kapanır:
///  - pencere ve uygulamanın bağlam menüsü dışına tıklanınca,
///  - bağlam menüsünden seçim yapılınca (menü kapanınca).
/// Uygulamanın menüsü açıkken pencere açık kalır. Win32 sorguları dışarıda yapılır, buraya bayraklar gelir.
#[derive(Debug, Default)]
pub struct TrayPopupDismissPolicy {
    phase: Phase,
    deadline: TimeSpan,
    last_press_inside_popup: bool,
}

impl TrayPopupDismissPolicy {
    /// Sağ tıktan sonra menünün belirmesi için beklenen süre; menü gelmezse (özel pencere açan uygulama) bekleme biter.
    pub const MENU_APPEAR_TIMEOUT: TimeSpan = TimeSpan::from_millis(800);

    pub fn new() -> Self {
        Self::default()
    }

    /// Menü bekleniyor ya da açık: çağıran menü sorgusunu bu sürede yapar (boştayken Win32 sorgusu yok).
    pub fn is_expecting_menu(&self) -> bool {
        self.phase != Phase::Idle
    }

    /// Bir simgeye sağ tıklandı ve uygulamaya iletildi: menü açılmasını beklemeye başla.
    pub fn on_icon_right_clicked(&mut self, now: TimeSpan) {
        self.phase = Phase::WaitingForMenu;
        self.deadline = now + Self::MENU_APPEAR_TIMEOUT;
        self.last_press_inside_popup = false;
    }

    pub fn reset(&mut self) {
        self.phase = Phase::Idle;
        self.last_press_inside_popup = false;
    }

    /// Yoklama adımı. Penceredeyken ya da menü varken kısa aralıklarla çağrılır.
    ///
    /// `pointer_pressed`: bir fare tuşu bu adımda yeni basıldı (basma kenarı). `pointer_inside_popup`: imleç tepsi
    /// penceresinin içinde. `pointer_inside_menu`: imleç görünen bir bağlam menüsünün (alt menüler dahil) içinde.
    /// `menu_visible`: ekranda herhangi bir bağlam menüsü görünüyor.
    pub fn tick(
        &mut self,
        now: TimeSpan,
        pointer_pressed: bool,
        pointer_inside_popup: bool,
        pointer_inside_menu: bool,
        menu_visible: bool,
    ) -> TrayPopupDismissAction {
        if pointer_pressed {
            if !pointer_inside_popup && !pointer_inside_menu {
                self.reset();
                return TrayPopupDismissAction::Close;
            }

            self.last_press_inside_popup = pointer_inside_popup && !pointer_inside_menu;
        }

        match self.phase {
            Phase::WaitingForMenu => {
                if menu_visible {
                    self.phase = Phase::MenuOpen;
                } else if now >= self.deadline {
                    self.phase = Phase::Idle;
                }
            }

            Phase::MenuOpen => {
                if !menu_visible {
                    self.phase = Phase::Idle;
                    // Menü pencereye tıklanarak kapandıysa (başka simge) pencere kalır; seçim yapıldıysa ya da
                    // klavyeyle kapandıysa pencere de kapanır.
                    return if self.last_press_inside_popup { TrayPopupDismissAction::None } else { TrayPopupDismissAction::Close };
                }
            }

            Phase::Idle => {}
        }

        TrayPopupDismissAction::None
    }
}
