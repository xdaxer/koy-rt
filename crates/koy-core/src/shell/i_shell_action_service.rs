/// Başlat menüsü, sistem tepsisi ve süreç başlatma gibi kabuk eylemleri sözleşmesi.
pub trait IShellActionService: Send + Sync {
    /// Windows Başlat menüsünü açmak için Win tuşu girdisi gönderir.
    fn send_win_key(&self);

    /// Windows Sistem Tepsisi taşma menüsünü açmak için Win+B girdisi gönderir.
    fn toggle_system_tray(&self);

    /// Windows Hızlı Ayarlar panelini (ağ, ses, pil) açmak için Win+A girdisi gönderir.
    fn open_quick_settings(&self);

    /// Windows Bildirim Merkezi ve takvimi açmak için Win+N girdisi gönderir.
    fn open_notification_center(&self);

    /// Belirtilen dosya veya kısayolu çalıştırır. C#'ta başlatılamayınca istisna fırlatır; burada `Err(mesaj)`.
    fn launch(&self, path: &str, arguments: Option<&str>) -> Result<(), String>;
}
