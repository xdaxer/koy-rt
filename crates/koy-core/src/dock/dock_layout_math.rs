use super::dock_edge::DockEdge;
use super::dock_layout_limits::DockLayoutLimits;

/// Ekranda DIP cinsinden dikdörtgen (sol, üst, genişlik, yükseklik).
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct DockRectDip {
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

impl DockRectDip {
    pub const fn new(left: f64, top: f64, width: f64, height: f64) -> Self {
        DockRectDip { left, top, width, height }
    }

    pub fn right(&self) -> f64 {
        self.left + self.width
    }

    pub fn bottom(&self) -> f64 {
        self.top + self.height
    }

    /// Her kenardan `amount` kadar içeri alınmış dikdörtgen. Boyut 0'ın altına inmez; dolgu bir
    /// eksenin yarısını aşarsa o eksen orta çizgiye çöker (iç dikdörtgen dışının dışına çıkmaz).
    pub fn inset(&self, amount: f64) -> DockRectDip {
        let dx = amount.min(self.width.max(0.0) / 2.0);
        let dy = amount.min(self.height.max(0.0) / 2.0);
        DockRectDip::new(self.left + dx, self.top + dy, (self.width - 2.0 * dx).max(0.0), (self.height - 2.0 * dy).max(0.0))
    }
}

/// Dock'un dış dikdörtgeni ve iç (dolgu düşülmüş) içerik alanı.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DockLayoutResult {
    pub outer: DockRectDip,
    pub content: DockRectDip,
}

/// Kenar, uzunluk, kenar ofseti ve iç dolgudan dock dikdörtgenini hesaplayan saf mantık (Core).
/// Dikdörtgen verilen alana göre kurulur. Varsayılan ayarlarla (alt kenar, %100, ofset 0, dolgu 0) sonuç
/// `DockPositionResolver`'ın tam genişlik dock'uyla aynıdır, AMA yalnız alan MONİTÖR SINIRLARI verilirse:
/// resolver bugün monitör sınırlarından hesaplar. Çalışma alanı (WorkArea) dock'un kendi AppBar payı kadar kısalır
/// (Yer ayır), onu verirsek dock kendi ayırdığı şeridin üstüne oturur. Yüzen kapsül stilindeki 10 DIP alt boşluk
/// (`DockPositionResolver::FLOATING_BOTTOM_MARGIN_DIP`) varsayılan ofsette (0) YOKTUR; bağlayan kod ekler.
/// Geçersiz girdiler (NaN, sonsuz, negatif boyut) sessizce güvenli değere iner.
pub struct DockLayoutMath;

impl DockLayoutMath {
    /// `thickness_dip` kenara dik ölçüdür (yatayda yükseklik, dikeyde genişlik). Uzunluk çalışma
    /// alanının kenara paralel ölçüsünün yüzdesidir ve ortalanır. Girdiler kenetlenir (`DockLayoutLimits`).
    pub fn compute(
        edge: DockEdge,
        length_percent: i32,
        edge_offset_dip: f64,
        inner_padding_dip: f64,
        thickness_dip: f64,
        work_area: DockRectDip,
    ) -> DockLayoutResult {
        let work_area = Self::normalize(work_area);
        let length_fraction = DockLayoutLimits::clamp_length_percent(length_percent) as f64 / 100.0;
        let mut offset = DockLayoutLimits::clamp_edge_offset_dip(edge_offset_dip);
        let padding = DockLayoutLimits::clamp_inner_padding_dip(inner_padding_dip);
        let mut thickness = if thickness_dip.is_nan() { 0.0 } else { thickness_dip.max(0.0) };
        // Kalınlık ve ofset çalışma alanının dışına taşamaz (küçük ekranda negatif konum olmasın).
        let across = if matches!(edge, DockEdge::Bottom | DockEdge::Top) { work_area.height } else { work_area.width };
        thickness = thickness.min(across.max(0.0));
        offset = offset.min((across - thickness).max(0.0));

        let outer = if matches!(edge, DockEdge::Bottom | DockEdge::Top) {
            let width = work_area.width * length_fraction;
            let left = work_area.left + (work_area.width - width) / 2.0;
            let top = if edge == DockEdge::Bottom { work_area.bottom() - offset - thickness } else { work_area.top + offset };
            DockRectDip::new(left, top, width, thickness)
        } else {
            let height = work_area.height * length_fraction;
            let top = work_area.top + (work_area.height - height) / 2.0;
            let left = if edge == DockEdge::Left { work_area.left + offset } else { work_area.right() - offset - thickness };
            DockRectDip::new(left, top, thickness, height)
        };

        DockLayoutResult { outer, content: outer.inset(padding) }
    }

    // Sıfır / negatif boyutlu ya da sonlu olmayan alan negatif genişlikli dock üretmesin.
    fn normalize(r: DockRectDip) -> DockRectDip {
        fn finite(v: f64) -> f64 {
            if v.is_finite() { v } else { 0.0 }
        }
        DockRectDip::new(finite(r.left), finite(r.top), finite(r.width).max(0.0), finite(r.height).max(0.0))
    }
}
