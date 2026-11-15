#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum VisibilityMode {
    /// Her zaman görünür. Dock ekranda yer ayırır.
    #[default]
    Fixed,

    /// Üstüne bir pencere gelince gizlenir, fare gelince çıkar.
    Smart,

    /// Fare ekran kenarına gelene ya da bir medya olayı olana kadar gizlidir.
    Peek,
}
