//! Ana sayfadaki hızlı başlat şeridi: tıklayınca başlat, "+" ile dosya / uygulama seçerek ekle, sağ tıkla kaldır.
//! Liste ve kalıcılık `QuickLaunchService` içinde (en çok 8). Simgeler kabuktan arka planda çözülür, 32 px'e indirilir
//! ve bu modelde önbelleğe alınır. Karşılığı: `Features/Home/QuickLaunchViewModel.cs`.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use koy_core::event::Subscription;
use koy_core::quick_launch::{IQuickLaunchService, MAX_ITEMS, QuickLaunchService};
use koy_platform::shell::IconHelper;

use crate::infra::dispatch;
use crate::infra::file_dialog;
use crate::infra::image_decode;
use crate::infra::observable::Notifier;
use crate::infra::ui_ref::UiRef;
use crate::resources::strings::Strings;

/// Şeritteki bir uygulama / dosya: simge (yoksa baş harfler) ve ad.
#[derive(Clone)]
pub struct QuickLaunchItem {
    pub path: String,
    pub name: String,
    /// Simge yüklenemezse gösterilen iki harf.
    pub initials: String,
    pub icon: Option<slint::Image>,
}

pub struct QuickLaunchViewModel {
    service: Rc<RefCell<QuickLaunchService>>,
    icons: RefCell<HashMap<String, Option<slint::Image>>>,
    items: RefCell<Vec<QuickLaunchItem>>,
    can_add: Cell<bool>,
    pub changed: Notifier,
    sub: Cell<Option<Subscription>>,
    disposed: Cell<bool>,
    /// Seçici açılırken true, kapanınca false: çentik fare ayrılınca kapanmasın diye.
    pub hold_open: RefCell<Option<Box<dyn Fn(bool)>>>,
    /// Seçicinin sahibi olacak pencere (çentik); verilmezse sahipsiz açılır.
    pub resolve_dialog_owner: RefCell<Option<Box<dyn Fn() -> Option<isize>>>>,
}

fn key(path: &str) -> String {
    path.to_lowercase()
}

impl QuickLaunchViewModel {
    pub fn new(service: Rc<RefCell<QuickLaunchService>>) -> Rc<Self> {
        Rc::new(QuickLaunchViewModel {
            service,
            icons: RefCell::new(HashMap::new()),
            items: RefCell::new(Vec::new()),
            can_add: Cell::new(true),
            changed: Notifier::default(),
            sub: Cell::new(None),
            disposed: Cell::new(false),
            hold_open: RefCell::new(None),
            resolve_dialog_owner: RefCell::new(None),
        })
    }

    pub fn items(&self) -> std::cell::Ref<'_, Vec<QuickLaunchItem>> {
        self.items.borrow()
    }

    /// Liste dolu değil: "+" düğmesi görünür.
    pub fn can_add(&self) -> bool {
        self.can_add.get()
    }

    fn is_active(&self) -> bool {
        let s = self.sub.take();
        let r = s.is_some();
        self.sub.set(s);
        r
    }

    pub fn set_active(self: &Rc<Self>, active: bool) {
        if self.disposed.get() || active == self.is_active() {
            return;
        }
        if active {
            let me = UiRef::new(self);
            let sub = self.service.borrow().changed().subscribe(move |_| {
                // Değişiklik arayüz iş parçacığında olur (ekle / kaldır komutları); bir sonraki tura bırakılır ki
                // servis ödünç alınmışken yeniden okunmasın.
                dispatch::post(move || {
                    if let Some(vm) = me.upgrade()
                        && !vm.disposed.get()
                        && vm.is_active()
                    {
                        vm.reload();
                    }
                });
            });
            self.sub.set(Some(sub));
            self.reload();
        } else if let Some(sub) = self.sub.take() {
            self.service.borrow().changed().unsubscribe(sub);
        }
    }

    fn reload(self: &Rc<Self>) {
        let paths: Vec<String> = self.service.borrow().items().to_vec();
        let mut missing = Vec::new();
        let items: Vec<QuickLaunchItem> = paths
            .iter()
            .map(|path| {
                let name = QuickLaunchService::display_name(path);
                let initials: String = if name.chars().count() >= 2 { name.chars().take(2).collect() } else { name.clone() };
                let icon = match self.icons.borrow().get(&key(path)) {
                    Some(cached) => cached.clone(),
                    None => {
                        missing.push(path.clone());
                        None
                    }
                };
                QuickLaunchItem { path: path.clone(), name, initials, icon }
            })
            .collect();
        self.can_add.set(items.len() < MAX_ITEMS);
        *self.items.borrow_mut() = items;
        self.changed.notify();
        if !missing.is_empty() {
            self.load_icons(missing);
        }
    }

    fn load_icons(self: &Rc<Self>, paths: Vec<String>) {
        let me = UiRef::new(self);
        std::thread::spawn(move || {
            for path in paths {
                // Simge baytları ve 32 px çözüm arka planda; slint resmi arayüz iş parçacığında kurulur.
                let bgra = IconHelper::get_icon_bytes(&path).and_then(|b| image_decode::decode(&b, Some(image_decode::pixels_for(32.0))));
                dispatch::post(move || {
                    let Some(vm) = me.upgrade() else { return };
                    if vm.disposed.get() {
                        return;
                    }
                    let image = bgra.map(|b| b.to_image());
                    vm.icons.borrow_mut().insert(key(&path), image.clone());
                    for item in vm.items.borrow_mut().iter_mut().filter(|i| i.path.eq_ignore_ascii_case(&path)) {
                        item.icon = image.clone();
                    }
                    vm.changed.notify();
                });
            }
        });
    }

    pub fn launch(&self, index: usize) {
        let path = self.items.borrow().get(index).map(|i| i.path.clone());
        if let Some(p) = path {
            self.service.borrow().launch(&p);
        }
    }

    pub fn remove(&self, index: usize) {
        let path = self.items.borrow().get(index).map(|i| i.path.clone());
        if let Some(p) = path {
            self.service.borrow_mut().remove(&p);
            self.icons.borrow_mut().remove(&key(&p));
        }
    }

    fn hold(&self, on: bool) {
        if let Some(h) = self.hold_open.borrow().as_ref() {
            h(on);
        }
    }

    fn owner(&self) -> Option<isize> {
        self.resolve_dialog_owner.borrow().as_ref().and_then(|f| f())
    }

    pub fn add(&self) {
        self.hold(true);
        let path = file_dialog::pick_file(self.owner(), &Strings::get("QuickLaunchPickTitle"), &Strings::get("QuickLaunchPickFilter"), false);
        if let Some(p) = path.filter(|p| !p.trim().is_empty()) {
            self.service.borrow_mut().add(&p);
        }
        self.hold(false);
    }

    pub fn add_folder(&self) {
        self.hold(true);
        let path = file_dialog::pick_folder(self.owner(), &Strings::get("QuickLaunchPickFolderTitle"));
        if let Some(p) = path.filter(|p| !p.trim().is_empty()) {
            self.service.borrow_mut().add(&p);
        }
        self.hold(false);
    }

    pub fn dispose(self: &Rc<Self>) {
        if self.disposed.get() {
            return;
        }
        self.set_active(false);
        self.disposed.set(true);
    }
}
