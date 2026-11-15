//! C# `event Action<T>` karşılığı: abonelik listesi.
//!
//! `+=` → [`Event::subscribe`] (dönen kimlikle `-=` → [`Event::unsubscribe`]), `?.Invoke(x)` → [`Event::invoke`].
//! İş parçacıkları arasında paylaşılabilir; çağrı sırasında liste kopyalanır, böylece işleyici
//! kendini çıkarabilir ya da yeni abone ekleyebilir (C#'taki delege kopyası davranışı).

use std::sync::{Arc, Mutex};

pub type Handler<T> = Arc<dyn Fn(&T) + Send + Sync>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Subscription(u64);

pub struct Event<T> {
    inner: Mutex<(u64, Vec<(u64, Handler<T>)>)>,
}

impl<T> Default for Event<T> {
    fn default() -> Self {
        Event { inner: Mutex::new((0, Vec::new())) }
    }
}

impl<T> std::fmt::Debug for Event<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Event({} handlers)", self.len())
    }
}

impl<T> Event<T> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn subscribe(&self, handler: impl Fn(&T) + Send + Sync + 'static) -> Subscription {
        let mut g = self.inner.lock().unwrap();
        g.0 += 1;
        let id = g.0;
        g.1.push((id, Arc::new(handler)));
        Subscription(id)
    }

    pub fn unsubscribe(&self, sub: Subscription) {
        self.inner.lock().unwrap().1.retain(|(id, _)| *id != sub.0);
    }

    pub fn clear(&self) {
        self.inner.lock().unwrap().1.clear();
    }

    pub fn len(&self) -> usize {
        self.inner.lock().unwrap().1.len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn invoke(&self, args: &T) {
        let handlers: Vec<Handler<T>> = self.inner.lock().unwrap().1.iter().map(|(_, h)| Arc::clone(h)).collect();
        for h in handlers {
            h(args);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicI32, Ordering};

    #[test]
    fn subscribe_invoke_unsubscribe() {
        let e = Event::<i32>::new();
        let sum = Arc::new(AtomicI32::new(0));
        let s = Arc::clone(&sum);
        let sub = e.subscribe(move |v| {
            s.fetch_add(*v, Ordering::SeqCst);
        });
        e.invoke(&5);
        e.unsubscribe(sub);
        e.invoke(&5);
        assert_eq!(sum.load(Ordering::SeqCst), 5);
    }
}
