//! OLE sürükle-bırak: pencereye dosya / metin bırakma (WPF `AllowDrop` + `Drop`) ve dosya ya da metni pencereden
//! başka uygulamaya sürükleme (`DragDrop.DoDragDrop`). Slint dış sürükle-bırakı açmaz; pencerenin HWND'sine kendi
//! `IDropTarget`'ımız kaydedilir (winit'in kendi hedefi önce kaldırılır). Yalnız arayüz iş parçacığında (STA).

use std::cell::RefCell;
use std::rc::Rc;

use windows::Win32::Foundation::{DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DRAGDROP_S_USEDEFAULTCURSORS, HGLOBAL, HWND, POINTL, S_OK};
use windows::Win32::System::Com::{DVASPECT_CONTENT, FORMATETC, IDataObject, STGMEDIUM, STGMEDIUM_0, TYMED_HGLOBAL};
use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::System::Ole::{
    CF_HDROP, CF_UNICODETEXT, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE, DoDragDrop, IDropSource, IDropSource_Impl, IDropTarget,
    IDropTarget_Impl, OleInitialize, RegisterDragDrop, ReleaseStgMedium, RevokeDragDrop,
};
use windows::Win32::System::SystemServices::{MK_LBUTTON, MODIFIERKEYS_FLAGS};
use windows::Win32::UI::Shell::{DragQueryFileW, HDROP, ILCreateFromPathW, ILFree, SHCreateDataObject};
use windows::core::{BOOL, HRESULT, HSTRING, Ref, implement};

/// Bırakılan veri: dosya yolları ya da metin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DropData {
    Files(Vec<String>),
    Text(String),
}

type DropHandler = Rc<dyn Fn(DropData)>;

#[implement(IDropTarget)]
struct DropTarget {
    on_drop: DropHandler,
    // Sürükleme sürerken veri kabul edilebilir mi (DragEnter'da belirlenir).
    accepts: RefCell<bool>,
}

fn format(cf: u16) -> FORMATETC {
    FORMATETC { cfFormat: cf, ptd: std::ptr::null_mut(), dwAspect: DVASPECT_CONTENT.0, lindex: -1, tymed: TYMED_HGLOBAL.0 as u32 }
}

fn has_format(data: &IDataObject, cf: u16) -> bool {
    // SAFETY: biçim yapısı yerel; QueryGetData yalnız okur.
    unsafe { data.QueryGetData(&format(cf)) == S_OK }
}

fn read_drop(data: &IDataObject) -> Option<DropData> {
    if has_format(data, CF_HDROP.0) {
        // SAFETY: GetData'nın verdiği ortam ReleaseStgMedium ile bırakılır; HDROP yalnız bu süre içinde okunur.
        unsafe {
            let mut medium = data.GetData(&format(CF_HDROP.0)).ok()?;
            let hdrop = HDROP(medium.u.hGlobal.0);
            let count = DragQueryFileW(hdrop, u32::MAX, None);
            let mut files = Vec::with_capacity(count as usize);
            for i in 0..count {
                let len = DragQueryFileW(hdrop, i, None) as usize;
                let mut buf = vec![0u16; len + 1];
                let n = DragQueryFileW(hdrop, i, Some(&mut buf)) as usize;
                files.push(String::from_utf16_lossy(&buf[..n]));
            }
            ReleaseStgMedium(&mut medium);
            return Some(DropData::Files(files));
        }
    }
    if has_format(data, CF_UNICODETEXT.0) {
        // SAFETY: HGLOBAL kilitlenip boyutu kadar okunur, sonra açılır ve ortam bırakılır.
        unsafe {
            let mut medium = data.GetData(&format(CF_UNICODETEXT.0)).ok()?;
            let h = medium.u.hGlobal;
            let p = GlobalLock(h) as *const u16;
            let text = if p.is_null() {
                String::new()
            } else {
                let max = GlobalSize(h) / 2;
                let slice = std::slice::from_raw_parts(p, max);
                let end = slice.iter().position(|c| *c == 0).unwrap_or(max);
                String::from_utf16_lossy(&slice[..end])
            };
            let _ = GlobalUnlock(h);
            ReleaseStgMedium(&mut medium);
            return Some(DropData::Text(text));
        }
    }
    None
}

impl IDropTarget_Impl for DropTarget_Impl {
    fn DragEnter(&self, data: Ref<IDataObject>, _keys: MODIFIERKEYS_FLAGS, _pt: &POINTL, effect: *mut DROPEFFECT) -> windows::core::Result<()> {
        let ok = data.as_ref().is_some_and(|d| has_format(d, CF_HDROP.0) || has_format(d, CF_UNICODETEXT.0));
        *self.accepts.borrow_mut() = ok;
        // SAFETY: OLE geçerli bir çıkış göstergesi verir.
        unsafe { *effect = if ok { DROPEFFECT_COPY } else { DROPEFFECT_NONE } };
        Ok(())
    }

    fn DragOver(&self, _keys: MODIFIERKEYS_FLAGS, _pt: &POINTL, effect: *mut DROPEFFECT) -> windows::core::Result<()> {
        // SAFETY: OLE geçerli bir çıkış göstergesi verir.
        unsafe { *effect = if *self.accepts.borrow() { DROPEFFECT_COPY } else { DROPEFFECT_NONE } };
        Ok(())
    }

    fn DragLeave(&self) -> windows::core::Result<()> {
        *self.accepts.borrow_mut() = false;
        Ok(())
    }

    fn Drop(&self, data: Ref<IDataObject>, _keys: MODIFIERKEYS_FLAGS, _pt: &POINTL, effect: *mut DROPEFFECT) -> windows::core::Result<()> {
        let dropped = data.as_ref().and_then(read_drop);
        // SAFETY: OLE geçerli bir çıkış göstergesi verir.
        unsafe { *effect = if dropped.is_some() { DROPEFFECT_COPY } else { DROPEFFECT_NONE } };
        if let Some(d) = dropped {
            // İşleyici pencereyi değiştirebilir: OLE çağrısı bittikten sonra çalışsın.
            let handler = Rc::clone(&self.on_drop);
            crate::infra::dispatch::post_local(move || handler(d));
        }
        Ok(())
    }
}

/// Kayıtlı bırakma hedefi; düşünce kaydı siler.
pub struct DropRegistration {
    hwnd: HWND,
}

impl Drop for DropRegistration {
    fn drop(&mut self) {
        // SAFETY: kaydı yalnız bu tür yaptı; pencere yok olduysa çağrı hata döner ve yok sayılır.
        unsafe {
            let _ = RevokeDragDrop(self.hwnd);
        }
    }
}

/// `hwnd` penceresine dosya / metin bırakmayı açar. winit'in kendi hedefi (yalnız dosya) önce kaldırılır.
pub fn register_drop(hwnd: isize, on_drop: impl Fn(DropData) + 'static) -> Option<DropRegistration> {
    let hwnd = HWND(hwnd as *mut _);
    let target: IDropTarget = DropTarget { on_drop: Rc::new(on_drop), accepts: RefCell::new(false) }.into();
    // SAFETY: arayüz iş parçacığı STA'dır; OleInitialize tekrar çağrılırsa S_FALSE döner. Hedef COM sayımıyla yaşar.
    unsafe {
        let _ = OleInitialize(None);
        let _ = RevokeDragDrop(hwnd);
        RegisterDragDrop(hwnd, &target).ok()?;
    }
    Some(DropRegistration { hwnd })
}

#[implement(IDropSource)]
struct DropSource;

impl IDropSource_Impl for DropSource_Impl {
    fn QueryContinueDrag(&self, escape: BOOL, keys: MODIFIERKEYS_FLAGS) -> HRESULT {
        if escape.as_bool() {
            DRAGDROP_S_CANCEL
        } else if keys.0 & MK_LBUTTON.0 == 0 {
            DRAGDROP_S_DROP
        } else {
            S_OK
        }
    }

    fn GiveFeedback(&self, _effect: DROPEFFECT) -> HRESULT {
        DRAGDROP_S_USEDEFAULTCURSORS
    }
}

fn file_data_object(path: &str) -> Option<IDataObject> {
    // SAFETY: PIDL ILCreateFromPathW'den gelir ve veri nesnesi kurulduktan sonra ILFree ile bırakılır.
    unsafe {
        let pidl = ILCreateFromPathW(&HSTRING::from(path));
        if pidl.is_null() {
            return None;
        }
        let result = SHCreateDataObject::<_, IDataObject>(None, Some(&[pidl as *const _]), None);
        ILFree(Some(pidl));
        result.ok()
    }
}

fn text_data_object(text: &str) -> Option<IDataObject> {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: HGLOBAL ayrılıp metin kopyalanır; SetData(fRelease = true) sahipliği veri nesnesine geçirir.
    unsafe {
        let data: IDataObject = SHCreateDataObject(None, None, None).ok()?;
        let h: HGLOBAL = GlobalAlloc(GMEM_MOVEABLE, wide.len() * 2).ok()?;
        let p = GlobalLock(h) as *mut u16;
        if p.is_null() {
            return None;
        }
        std::ptr::copy_nonoverlapping(wide.as_ptr(), p, wide.len());
        let _ = GlobalUnlock(h);
        let medium = STGMEDIUM { tymed: TYMED_HGLOBAL.0 as u32, u: STGMEDIUM_0 { hGlobal: h }, pUnkForRelease: std::mem::ManuallyDrop::new(None) };
        data.SetData(&format(CF_UNICODETEXT.0), &medium, true).ok()?;
        Some(data)
    }
}

/// Bir dosyayı ya da klasörü başka uygulamaya sürükler (kopya). Sürükleme bitene kadar döner (kendi ileti döngüsü).
pub fn drag_file(path: &str) {
    if let Some(data) = file_data_object(path) {
        do_drag(&data);
    }
}

/// Metni başka uygulamaya sürükler (kopya).
pub fn drag_text(text: &str) {
    if let Some(data) = text_data_object(text) {
        do_drag(&data);
    }
}

fn do_drag(data: &IDataObject) {
    let source: IDropSource = DropSource.into();
    let mut effect = DROPEFFECT_NONE;
    // SAFETY: STA arayüz iş parçacığında, geçerli veri nesnesi ve kaynakla çağrılır.
    unsafe {
        let _ = OleInitialize(None);
        let _ = DoDragDrop(data, &source, DROPEFFECT_COPY, &mut effect);
    }
}
