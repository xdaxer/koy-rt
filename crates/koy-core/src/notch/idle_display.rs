use super::IdleDisplayMode;

/// Çentik boştayken (kapalı, müzik çalmıyor) ne gösterileceğine dair kararlar (`IdleDisplayMode`).
pub struct IdleDisplay;

impl IdleDisplay {
    pub fn show_clock(mode: IdleDisplayMode) -> bool {
        mode != IdleDisplayMode::Nothing
    }

    /// Pil (kutu ya da halka) gösterilsin mi.
    pub fn show_battery(mode: IdleDisplayMode) -> bool {
        !matches!(mode, IdleDisplayMode::ClockOnly | IdleDisplayMode::Nothing)
    }

    /// Pil kutu yerine halka olarak, saat de gün adıyla birlikte çizilsin mi.
    pub fn is_ring(mode: IdleDisplayMode) -> bool {
        mode == IdleDisplayMode::RingClockDay
    }

    /// Wi-Fi / Bluetooth durum simgeleri pil gibi "ek bilgi"dir: "saat, pil ve bağlantılar" ve halka modunda görünür
    /// (halka modunda yalnız Peek'te, bkz. [`IdleDisplay::show_radio_when_collapsed`]). "Yalnız saat" ve "Hiçbiri" seçimi
    /// kullanıcının çentikte başka bir şey istemediği anlamına gelir. Bu değer radyo izleyicisinin açılışta kurulup
    /// kurulmayacağını da belirler.
    pub fn show_radio(mode: IdleDisplayMode) -> bool {
        !matches!(mode, IdleDisplayMode::ClockOnly | IdleDisplayMode::Nothing)
    }

    /// Radyo simgeleri kapalı çentikte de görünsün mü. Halka modunda kapalı çentik yalnız halka ve gün/saatle sade kalır;
    /// simgeler fareyle üstüne gelince (Peek) çıkar.
    pub fn show_radio_when_collapsed(mode: IdleDisplayMode) -> bool {
        Self::show_radio(mode) && !Self::is_ring(mode)
    }

    /// Kapalı klasik çentikte saatin yanında kısa tarih gösterilsin mi ("Saat, pil ve bağlantılar" modunda ve ayar açıksa).
    /// "Yalnız saat", "Hiçbiri" ve zaten gün adı gösteren halka modunda kapalı çentikte ayrıca tarih eklenmez.
    pub fn show_collapsed_date(mode: IdleDisplayMode, clock_show_date: bool) -> bool {
        clock_show_date && mode == IdleDisplayMode::ClockAndBattery
    }

    /// Üzerine gelince görünen ek bilgiler (ikinci saat, halka modunda tarih ve ağ simgeleri):
    /// "saat, pil ve bağlantılar" ve halka modunda çıkar; "Yalnız saat" ve "Hiçbiri"nde kapalıdır.
    pub fn show_peek_details(mode: IdleDisplayMode) -> bool {
        !matches!(mode, IdleDisplayMode::ClockOnly | IdleDisplayMode::Nothing)
    }
}
