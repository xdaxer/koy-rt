use crate::search::FileStampCache;
use std::cell::Cell;

#[test]
fn same_stamp_reuses_value() {
    let cache = FileStampCache::<String>::new();
    let loads = Cell::new(0);
    let next = || {
        loads.set(loads.get() + 1);
        format!("v{}", loads.get())
    };

    let first = cache.get_or_load("a", 10, 100, next);
    let second = cache.get_or_load("A", 10, 100, next);

    assert_eq!("v1", first);
    assert_eq!("v1", second);
    assert_eq!(1, loads.get());
}

#[test]
fn changed_write_time_or_size_reloads() {
    for (write_ticks, length) in [(11, 100), (10, 101)] {
        let cache = FileStampCache::<String>::new();
        let loads = Cell::new(0);
        let next = || {
            loads.set(loads.get() + 1);
            format!("v{}", loads.get())
        };
        cache.get_or_load("a", 10, 100, next);

        let value = cache.get_or_load("a", write_ticks, length, next);

        assert_eq!("v2", value, "{write_ticks}, {length}");
    }
}

#[test]
fn removed_entry_is_loaded_again() {
    let cache = FileStampCache::<i32>::new();
    let loads = Cell::new(0);
    let next = || {
        loads.set(loads.get() + 1);
        loads.get()
    };
    cache.get_or_load("a", 1, 1, next);
    cache.remove("a");
    cache.get_or_load("a", 1, 1, next);

    assert_eq!(2, loads.get());
}
