//! Karşılığı: `Tasks/ITaskService.cs`.

use chrono::{NaiveDate, NaiveTime};

use super::guid::Guid;
use super::task_item::TaskItem;
use crate::event::Event;

/// En çok bu kadar kayıt tutulur; fazlası budanır (önce eski tamamlananlar).
/// C#: `ITaskService.MaxTasks` (arayüz sabiti; Rust'ta trait nesne güvenli kalsın diye serbest sabit).
pub const MAX_TASKS: usize = 200;

/// Odak görev listesi. Her değişiklik anında diske yazılır (tasks.json, ayar klasörü). Servis saf Core'dadır,
/// WinAPI yok. `changed` değişikliği yapan iş parçacığında tetiklenir: arayüzden çağrılırsa UI
/// iş parçacığında gelir (başka iş parçacığından çağrılırsa App Dispatcher'a geçirmeli).
pub trait ITaskService: Send + Sync {
    /// C#: `Changed` (değer yok; `EventArgs.Empty`).
    fn changed(&self) -> &Event<()>;

    /// Bugün görünümü: bugünün görevleri ve önceki günlerden kalan tamamlanmamışlar. Sıra: tamamlanmamışlar
    /// (gün, sonra saatli olanlar saate göre, saatsizler sonda), en altta tamamlananlar. Dünün (ve eski)
    /// tamamlananları gizlenir ama silinmez.
    fn get_today(&self) -> Vec<TaskItem>;

    /// Tek gün sorgusu (gizleme kuralı yok): o güne ait bütün görevler, saate göre.
    fn get_day(&self, day: NaiveDate) -> Vec<TaskItem>;

    /// Hafta/ay görünümü için aralık sorgusu (uçlar dahil), gün sonra saate göre.
    fn get_range(&self, from: NaiveDate, to: NaiveDate) -> Vec<TaskItem>;

    /// Başlık boşsa `None`. Gün verilmezse bugün. (C#: `time = null, day = null` varsayılanları.)
    fn add(&self, title: &str, time: Option<NaiveTime>, day: Option<NaiveDate>) -> Option<TaskItem>;

    /// Başlık, saat ve günü günceller. Boş başlık ya da bilinmeyen id için `false`. `day` `None` ise gün korunur
    /// (C#: `day = null` varsayılanı).
    fn update(&self, id: Guid, title: &str, time: Option<NaiveTime>, day: Option<NaiveDate>) -> bool;

    fn set_done(&self, id: Guid, done: bool) -> bool;

    fn remove(&self, id: Guid) -> bool;
}
