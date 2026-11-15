use super::NotchState;

/// Kapalı ve ipucu (Peek) durumlarında hangi yerleşimin çizileceği. Peek yalnız şarkı bilgisi için tasarlandı;
/// parça yokken fareyle üstüne gelmek boş bir ipucu çizmek yerine boşta yerleşimini (saat, pil, Wi-Fi / Bluetooth)
/// açık tutar. Böylece simgelere tıklanabilir ve ipucu metinleri görünür (simgeler fare çentiğe girer girmez
/// kaybolmaz).
pub struct NotchLayout;

impl NotchLayout {
    /// Sabit pencere tuvalinin genişliği (kural #13). Açık çentik (620) ve kenar payı sığar.
    pub const CANVAS_WIDTH: f64 = 640.0;

    /// Sabit pencere tuvalinin yüksekliği (kural #13). Açık çentik 352 DIP; üstüne titreme (shake) payı kalır.
    pub const CANVAS_HEIGHT: f64 = 364.0;

    /// Açık çentiğin genişliği (H tasarımı; kenar boşlukları dahil).
    pub const EXPANDED_WIDTH: f64 = 620.0;

    /// Açık çentiğin yüksekliği, her sekmede aynı (sabit gövde): üst boşluk 10 + durum satırı 26 + 12 + gövde 236 + 12 +
    /// sekme çubuğu 44 + alt boşluk 12. Gövde sabit olduğu için sekme değişince çentik boyut değiştirmez.
    pub const EXPANDED_HEIGHT: f64 = 352.0;

    /// Çentik kartının genişliği (güncelleme, bildirim). HUD'dan (260×40) ayrıdır; tuvalin içinde kalır.
    pub const CARD_WIDTH: f64 = 380.0;

    /// Çentik kartının yüksekliği.
    pub const CARD_HEIGHT: f64 = 72.0;

    /// Boşta yerleşimi (saat, pil, durum simgeleri ya da müzik çalıyorsa kapak) çizilsin mi.
    pub fn show_collapsed_layout(state: NotchState, has_track: bool) -> bool {
        state == NotchState::Collapsed || (state == NotchState::Peek && !has_track)
    }

    /// Üzerine gelince (Peek) boşta yerleşimine yalnız Peek'e özel ayrıntılar (ikinci saat, halka modunda tarih ve
    /// bağlantı simgeleri) eklensin mi. Parça çalıyorken Peek müzik kartını gösterdiği için false döner.
    pub fn show_peek_details(state: NotchState, has_track: bool) -> bool {
        state == NotchState::Peek && !has_track
    }

    /// Şarkı adı ve sanatçıyı gösteren ipucu yerleşimi çizilsin mi.
    pub fn show_peek_layout(state: NotchState, has_track: bool) -> bool {
        state == NotchState::Peek && has_track
    }
}
