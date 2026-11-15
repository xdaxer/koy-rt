//! Windows DPAPI (`LOCAL=user`, geçerli kullanıcı kapsamı) ile OAuth belirteçlerini şifreler ve çözer.
//! Windows'un yerleşik WinRT veri koruma sağlayıcısını kullanır.

use koy_core::calendar::ITokenProtector;
use windows::Security::Cryptography::CryptographicBuffer;
use windows::Security::Cryptography::DataProtection::DataProtectionProvider;
use windows::core::{Array, HSTRING};

#[derive(Default)]
pub struct DpapiTokenProtector;

fn run(protect: bool, input: &[u8]) -> windows::core::Result<Vec<u8>> {
    let provider = if protect {
        DataProtectionProvider::CreateOverloadExplicit(&HSTRING::from("LOCAL=user"))?
    } else {
        // Çözmek için tanımlayıcı gerekmez (C#: parametresiz yapıcı).
        DataProtectionProvider::new()?
    };
    let buffer = CryptographicBuffer::CreateFromByteArray(input)?;
    let output = if protect { provider.ProtectAsync(&buffer)?.join()? } else { provider.UnprotectAsync(&buffer)?.join()? };
    let mut bytes = Array::<u8>::new();
    CryptographicBuffer::CopyToByteArray(&output, &mut bytes)?;
    Ok(bytes.to_vec())
}

impl ITokenProtector for DpapiTokenProtector {
    /// C# gibi başarısızlıkta istisna (burada panik); çağıran (`CalendarTokenStore`) yakalar.
    fn protect(&self, plain_bytes: &[u8]) -> Vec<u8> {
        run(true, plain_bytes).unwrap_or_else(|e| panic!("DPAPI protect failed: {e}"))
    }

    fn unprotect(&self, protected_bytes: &[u8]) -> Vec<u8> {
        run(false, protected_bytes).unwrap_or_else(|e| panic!("DPAPI unprotect failed: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_for_current_user() {
        let p = DpapiTokenProtector;
        let secret = b"refresh-token-123";
        let protected = p.protect(secret);
        assert_ne!(protected.as_slice(), secret.as_slice());
        assert_eq!(p.unprotect(&protected), secret);
    }
}
