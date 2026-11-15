use crate::event::Event;
use crate::shell::WindowRect;

/// Bir ekranın fiziksel ve DIP boyutlarını, DPI ölçeğini ve birincil durumunu temsil eden model.
/// `id` kararlı kimliktir (DisplayConfig monitorDevicePath; çözünürlük ya da sıra değişince değişmez),
/// `name` dostça addır, `device_name` Windows'un geçici aygıt adıdır (örn. DISPLAY2).
#[derive(Debug, Clone, PartialEq)]
pub struct DisplayMonitor {
    pub id: String,
    pub name: String,
    pub bounds: WindowRect,
    pub work_area: WindowRect,
    pub is_primary: bool,
    pub dpi_scale_x: f64,
    pub dpi_scale_y: f64,
    pub handle: isize,
    pub device_name: String,
}

impl DisplayMonitor {
    /// C# kurucusunun zorunlu parametreleri; varsayılanlar: DPI 1.0, `handle` 0, `device_name` "".
    pub fn new(id: impl Into<String>, name: impl Into<String>, bounds: WindowRect, work_area: WindowRect, is_primary: bool) -> Self {
        DisplayMonitor {
            id: id.into(),
            name: name.into(),
            bounds,
            work_area,
            is_primary,
            dpi_scale_x: 1.0,
            dpi_scale_y: 1.0,
            handle: 0,
            device_name: String::new(),
        }
    }

    /// DPI ölçekli kurucu (`DpiScaleX`, `DpiScaleY` verilir).
    pub fn with_dpi(mut self, dpi_scale_x: f64, dpi_scale_y: f64) -> Self {
        self.dpi_scale_x = dpi_scale_x;
        self.dpi_scale_y = dpi_scale_y;
        self
    }

    pub fn with_handle(mut self, handle: isize) -> Self {
        self.handle = handle;
        self
    }

    pub fn with_device_name(mut self, device_name: impl Into<String>) -> Self {
        self.device_name = device_name.into();
        self
    }

    /// Piksel cinsinden çözünürlük genişliği.
    pub fn pixel_width(&self) -> i32 {
        self.bounds.width()
    }

    /// Piksel cinsinden çözünürlük yüksekliği.
    pub fn pixel_height(&self) -> i32 {
        self.bounds.height()
    }

    /// Windows'un "Ekranı tanımla" ile gösterdiği numara (aygıt adının sonundaki sayı: DISPLAY2 → 2).
    /// Aygıt adı yoksa 0.
    pub fn number(&self) -> i32 {
        let chars: Vec<char> = self.device_name.chars().collect();
        let end = chars.len();
        let mut start = end;
        while start > 0 && chars[start - 1].is_ascii_digit() {
            start -= 1;
        }
        if start < end {
            // int.TryParse: taşan sayı 0 döner.
            chars[start..end].iter().collect::<String>().parse::<i32>().unwrap_or(0)
        } else {
            0
        }
    }

    /// DIP cinsinden genişlik.
    pub fn width_dip(&self) -> f64 {
        (self.bounds.right - self.bounds.left) as f64 / self.dpi_scale_x
    }

    /// DIP cinsinden yükseklik.
    pub fn height_dip(&self) -> f64 {
        (self.bounds.bottom - self.bounds.top) as f64 / self.dpi_scale_y
    }

    /// DIP cinsinden sol koordinat.
    pub fn left_dip(&self) -> f64 {
        self.bounds.left as f64 / self.dpi_scale_x
    }

    /// DIP cinsinden üst koordinat.
    pub fn top_dip(&self) -> f64 {
        self.bounds.top as f64 / self.dpi_scale_y
    }
}

/// Çentik veya Dock'un hedef monitör seçim modu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum MonitorTargetMode {
    #[default]
    Primary,
    All,
    Specific,
}

/// Bağlı monitörleri sorgulama ve ekran değişikliklerini izleme sözleşmesi.
/// C# `IDisposable` karşılığı uygulayan tipin `Drop`'udur.
pub trait IMonitorService: Send + Sync {
    /// `event EventHandler MonitorsChanged`.
    fn monitors_changed(&self) -> &Event<()>;

    fn get_monitors(&self) -> Vec<DisplayMonitor>;

    fn get_primary_monitor(&self) -> Option<DisplayMonitor>;

    fn get_monitor(&self, id_or_name: &str) -> Option<DisplayMonitor>;

    fn start(&self);

    fn stop(&self);

    fn notify_display_changed(&self) {}
}
