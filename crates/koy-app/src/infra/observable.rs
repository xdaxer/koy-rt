//! Görünüm modeli yardımcıları (CommunityToolkit `ObservableObject` karşılığı). Görünüm modelleri arayüzden
//! bağımsızdır: durumlarını değiştirip `Notifier::notify` çağırır, pencere bağlayıcısı durumu Slint özelliklerine
//! kopyalar. Yalnız arayüz iş parçacığında kullanılır.

use std::cell::RefCell;
use std::rc::Rc;

use koy_core::settings::AppSettings;

/// Özellik değişti bildirimi (PropertyChanged). Abone sırasıyla çağrılır; çağrı sırasında abone eklenebilir.
#[derive(Default)]
pub struct Notifier {
    listeners: RefCell<Vec<(u64, Rc<dyn Fn()>)>>,
    next: std::cell::Cell<u64>,
}

impl Notifier {
    pub fn subscribe(&self, f: impl Fn() + 'static) -> u64 {
        let id = self.next.get() + 1;
        self.next.set(id);
        self.listeners.borrow_mut().push((id, Rc::new(f)));
        id
    }

    pub fn unsubscribe(&self, id: u64) {
        self.listeners.borrow_mut().retain(|(i, _)| *i != id);
    }

    pub fn clear(&self) {
        self.listeners.borrow_mut().clear();
    }

    pub fn notify(&self) {
        let list: Vec<Rc<dyn Fn()>> = self.listeners.borrow().iter().map(|(_, f)| Rc::clone(f)).collect();
        for f in list {
            f();
        }
    }
}

/// Tek parametreli olay (C# `event Action<T>`), arayüz iş parçacığında.
pub struct LocalEvent<T> {
    listeners: RefCell<Vec<(u64, Rc<dyn Fn(&T)>)>>,
    next: std::cell::Cell<u64>,
}

impl<T> Default for LocalEvent<T> {
    fn default() -> Self {
        LocalEvent { listeners: RefCell::new(Vec::new()), next: std::cell::Cell::new(0) }
    }
}

impl<T> LocalEvent<T> {
    pub fn subscribe(&self, f: impl Fn(&T) + 'static) -> u64 {
        let id = self.next.get() + 1;
        self.next.set(id);
        self.listeners.borrow_mut().push((id, Rc::new(f)));
        id
    }

    pub fn unsubscribe(&self, id: u64) {
        self.listeners.borrow_mut().retain(|(i, _)| *i != id);
    }

    pub fn clear(&self) {
        self.listeners.borrow_mut().clear();
    }

    pub fn invoke(&self, arg: &T) {
        let list: Vec<Rc<dyn Fn(&T)>> = self.listeners.borrow().iter().map(|(_, f)| Rc::clone(f)).collect();
        for f in list {
            f(arg);
        }
    }
}

/// Özelliklerin paylaştığı ayar nesnesi (C#'ta aynı `AppSettings` örneği bütün görünüm modellerine verilir ve yerinde
/// güncellenir).
pub type SharedSettings = Rc<RefCell<AppSettings>>;
