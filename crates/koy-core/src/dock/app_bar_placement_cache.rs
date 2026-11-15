use std::collections::HashMap;

use crate::shell::WindowRect;

/// AppBar SETPOS tekrarını eler. Kurallar: ilk kayıtta HER ZAMAN gönderilir; zorlanınca gönderilir; alt kenar Koy'un
/// değilken (görev çubuğu görünür) Gezgin'in durumu değişmiş olabileceğinden gönderilir; yalnız alt kenar Koy'undayken
/// ve istenen alan en son uygulananla aynıysa atlanır. Sahiplik değişince önbellek temizlenir.
#[derive(Debug, Default)]
pub struct AppBarPlacementCache {
    last: HashMap<isize, WindowRect>,
}

impl AppBarPlacementCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn should_send(&self, window: isize, requested: WindowRect, owns_bottom_edge: bool, force: bool) -> bool {
        if force || !owns_bottom_edge {
            return true;
        }

        match self.last.get(&window) {
            Some(last) => *last != requested,
            None => true,
        }
    }

    pub fn record(&mut self, window: isize, requested: WindowRect) {
        self.last.insert(window, requested);
    }

    pub fn forget(&mut self, window: isize) {
        self.last.remove(&window);
    }

    pub fn clear(&mut self) {
        self.last.clear();
    }
}
