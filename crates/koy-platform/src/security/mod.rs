//! Kriptografik rastgelelik: Core'un `random` kaynağını Windows `BCryptGenRandom` ile kurar.

use windows::Win32::Security::Cryptography::{BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom};

fn fill(buf: &mut [u8]) -> bool {
    // SAFETY: algoritma tutamacı yok (sistem tercihli RNG), tampon bizim.
    unsafe { BCryptGenRandom(None, buf, BCRYPT_USE_SYSTEM_PREFERRED_RNG) }.is_ok()
}

/// Uygulama açılışında bir kez çağrılır.
pub fn install_secure_random() {
    koy_core::random::set_secure_source(fill);
}

#[cfg(test)]
mod tests {
    #[test]
    fn bcrypt_fills_buffer() {
        let mut a = [0u8; 32];
        let mut b = [0u8; 32];
        assert!(super::fill(&mut a) && super::fill(&mut b));
        assert_ne!(a, b);
        super::install_secure_random();
        assert!(koy_core::random::has_secure_source());
        assert_eq!(koy_core::random::get_bytes(16).len(), 16);
    }
}
