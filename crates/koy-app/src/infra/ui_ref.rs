//! Arayüz iş parçacığına bağlı `Rc` nesnelerine `Send + Sync` jetonla ulaşma.
//!
//! Core/platform olayları `Fn + Send + Sync` işleyici ister; arayüz nesneleri `Rc`'dir. Jeton yalnız bir kimlik taşır,
//! nesne arayüz iş parçacığının `thread_local` kaydında zayıf işaretçi olarak durur. `upgrade` başka iş parçacığında
//! çağrılırsa kayıt boş olduğundan `None` döner: güvenli, `unsafe` yok. Olaylar zaten arayüz iş parçacığında yükselir
//! (pencere yordamı ya da `dispatch::post`); değilse çağıran önce `dispatch::post` ile oraya geçer.

use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::rc::{Rc, Weak};

thread_local! {
    static REGISTRY: RefCell<(u64, HashMap<u64, Box<dyn Any>>)> = RefCell::new((0, HashMap::new()));
}

pub struct UiRef<T: 'static> {
    id: u64,
    _t: PhantomData<fn() -> T>,
}

impl<T: 'static> Clone for UiRef<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T: 'static> Copy for UiRef<T> {}

impl<T: 'static> UiRef<T> {
    /// Nesneyi kaydeder. Kayıt nesne düşünce ilk `upgrade` denemesinde temizlenir.
    pub fn new(rc: &Rc<T>) -> UiRef<T> {
        let weak: Weak<T> = Rc::downgrade(rc);
        REGISTRY.with(|r| {
            let mut r = r.borrow_mut();
            r.0 += 1;
            let id = r.0;
            r.1.insert(id, Box::new(weak));
            UiRef { id, _t: PhantomData }
        })
    }

    /// Yalnız arayüz iş parçacığında ve nesne yaşıyorsa `Some`.
    pub fn upgrade(&self) -> Option<Rc<T>> {
        REGISTRY.with(|r| {
            let mut r = r.borrow_mut();
            let weak = r.1.get(&self.id)?.downcast_ref::<Weak<T>>()?.clone();
            let rc = weak.upgrade();
            if rc.is_none() {
                r.1.remove(&self.id);
            }
            rc
        })
    }
}
