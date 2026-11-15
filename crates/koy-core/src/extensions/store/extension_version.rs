//! Karşılığı: `Extensions/Store/ExtensionVersion.cs`.

use std::cmp::Ordering;
use std::fmt;

use crate::extensions::ExtensionManifestValidator;

/// Üç parçalı sayısal sürüm ("1.2.3"). Biçim kuralı manifest doğrulayıcısıyla aynıdır (parça en çok 6 hane, baştaki
/// sıfır yok); bu yüzden iki yazım aynı sürümü anlatmaz. Karşılaştırma sayısaldır: "1.10.0" > "1.9.0".
/// Sıralama alan sırasıyla (Major, Minor, Patch) C# `CompareTo` ile aynıdır.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ExtensionVersion {
    pub major: i32,
    pub minor: i32,
    pub patch: i32,
}

impl ExtensionVersion {
    pub fn new(major: i32, minor: i32, patch: i32) -> Self {
        ExtensionVersion { major, minor, patch }
    }

    pub fn try_parse(text: Option<&str>) -> Option<ExtensionVersion> {
        if !ExtensionManifestValidator::is_valid_version(text) {
            return None;
        }

        // Doğrulayıcı: tam üç parça, her biri en çok 6 rakam (i32'ye sığar).
        let mut parts = text?.split('.').map(|p| p.parse::<i32>().unwrap_or_default());
        Some(ExtensionVersion::new(parts.next()?, parts.next()?, parts.next()?))
    }

    /// `CompareTo`: -1, 0 ya da 1.
    pub fn compare_to(&self, other: &ExtensionVersion) -> i32 {
        match self.cmp(other) {
            Ordering::Less => -1,
            Ordering::Equal => 0,
            Ordering::Greater => 1,
        }
    }

    /// İki metni karşılaştırır. Geçersiz sürüm her zaman geçerliden küçük sayılır; ikisi de geçersizse eşittir.
    pub fn compare(left: Option<&str>, right: Option<&str>) -> i32 {
        let l = Self::try_parse(left);
        let r = Self::try_parse(right);
        match (l, r) {
            (Some(l), Some(r)) => l.compare_to(&r),
            (None, None) => 0,
            (Some(_), None) => 1,
            (None, Some(_)) => -1,
        }
    }

    /// Aday, kurulu sürümden kesin yeni mi. Biri geçersizse false (güncelleme önerilmez).
    pub fn is_newer(candidate: Option<&str>, installed: Option<&str>) -> bool {
        match (Self::try_parse(candidate), Self::try_parse(installed)) {
            (Some(c), Some(i)) => c.compare_to(&i) > 0,
            _ => false,
        }
    }
}

impl fmt::Display for ExtensionVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}
