//! Bluetooth ses cihazına bağlan / bağlantıyı kes. Windows'ta genel bir API yok; belgelenmiş yol, cihazın ses uç
//! noktasının kernel streaming filtresine `KSPROPSETID_BtAudio` kümesindeki `KSPROPERTY_ONESHOT_RECONNECT` /
//! `KSPROPERTY_ONESHOT_DISCONNECT` özelliğini `IKsControl` ile göndermek. Başarılı dönüş yalnız sürücünün isteği aldığını
//! gösterir; sonuç bağlantı durumundan okunmalıdır (bunu `BluetoothService` yapar).

use koy_core::diagnostics::{CrashError, CrashLog};
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::Media::Audio::{
    DEVICE_STATE, IConnector, IDeviceTopology, IMMDevice, IMMDeviceEnumerator, IPart, MMDeviceEnumerator, eCapture, eRender,
};
use windows::Win32::Media::KernelStreaming::{IKsControl, KSIDENTIFIER};
use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PropVariantToGUID};
use windows::Win32::System::Com::{CLSCTX_ALL, CoCreateInstance, STGM_READ};
use windows::core::{GUID, Interface};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum Operation {
    Reconnect = 0,
    Disconnect = 1,
}

// KSPROPSETID_BtAudio
const BT_AUDIO_SET: GUID = GUID::from_u128(0x7FA06C40_B8F6_4C7E_8556_E8C33A12E54D);
// PKEY_Device_ContainerId
const CONTAINER_ID_KEY: PROPERTYKEY = PROPERTYKEY { fmtid: GUID::from_u128(0x8C7ED206_3F8A_4827_B3AB_AE9E1FAEFC6C), pid: 2 };
const KSPROPERTY_TYPE_GET: u32 = 0x1;

/// KSPROPERTY: Set (GUID), Id, Flags. Üretilen birleşim tipinin yerine aynı bellek düzeni.
#[repr(C)]
struct KsPropertyBlock {
    set: GUID,
    id: u32,
    flags: u32,
}

pub(crate) struct BluetoothAudioKs;

impl BluetoothAudioKs {
    /// Kapsayıcıya (ContainerId) ait ses uç noktalarına isteği gönderir. En az bir uç nokta isteği kabul ettiyse true;
    /// uç nokta bulunamadıysa ya da hiçbiri kabul etmediyse false. Hata fırlatmaz. COM bu iş parçacığında başlatılmış olmalı.
    pub(crate) fn try_send(container_id: &str, operation: Operation) -> bool {
        let Ok(container) = GUID::try_from(container_id.trim_matches(|c| c == '{' || c == '}')) else { return false };
        let mut sent = false;
        // SAFETY: COM arayüzleri Drop ile bırakılır.
        let result = unsafe {
            (|| -> windows::core::Result<()> {
                let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;
                for flow in [eRender, eCapture] {
                    let devices = enumerator.EnumAudioEndpoints(flow, DEVICE_STATE(0xF))?;
                    for i in 0..devices.GetCount()? {
                        let device = devices.Item(i)?;
                        if belongs_to(&device, &container) {
                            sent |= send_to_endpoint(&device, operation);
                        }
                    }
                }
                Ok(())
            })()
        };
        if let Err(e) = result {
            CrashLog::write("BluetoothAudioKs.TrySend", &CrashError::exception("System.Exception", &e.message(), ""));
            return false;
        }
        sent
    }
}

fn belongs_to(device: &IMMDevice, container: &GUID) -> bool {
    // SAFETY: PROPVARIANT temizlenir.
    unsafe {
        let Ok(store) = device.OpenPropertyStore(STGM_READ) else { return false };
        let Ok(mut value) = store.GetValue(&CONTAINER_ID_KEY) else { return false };
        let id = PropVariantToGUID(&value);
        let _ = PropVariantClear(&mut value);
        id.is_ok_and(|g| g == *container)
    }
}

fn send_to_endpoint(device: &IMMDevice, operation: Operation) -> bool {
    // Uç nokta bağlı olmayan cihazda topoloji açılamayabilir; bu uç nokta atlanır.
    // SAFETY: COM çağrıları; özellik bloğu yığında ve çağrı süresince geçerli.
    unsafe {
        (|| -> windows::core::Result<()> {
            let topology: IDeviceTopology = device.Activate(CLSCTX_ALL, None)?;
            let connector: IConnector = topology.GetConnector(0)?;
            let peer: IConnector = connector.GetConnectedTo()?;
            let part: IPart = peer.cast()?;
            let mut raw: *mut core::ffi::c_void = std::ptr::null_mut();
            part.Activate(CLSCTX_ALL.0, &IKsControl::IID, Some(&mut raw))?;
            if raw.is_null() {
                return Err(windows::core::Error::empty());
            }
            let control = IKsControl::from_raw(raw);
            let mut property = KsPropertyBlock { set: BT_AUDIO_SET, id: operation as u32, flags: KSPROPERTY_TYPE_GET };
            let mut returned = 0u32;
            control.KsProperty(
                &mut property as *mut KsPropertyBlock as *mut KSIDENTIFIER,
                std::mem::size_of::<KsPropertyBlock>() as u32,
                std::ptr::null_mut(),
                0,
                &mut returned,
            )?;
            Ok(())
        })()
        .is_ok()
    }
}
