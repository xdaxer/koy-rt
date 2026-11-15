/// Paylaşılan tepsi penceresinin kullanıcı sayacı (saf mantık). Bırakma sayıdan fazla olursa eksiye düşmez.
#[derive(Debug, Default)]
pub struct TrayHostLease {
    users: i32,
}

impl TrayHostLease {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn users(&self) -> i32 {
        self.users
    }

    pub fn acquire(&mut self) {
        self.users += 1;
    }

    /// Bir kullanıcı bırakır; sayaç sıfıra indiyse (pencere kapatılmalı) true.
    pub fn release(&mut self) -> bool {
        if self.users == 0 {
            return false;
        }

        self.users -= 1;
        self.users == 0
    }

    pub fn reset(&mut self) {
        self.users = 0;
    }
}
