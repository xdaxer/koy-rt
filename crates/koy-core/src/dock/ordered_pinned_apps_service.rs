use std::io;
use std::sync::{Arc, Mutex};

use super::i_pinned_apps_service::{IPinnedAppsService, PinnedApp};
use super::pinned_order::PinnedOrder;
use crate::text;

/// Sabitli uygulamaların sırasını değiştirebilen kaynak (dock'ta sürükleyerek sıralama).
pub trait IReorderablePinnedApps: IPinnedAppsService {
    /// Kimliği verilen sabitli uygulamayı yeni dizine taşır ve sırayı kalıcı yapar.
    fn move_to(&self, id: &str, new_index: i32);
}

fn seq_equal_ignore_case(a: &[String], b: &[String]) -> bool {
    a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| text::eq_ignore_case(x, y))
}

type LoadOrder = Box<dyn Fn() -> Vec<String> + Send + Sync>;
/// Sıra kaydı: `Err` (ayar dosyası yazılamadı) yutulur, sıra bu oturumda bellekte geçerli kalır.
type SaveOrder = Box<dyn Fn(&[String]) -> io::Result<()> + Send + Sync>;
/// Taskband `Favorites` okuyucusu: `Err` (belgesiz kaynak okunamadı) yutulur, klasör sırasına düşülür.
type ReadTaskbandFavorites = Box<dyn Fn() -> io::Result<Option<Vec<u8>>> + Send + Sync>;

/// Klasör sırasındaki sabitli uygulamaları Koy'un kendi kayıtlı sırasına göre dizer.
/// Kayıt boşsa (ilk açılış) Windows'un sırası tohum alınır; sonra Koy'un listesi esastır: yeni sabitlenen sona eklenir,
/// kaldırılan listeden düşer. G/Ç dışarıdan verilir (ayar dosyası, kayıt defteri), böylece mantık testlenir.
pub struct OrderedPinnedAppsService {
    inner: Arc<dyn IPinnedAppsService>,
    load_order: LoadOrder,
    save_order: SaveOrder,
    read_taskband_favorites: Option<ReadTaskbandFavorites>,
    order: Mutex<Option<Vec<String>>>,
}

impl OrderedPinnedAppsService {
    pub fn new(
        inner: Arc<dyn IPinnedAppsService>,
        load_order: impl Fn() -> Vec<String> + Send + Sync + 'static,
        save_order: impl Fn(&[String]) -> io::Result<()> + Send + Sync + 'static,
        read_taskband_favorites: Option<ReadTaskbandFavorites>,
    ) -> Self {
        OrderedPinnedAppsService {
            inner,
            load_order: Box::new(load_order),
            save_order: Box::new(save_order),
            read_taskband_favorites,
            order: Mutex::new(None),
        }
    }

    // Ayar dosyası yazılamazsa sıra bu oturumda bellekte geçerli kalır.
    fn save(&self, order: &[String]) {
        let _ = (self.save_order)(order);
    }
}

impl IPinnedAppsService for OrderedPinnedAppsService {
    fn get_pinned_apps(&self) -> Vec<PinnedApp> {
        let apps = self.inner.get_pinned_apps();
        let ids: Vec<String> = apps.iter().map(|a| PinnedOrder::id_of(&a.shortcut_path)).collect();

        let mut guard = self.order.lock().unwrap();
        let current = guard.get_or_insert_with(|| (self.load_order)()).clone();

        // Klasör okunamadıysa (boş liste) kayıt olduğu gibi kalır; boş sonuçla sıra silinmez.
        if ids.is_empty() {
            return apps;
        }

        let effective: Vec<String> = if current.is_empty() {
            // Belgesiz kaynak okunamazsa klasör sırasına düşülür (None).
            let favorites = self.read_taskband_favorites.as_ref().and_then(|f| f().ok().flatten());
            PinnedOrder::seed_from_taskband(favorites.as_deref(), &ids)
        } else {
            PinnedOrder::reconcile(&current, &ids)
        };

        if !seq_equal_ignore_case(&effective, &current) {
            *guard = Some(effective);
            self.save(guard.as_ref().unwrap());
        }

        PinnedOrder::apply(&apps, guard.as_ref().unwrap())
    }

    fn as_reorderable(&self) -> Option<&dyn IReorderablePinnedApps> {
        Some(self)
    }
}

impl IReorderablePinnedApps for OrderedPinnedAppsService {
    fn move_to(&self, id: &str, new_index: i32) {
        let mut guard = self.order.lock().unwrap();
        let current = guard.get_or_insert_with(|| (self.load_order)()).clone();
        let moved = PinnedOrder::move_item(&current, id, new_index);
        if seq_equal_ignore_case(&moved, &current) {
            return;
        }

        *guard = Some(moved);
        self.save(guard.as_ref().unwrap());
    }
}
