//! COM yardımcıları: geç bağlamalı `IDispatch` çağrıları (C# `Type.GetTypeFromProgID` + `InvokeMember`).

use std::mem::ManuallyDrop;

use windows::Win32::System::Com::{
    CLSCTX_ALL, CLSIDFromProgID, CoCreateInstance, DISPATCH_FLAGS, DISPATCH_METHOD, DISPATCH_PROPERTYGET, DISPPARAMS, IDispatch,
};
use windows::Win32::System::Variant::{VARENUM, VARIANT, VT_BOOL, VT_BSTR, VT_DISPATCH, VT_EMPTY, VT_I4, VT_NULL, VariantClear};
use windows::core::{BSTR, GUID, HSTRING, Interface, PCWSTR};

/// Çağrı argümanı.
pub enum Arg<'a> {
    Str(&'a str),
    Int(i32),
}

/// Sahipli VARIANT; bırakılınca `VariantClear`.
pub struct Variant(VARIANT);

impl Drop for Variant {
    fn drop(&mut self) {
        // SAFETY: değer bizim; bir kez temizlenir.
        let _ = unsafe { VariantClear(&mut self.0) };
    }
}

impl Variant {
    fn vt(&self) -> VARENUM {
        // SAFETY: vt alanı her zaman geçerlidir.
        unsafe { self.0.Anonymous.Anonymous.vt }
    }

    /// `null`/`DBNull`/boş mu.
    pub fn is_null(&self) -> bool {
        matches!(self.vt(), VT_NULL | VT_EMPTY)
    }

    pub fn as_string(&self) -> Option<String> {
        if self.vt() != VT_BSTR {
            return None;
        }
        // SAFETY: vt BSTR olduğunda bstrVal geçerlidir.
        Some(unsafe { self.0.Anonymous.Anonymous.Anonymous.bstrVal.to_string() })
    }

    pub fn as_bool(&self) -> Option<bool> {
        if self.vt() != VT_BOOL {
            return None;
        }
        // SAFETY: vt BOOL olduğunda boolVal geçerlidir.
        Some(unsafe { self.0.Anonymous.Anonymous.Anonymous.boolVal.as_bool() })
    }

    pub fn as_dispatch(&self) -> Option<Dispatch> {
        if self.vt() != VT_DISPATCH {
            return None;
        }
        // SAFETY: vt DISPATCH olduğunda pdispVal geçerlidir; klonlama başvuru sayısını artırır.
        unsafe { self.0.Anonymous.Anonymous.Anonymous.pdispVal.as_ref().map(|d| Dispatch(d.clone())) }
    }
}

fn to_variant(arg: &Arg<'_>) -> VARIANT {
    let mut v = VARIANT::default();
    // SAFETY: birleşimin yalnız seçilen vt'ye uygun alanı yazılır.
    unsafe {
        let inner = &mut v.Anonymous.Anonymous;
        match arg {
            Arg::Str(s) => {
                inner.vt = VT_BSTR;
                inner.Anonymous.bstrVal = ManuallyDrop::new(BSTR::from(*s));
            }
            Arg::Int(i) => {
                inner.vt = VT_I4;
                inner.Anonymous.lVal = *i;
            }
        }
    }
    v
}

/// Geç bağlamalı COM nesnesi.
pub struct Dispatch(IDispatch);

impl Dispatch {
    /// ProgID'den nesne oluşturur (`Activator.CreateInstance(Type.GetTypeFromProgID(..))`). Kayıtlı değilse `None`.
    pub fn create(prog_id: &str) -> windows::core::Result<Option<Dispatch>> {
        let id = HSTRING::from(prog_id);
        // SAFETY: COM bu iş parçacığında başlatılmış olmalı (çağıran sağlar).
        let clsid = match unsafe { CLSIDFromProgID(&id) } {
            Ok(c) => c,
            Err(_) => return Ok(None),
        };
        let obj: IDispatch = unsafe { CoCreateInstance(&clsid, None, CLSCTX_ALL) }?;
        Ok(Some(Dispatch(obj)))
    }

    fn dispid(&self, name: &str) -> windows::core::Result<i32> {
        let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
        let names = [PCWSTR(wide.as_ptr())];
        let mut id = 0i32;
        // SAFETY: tek ad, tek kimlik çıkışı; LOCALE_USER_DEFAULT = 0x400 yerine değişmez 0 (C# InvariantCulture).
        unsafe { self.0.GetIDsOfNames(&GUID::zeroed(), names.as_ptr(), 1, 0, &mut id) }?;
        Ok(id)
    }

    fn invoke(&self, name: &str, flags: DISPATCH_FLAGS, args: &[Arg<'_>]) -> windows::core::Result<Variant> {
        let id = self.dispid(name)?;
        // IDispatch argümanları ters sırayla alır.
        let mut vargs: Vec<VARIANT> = args.iter().rev().map(to_variant).collect();
        let params = DISPPARAMS {
            rgvarg: if vargs.is_empty() { std::ptr::null_mut() } else { vargs.as_mut_ptr() },
            rgdispidNamedArgs: std::ptr::null_mut(),
            cArgs: vargs.len() as u32,
            cNamedArgs: 0,
        };
        let mut result = VARIANT::default();
        // SAFETY: parametreler çağrı süresince yaşar; sonuç sahipli Variant'a geçer.
        let hr = unsafe { self.0.Invoke(id, &GUID::zeroed(), 0, flags, &params, Some(&mut result), None, None) };
        for v in &mut vargs {
            // SAFETY: argüman VARIANT'ları bizim.
            let _ = unsafe { VariantClear(v) };
        }
        hr.map(|_| Variant(result))
    }

    pub fn call(&self, method: &str, args: &[Arg<'_>]) -> windows::core::Result<Variant> {
        self.invoke(method, DISPATCH_METHOD, args)
    }

    pub fn get(&self, property: &str) -> windows::core::Result<Variant> {
        self.invoke(property, DISPATCH_PROPERTYGET, &[])
    }

    pub fn get_indexed(&self, property: &str, args: &[Arg<'_>]) -> windows::core::Result<Variant> {
        self.invoke(property, DISPATCH_PROPERTYGET, args)
    }

    pub fn raw(&self) -> &IDispatch {
        &self.0
    }

    pub fn cast<T: Interface>(&self) -> windows::core::Result<T> {
        self.0.cast()
    }
}
