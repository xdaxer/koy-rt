//! Kriptografik rastgelelik (C# `RandomNumberGenerator.GetBytes`).
//!
//! Core işletim sistemi API'si çağırmaz; gerçek kaynak (Windows'ta `BCryptGenRandom`) açılışta Platform katmanınca
//! [`set_secure_source`] ile kurulur. Kurulmamışsa (testler, araçlar) yedek karıştırıcı kullanılır: işletim
//! sistemi tohumlu SipHash anahtarları (`RandomState`) + zaman + sayaç, SHA-256 ile. Yedek tahmin edilemez ama
//! standart bir CSPRNG değildir; üretimde Platform kaynağı kurulu olmalıdır.

use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};

/// Tamponu doldurur; başarısızlıkta `false`.
pub type SecureSource = fn(&mut [u8]) -> bool;

static SOURCE: OnceLock<SecureSource> = OnceLock::new();

/// Platform kaynağını kurar (bir kez; sonraki çağrılar yok sayılır).
pub fn set_secure_source(source: SecureSource) {
    let _ = SOURCE.set(source);
}

pub fn has_secure_source() -> bool {
    SOURCE.get().is_some()
}

/// `RandomNumberGenerator.GetBytes(len)`.
pub fn get_bytes(len: usize) -> Vec<u8> {
    let mut out = vec![0u8; len];
    if let Some(source) = SOURCE.get()
        && source(&mut out)
    {
        return out;
    }
    fallback_bytes(len)
}

fn fallback_bytes(len: usize) -> Vec<u8> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let mut out = Vec::with_capacity(len + 32);
    while out.len() < len {
        let mut hasher = Sha256::new();
        for _ in 0..4 {
            let mut h = RandomState::new().build_hasher();
            h.write_u64(COUNTER.fetch_add(1, Ordering::Relaxed));
            hasher.update(h.finish().to_le_bytes());
        }
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or_default();
        hasher.update(nanos.to_le_bytes());
        hasher.update(format!("{:?}", std::thread::current().id()).as_bytes());
        out.extend_from_slice(&hasher.finalize());
    }
    out.truncate(len);
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn produces_requested_length_and_differs() {
        let a = super::get_bytes(32);
        let b = super::get_bytes(32);
        assert_eq!(a.len(), 32);
        assert_ne!(a, b);
    }
}
