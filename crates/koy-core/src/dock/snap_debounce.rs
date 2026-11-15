use crate::time::TimeSpan;

/// Kapsülün "yüzen (60 px) ↔ alta yapışık (46 px)" geçişini gürültüden korur. Alt+Tab sırasında ön plan kısa süre
/// ara pencerelere uğrar (büyütülmüş / değil, YouTube tam ekran denemesi); her uğrakta dock'u 14 px oynatmak "zıplama"
/// gibi görünüyordu. İstenen durum `delay` boyunca değişmeden kalırsa geçerli olur; daha kısa süren değişim
/// yok sayılır. Yoklama yok: durum bekliyorsa çağıran kalan süre sonra tek atımlık bir zamanlayıcıyla yeniden sorar.
#[derive(Debug, Clone)]
pub struct SnapDebounce {
    delay: TimeSpan,
    has_effective: bool,
    pending_desired: bool,
    pending_since: TimeSpan,
    has_pending: bool,
    effective: bool,
}

impl SnapDebounce {
    pub fn new(delay: TimeSpan) -> Self {
        SnapDebounce {
            delay,
            has_effective: false,
            pending_desired: false,
            pending_since: TimeSpan::ZERO,
            has_pending: false,
            effective: false,
        }
    }

    pub fn delay(&self) -> TimeSpan {
        self.delay
    }

    /// Şu an uygulanan durum (true = alta yapışık).
    pub fn effective(&self) -> bool {
        self.effective
    }

    /// İstenen durum uygulananla farklı ve bekliyor.
    pub fn is_pending(&self) -> bool {
        self.has_pending
    }

    /// Bekleyen değişimin geçerli olmasına kalan süre; bekleme yoksa sıfır.
    pub fn remaining(&self, now: TimeSpan) -> TimeSpan {
        if self.has_pending {
            TimeSpan::from_ticks(0.max((self.delay - (now - self.pending_since)).ticks()))
        } else {
            TimeSpan::ZERO
        }
    }

    /// İstenen durumu bildirir, uygulanacak durumu döndürür. İlk çağrı beklemeden benimsenir.
    pub fn update(&mut self, desired: bool, now: TimeSpan) -> bool {
        if !self.has_effective {
            self.has_effective = true;
            self.effective = desired;
            self.has_pending = false;
            return self.effective;
        }

        if desired == self.effective {
            self.has_pending = false;
            return self.effective;
        }

        if !self.has_pending || self.pending_desired != desired {
            self.has_pending = true;
            self.pending_desired = desired;
            self.pending_since = now;
        }

        if now - self.pending_since >= self.delay {
            self.effective = desired;
            self.has_pending = false;
        }

        self.effective
    }

    /// Beklemeyi bırakıp doğrudan geçer (gizlenme, tam genişlik gibi geometrinin zaten değiştiği durumlar).
    pub fn reset(&mut self, state: bool) {
        self.has_effective = true;
        self.effective = state;
        self.has_pending = false;
    }
}
