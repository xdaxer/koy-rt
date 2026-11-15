/// Kütle-yay-sönüm modeli. Sabit alt adımlarla ilerlediği için kare hızından bağımsızdır.
/// Varsayılan değerler hafif bir sekme verir (kritik sönümün biraz altı).
///
/// C#'taki `init`-yalnız özellikler (Stiffness, Mass, RestThreshold) kurucudan sonra değişmez: burada
/// `with_*` kurucu zinciriyle verilir. `Damping` ve `Target` sonradan da ayarlanabilir.
#[derive(Debug, Clone, PartialEq)]
pub struct Spring {
    stiffness: f64,
    damping: f64,
    mass: f64,
    rest_threshold: f64,
    value: f64,
    velocity: f64,
    target: f64,
}

const MAX_STEP: f64 = 1.0 / 240.0;

impl Spring {
    pub fn new(initial: f64) -> Self {
        Spring {
            stiffness: 300.0,
            damping: 26.0,
            mass: 1.0,
            rest_threshold: 0.01,
            value: initial,
            velocity: 0.0,
            target: initial,
        }
    }

    pub fn with_stiffness(mut self, stiffness: f64) -> Self {
        self.stiffness = stiffness;
        self
    }

    pub fn with_damping(mut self, damping: f64) -> Self {
        self.damping = damping;
        self
    }

    pub fn with_mass(mut self, mass: f64) -> Self {
        self.mass = mass;
        self
    }

    pub fn with_rest_threshold(mut self, rest_threshold: f64) -> Self {
        self.rest_threshold = rest_threshold;
        self
    }

    pub fn with_target(mut self, target: f64) -> Self {
        self.target = target;
        self
    }

    pub fn stiffness(&self) -> f64 {
        self.stiffness
    }

    /// Sönüm. Ayarlanabilir: aynı yay yön değiştirirken farklı sönümle sürebilir (çentik kapanışı gibi).
    pub fn damping(&self) -> f64 {
        self.damping
    }

    pub fn set_damping(&mut self, damping: f64) {
        self.damping = damping;
    }

    pub fn mass(&self) -> f64 {
        self.mass
    }

    pub fn rest_threshold(&self) -> f64 {
        self.rest_threshold
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn velocity(&self) -> f64 {
        self.velocity
    }

    pub fn target(&self) -> f64 {
        self.target
    }

    pub fn set_target(&mut self, target: f64) {
        self.target = target;
    }

    /// Sekmesiz (hedefi aşmadan) en hızlı oturan sönüm: 2·√(sertlik·kütle).
    pub fn critical_damping(&self) -> f64 {
        2.0 * (self.stiffness * self.mass).sqrt()
    }

    pub fn is_at_rest(&self) -> bool {
        (self.target - self.value).abs() < self.rest_threshold && self.velocity.abs() < self.rest_threshold
    }

    /// Durumu `dt` saniye ilerletir. Hareket sürüyorsa true döner.
    pub fn step(&mut self, mut dt: f64) -> bool {
        while dt > 0.0 {
            let h = dt.min(MAX_STEP);
            let force = -self.stiffness * (self.value - self.target) - self.damping * self.velocity;
            self.velocity += force / self.mass * h;
            self.value += self.velocity * h;
            dt -= h;
        }

        if !self.is_at_rest() {
            return true;
        }

        self.value = self.target;
        self.velocity = 0.0;
        false
    }

    /// Anlık bir darbe: hıza `velocity` ekler (birim/sn). Yay sonra hedefine geri döner.
    pub fn kick(&mut self, velocity: f64) {
        self.velocity += velocity;
    }

    /// Animasyonsuz olarak değere atlar.
    pub fn snap_to(&mut self, value: f64) {
        self.value = value;
        self.target = value;
        self.velocity = 0.0;
    }
}
