use crate::event::Event;

/// Bir örnekleme anındaki sistem verisi.
///
/// - `cpu_percent`: toplam işlemci kullanımı, 0-100.
/// - `top_process_name`: en çok CPU kullanan sürecin adı (uzantısız); henüz bilinmiyorsa `None`.
/// - `top_process_cpu_percent`: o sürecin toplam işlemci içindeki payı, 0-100.
/// - `network_name`: bağlı ağ adı (INetworkService), yoksa `None`.
/// - `koy_memory_bytes`: Koy'un özel çalışma kümesi (Görev Yöneticisi "Bellek" ile aynı ölçü).
/// - `rates_ready`: CPU yüzdesi, en çok CPU kullanan süreç ve ağ hızı iki örnek arası fark ister. Servis yeni başlayınca
///   yalnız anlık değerler (RAM, Koy belleği, ağ adı) hemen okunur ve bu `false` olur; ilk gerçek örnekle `true` olur.
#[derive(Debug, Clone, PartialEq)]
pub struct SystemSnapshot {
    pub cpu_percent: f64,
    pub top_process_name: Option<String>,
    pub top_process_cpu_percent: f64,
    pub ram_used_bytes: i64,
    pub ram_total_bytes: i64,
    pub ram_percent: f64,
    pub net_down_bytes_per_sec: i64,
    pub net_up_bytes_per_sec: i64,
    pub network_name: Option<String>,
    pub koy_memory_bytes: i64,
    pub rates_ready: bool,
}

impl SystemSnapshot {
    /// C# kurucusu; `RatesReady` varsayılanı `true` (C#: `bool RatesReady = true`).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cpu_percent: f64,
        top_process_name: Option<&str>,
        top_process_cpu_percent: f64,
        ram_used_bytes: i64,
        ram_total_bytes: i64,
        ram_percent: f64,
        net_down_bytes_per_sec: i64,
        net_up_bytes_per_sec: i64,
        network_name: Option<&str>,
        koy_memory_bytes: i64,
    ) -> Self {
        SystemSnapshot {
            cpu_percent,
            top_process_name: top_process_name.map(str::to_owned),
            top_process_cpu_percent,
            ram_used_bytes,
            ram_total_bytes,
            ram_percent,
            net_down_bytes_per_sec,
            net_up_bytes_per_sec,
            network_name: network_name.map(str::to_owned),
            koy_memory_bytes,
            rates_ready: true,
        }
    }

    /// `RatesReady:` adlı bağımsız değişkenli kurucu çağrısı.
    pub fn with_rates_ready(mut self, rates_ready: bool) -> Self {
        self.rates_ready = rates_ready;
        self
    }

    /// C# `SystemSnapshot.Empty` (`RatesReady: false`).
    pub fn empty() -> Self {
        SystemSnapshot::new(0.0, None, 0.0, 0, 0, 0.0, 0, 0, None, 0).with_rates_ready(false)
    }
}

/// Sistem verisi örnekleyicisi. Zamanlayıcı YALNIZ [`ISystemStatsService::start`]'tan sonra çalışır (çentik açıkken),
/// [`ISystemStatsService::stop`] zamanlayıcıyı tamamen durdurur; boşta sıfır iş. Start/Stop idempotenttir. Birden çok
/// çentik kullanacaksa App sayaçlı paylaşım yapar (SharedServices.AcquireSystemStats).
///
/// İPLİK: [`ISystemStatsService::updated`] ARKA PLAN iş parçacığından (zamanlayıcı) gelir; App tarafı Dispatcher'a
/// geçirmeli. [`ISystemStatsService::current`] her iş parçacığından okunabilir.
/// (`IDisposable` karşılığı uygulayan tipin `Drop`'udur.)
pub trait ISystemStatsService: Send + Sync {
    fn is_running(&self) -> bool;

    /// Son örnek (Start'tan önce [`SystemSnapshot::empty`]).
    fn current(&self) -> SystemSnapshot;

    /// Her saniye bir kez (Start sonrası). Arka plan iş parçacığından gelir.
    fn updated(&self) -> &Event<SystemSnapshot>;

    fn start(&self);

    fn stop(&self);
}
