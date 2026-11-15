//! WMI yardımcıları (C# `System.Management` karşılığı; yeni bağımlılık yok, doğrudan WMI COM arayüzleri).
//! Çağıran iş parçacığında COM başlatılmış olmalı (MTA önerilir).

use std::mem::ManuallyDrop;

use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, CoCreateInstance, CoSetProxyBlanket, EOAC_NONE, RPC_C_AUTHN_LEVEL_CALL, RPC_C_IMP_LEVEL_IMPERSONATE,
};
use windows::Win32::System::Rpc::{RPC_C_AUTHN_WINNT, RPC_C_AUTHZ_NONE};
use windows::Win32::System::Variant::{
    VARIANT, VT_BOOL, VT_BSTR, VT_I1, VT_I2, VT_I4, VT_I8, VT_UI1, VT_UI2, VT_UI4, VT_UI8, VariantClear,
};
use windows::Win32::System::Wmi::{
    IEnumWbemClassObject, IWbemClassObject, IWbemLocator, IWbemServices, WBEM_FLAG_FORWARD_ONLY, WBEM_FLAG_RETURN_IMMEDIATELY,
    WBEM_GENERIC_FLAG_TYPE, WbemLocator,
};
use windows::core::{BSTR, HSTRING, PCWSTR};

/// Ad alanına bağlanır (`root\wmi`, `root\cimv2`).
pub fn connect(namespace: &str) -> windows::core::Result<IWbemServices> {
    // SAFETY: COM başlatılmış iş parçacığında; arayüzler Drop ile bırakılır.
    unsafe {
        let locator: IWbemLocator = CoCreateInstance(&WbemLocator, None, CLSCTX_INPROC_SERVER)?;
        let services = locator.ConnectServer(&BSTR::from(namespace), &BSTR::new(), &BSTR::new(), &BSTR::new(), 0, &BSTR::new(), None)?;
        CoSetProxyBlanket(&services, RPC_C_AUTHN_WINNT, RPC_C_AUTHZ_NONE, PCWSTR::null(), RPC_C_AUTHN_LEVEL_CALL, RPC_C_IMP_LEVEL_IMPERSONATE, None, EOAC_NONE)?;
        Ok(services)
    }
}

/// WQL sorgusunun bütün nesneleri.
pub fn query(services: &IWbemServices, wql: &str) -> windows::core::Result<Vec<IWbemClassObject>> {
    // SAFETY: COM çağrıları.
    unsafe {
        let enumerator = services.ExecQuery(
            &BSTR::from("WQL"),
            &BSTR::from(wql),
            WBEM_GENERIC_FLAG_TYPE(WBEM_FLAG_FORWARD_ONLY.0 | WBEM_FLAG_RETURN_IMMEDIATELY.0),
            None,
        )?;
        let mut out = Vec::new();
        loop {
            let mut row: [Option<IWbemClassObject>; 1] = [None];
            let mut returned = 0u32;
            // WBEM_INFINITE = -1.
            let _ = enumerator.Next(-1, &mut row, &mut returned);
            match (returned, row[0].take()) {
                (0, _) | (_, None) => break,
                (_, Some(obj)) => out.push(obj),
            }
        }
        Ok(out)
    }
}

/// Olay sorgusu (`SELECT * FROM ...Event`); `next_event` ile yarı eşzamanlı okunur.
pub fn notification_query(services: &IWbemServices, wql: &str) -> windows::core::Result<IEnumWbemClassObject> {
    // SAFETY: COM çağrısı.
    unsafe {
        services.ExecNotificationQuery(
            &BSTR::from("WQL"),
            &BSTR::from(wql),
            WBEM_GENERIC_FLAG_TYPE(WBEM_FLAG_FORWARD_ONLY.0 | WBEM_FLAG_RETURN_IMMEDIATELY.0),
            None,
        )
    }
}

/// Sonraki olay; `timeout_ms` içinde gelmezse `None`.
pub fn next_event(events: &IEnumWbemClassObject, timeout_ms: i32) -> Option<IWbemClassObject> {
    let mut row: [Option<IWbemClassObject>; 1] = [None];
    let mut returned = 0u32;
    // SAFETY: tek nesnelik dizi.
    let _ = unsafe { events.Next(timeout_ms, &mut row, &mut returned) };
    if returned == 0 { None } else { row[0].take() }
}

/// Sayısal özellik (C# `Convert.ToInt32(mo[name])`).
pub fn get_i64(obj: &IWbemClassObject, name: &str) -> Option<i64> {
    let mut value = VARIANT::default();
    // SAFETY: VARIANT her yolda temizlenir; birleşim alanı vt'ye göre okunur.
    unsafe {
        obj.Get(&HSTRING::from(name), 0, &mut value, None, None).ok()?;
        let inner = &value.Anonymous.Anonymous;
        let v = match inner.vt {
            VT_UI1 => Some(inner.Anonymous.bVal as i64),
            VT_I1 => Some(inner.Anonymous.cVal as i64),
            VT_I2 => Some(inner.Anonymous.iVal as i64),
            VT_UI2 => Some(inner.Anonymous.uiVal as i64),
            VT_I4 => Some(inner.Anonymous.lVal as i64),
            VT_UI4 => Some(inner.Anonymous.ulVal as i64),
            VT_I8 => Some(inner.Anonymous.llVal),
            VT_UI8 => Some(inner.Anonymous.ullVal as i64),
            VT_BOOL => Some(if inner.Anonymous.boolVal.as_bool() { 1 } else { 0 }),
            VT_BSTR => inner.Anonymous.bstrVal.to_string().trim().parse().ok(),
            _ => None,
        };
        let _ = VariantClear(&mut value);
        v
    }
}

/// Dize özellik (`__PATH` gibi).
pub fn get_string(obj: &IWbemClassObject, name: &str) -> Option<String> {
    let mut value = VARIANT::default();
    // SAFETY: VARIANT temizlenir.
    unsafe {
        obj.Get(&HSTRING::from(name), 0, &mut value, None, None).ok()?;
        let inner = &value.Anonymous.Anonymous;
        let s = (inner.vt == VT_BSTR).then(|| inner.Anonymous.bstrVal.to_string());
        let _ = VariantClear(&mut value);
        s
    }
}

/// Yöntem argümanı.
pub enum MethodArg {
    U32(u32),
    U8(u8),
}

/// Örnek yöntemini çağırır (C# `mo.InvokeMethod(name, args)`): sınıfın yöntem imzasından giriş nesnesi üretilir,
/// argümanlar sırayla adlandırılmış parametrelere yazılır.
pub fn invoke_method(services: &IWbemServices, instance: &IWbemClassObject, class: &str, method: &str, args: &[(&str, MethodArg)]) -> windows::core::Result<()> {
    let path = get_string(instance, "__PATH").ok_or_else(windows::core::Error::empty)?;
    // SAFETY: COM çağrıları; giriş VARIANT'ları temizlenir.
    unsafe {
        let mut class_obj: Option<IWbemClassObject> = None;
        services.GetObject(&BSTR::from(class), WBEM_GENERIC_FLAG_TYPE(0), None, Some(&mut class_obj), None)?;
        let class_obj = class_obj.ok_or_else(windows::core::Error::empty)?;
        let mut in_sig: Option<IWbemClassObject> = None;
        class_obj.GetMethod(&HSTRING::from(method), 0, &mut in_sig, std::ptr::null_mut())?;
        let in_params = in_sig.ok_or_else(windows::core::Error::empty)?.SpawnInstance(0)?;
        for (name, arg) in args {
            let mut v = VARIANT::default();
            {
                let inner = &mut v.Anonymous.Anonymous;
                match arg {
                    // WMI uint32/uint8 parametreleri VT_I4 / VT_UI1 olarak kabul eder.
                    MethodArg::U32(x) => {
                        inner.vt = VT_I4;
                        inner.Anonymous.lVal = *x as i32;
                    }
                    MethodArg::U8(x) => {
                        inner.vt = VT_UI1;
                        inner.Anonymous.bVal = *x;
                    }
                }
            }
            let r = in_params.Put(&HSTRING::from(*name), 0, &v, 0);
            let _ = VariantClear(&mut v);
            r?;
        }
        services.ExecMethod(&BSTR::from(path), &BSTR::from(method), WBEM_GENERIC_FLAG_TYPE(0), None, &in_params, None, None)
    }
}

#[allow(dead_code)]
fn _keep(_: ManuallyDrop<()>) {}

#[cfg(test)]
mod tests {
    #[test]
    fn queries_cimv2() {
        let _com = crate::bluetooth::support::ComApartment::enter_mta();
        let services = super::connect(r"root\cimv2").unwrap();
        let rows = super::query(&services, "SELECT NumberOfLogicalProcessors FROM Win32_ComputerSystem").unwrap();
        assert!(rows.iter().any(|r| super::get_i64(r, "NumberOfLogicalProcessors").is_some_and(|n| n > 0)));
    }
}
