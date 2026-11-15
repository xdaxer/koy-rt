/// Kapak baytlarının hafif parmak izi (uzunluk + ilk/son 64 baytın özeti). Aynı kapak için paleti yeniden
/// hesaplamamak içindir; kriptografik değildir, resmin tamamını taramaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CoverFingerprint {
    pub length: i32,
    pub hash: u64,
}

const EDGE: usize = 64;

impl CoverFingerprint {
    pub fn of(bytes: &[u8]) -> CoverFingerprint {
        // FNV-1a, ilk ve son Edge bayt üstünde.
        let mut hash: u64 = 14695981039346656037;
        let head = &bytes[..EDGE.min(bytes.len())];
        let tail: &[u8] = if bytes.len() > EDGE { &bytes[EDGE.max(bytes.len() - EDGE)..] } else { &[] };
        for &b in head {
            hash = (hash ^ b as u64).wrapping_mul(1099511628211);
        }
        for &b in tail {
            hash = (hash ^ b as u64).wrapping_mul(1099511628211);
        }
        CoverFingerprint { length: bytes.len() as i32, hash }
    }
}
