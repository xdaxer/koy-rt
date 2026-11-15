//! Atlama listesindeki bir ShellLink akışını ya da kabuk öğesini `JumpListEntry`'ye çevirir. Yalnız `JumpListService`'in
//! ayrı STA iş parçacığında çağrılır; açtığı her COM nesnesi Drop ile serbest kalır.

use koy_core::jump_list::{JumpListEntry, JumpListEntryFactory, JumpListText};
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::System::Com::StructuredStorage::{PropVariantClear, PropVariantToBooleanWithDefault};
use windows::Win32::System::Com::{CLSCTX_INPROC_SERVER, CoCreateInstance, CoTaskMemFree, IPersistStream};
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
use windows::Win32::UI::Shell::{
    IShellItem, IShellLinkW, SHCreateMemStream, SHGetNameFromIDList, SHLoadIndirectString, SIGDN, SIGDN_DESKTOPABSOLUTEPARSING,
    SIGDN_FILESYSPATH, SIGDN_NORMALDISPLAY, SIGDN_URL, ShellLink,
};
use windows::core::{GUID, HSTRING, Interface};

use crate::search::win_env::expand_environment_variables;
use crate::shell::read_string_property;

const TEXT_BUFFER: usize = 1024;

// PKEY_Title
const TITLE_KEY: PROPERTYKEY = PROPERTYKEY { fmtid: GUID::from_u128(0xF29F85E0_4FF9_1068_AB91_08002B27B3D9), pid: 2 };
// PKEY_ItemNameDisplay (ör. Spotify gibi PKEY_Title boş bırakan uygulamalar için yedek)
const ITEM_NAME_DISPLAY_KEY: PROPERTYKEY = PROPERTYKEY { fmtid: GUID::from_u128(0xB725F130_47EF_101A_A5F1_02608C9EEBAC), pid: 10 };
// PKEY_AppUserModel_IsDestListSeparator
const SEPARATOR_KEY: PROPERTYKEY = PROPERTYKEY { fmtid: GUID::from_u128(0x9F4C2855_9F79_4B39_A8D0_E1D42DE1D5F3), pid: 6 };

pub(crate) struct ShellLinkEntryReader;

fn blank_to_none(s: String) -> Option<String> {
    if s.trim().is_empty() { None } else { Some(s) }
}

impl ShellLinkEntryReader {
    /// customDestinations-ms içindeki ShellLink akışını (CLSID'siz) çözer. Atlanacak öğede `None`.
    pub(crate) fn from_stream(link_stream: &[u8]) -> Option<JumpListEntry> {
        // SAFETY: STA'da; bozuk akış hata döner ve öğe atlanır (uydurma öğe yok).
        unsafe {
            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).ok()?;
            let persist: IPersistStream = link.cast().ok()?;
            let stream = SHCreateMemStream(Some(link_stream))?;
            persist.Load(&stream).ok()?;
            Self::from_link(&link)
        }
    }

    /// Yüklenmiş bir IShellLinkW nesnesini girdiye çevirir.
    pub(crate) fn from_link(link: &IShellLinkW) -> Option<JumpListEntry> {
        let store: Option<IPropertyStore> = link.cast().ok();
        let mut title = store
            .as_ref()
            .and_then(|s| Self::resolve(read_string_property(s, &TITLE_KEY, true).as_deref()))
            .or_else(|| store.as_ref().and_then(|s| Self::resolve(read_string_property(s, &ITEM_NAME_DISPLAY_KEY, true).as_deref())));
        let separator = store.as_ref().is_some_and(|s| read_bool(s, &SEPARATOR_KEY));

        let mut target = read_link_text(link, LinkText::Path);
        let args = read_link_text(link, LinkText::Arguments);
        let dir = read_link_text(link, LinkText::WorkingDirectory);

        if title.as_deref().is_none_or(|t| t.trim().is_empty()) && !separator {
            title = Self::resolve(Some(&read_link_text(link, LinkText::Description)));
        }
        if target.trim().is_empty() && !separator {
            target = parsing_name_of_id_list(link).unwrap_or_default();
        }

        let dir = if dir.trim().is_empty() { None } else { Some(expand_environment_variables(&dir)) };
        JumpListEntryFactory::from_link(title.as_deref(), Some(&expand_environment_variables(&target)), Some(&args), dir.as_deref(), separator)
    }

    /// Son / sık kullanılan öğe (IShellItem): ad ve çalıştırılacak yol / URL.
    pub(crate) fn from_shell_item(item: &IShellItem) -> Option<JumpListEntry> {
        let title = get_name(item, SIGDN_NORMALDISPLAY);
        let target = get_name(item, SIGDN_FILESYSPATH)
            .or_else(|| get_name(item, SIGDN_URL))
            .or_else(|| get_name(item, SIGDN_DESKTOPABSOLUTEPARSING));
        JumpListEntryFactory::from_document(title.as_deref(), target.as_deref())
    }

    /// `@dll,-id` ve `@{ms-resource…}` dizelerini çözer; çözülemezse `None` (başlık uydurulmaz).
    pub(crate) fn resolve(text: Option<&str>) -> Option<String> {
        if !JumpListText::is_indirect(text) {
            return text.map(str::to_string);
        }
        let source = HSTRING::from(text?);
        let mut buffer = [0u16; TEXT_BUFFER];
        // SAFETY: kaynak geçerli dize, arabellek TEXT_BUFFER karakter.
        unsafe { SHLoadIndirectString(&source, &mut buffer, None) }.ok()?;
        let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
        blank_to_none(String::from_utf16_lossy(&buffer[..len]))
    }
}

#[derive(Clone, Copy)]
enum LinkText {
    Path,
    Arguments,
    WorkingDirectory,
    Description,
}

fn read_link_text(link: &IShellLinkW, which: LinkText) -> String {
    let mut buffer = [0u16; TEXT_BUFFER];
    // SAFETY: arabellek TEXT_BUFFER karakter; hata durumunda boş dize.
    let ok = unsafe {
        match which {
            LinkText::Path => link.GetPath(&mut buffer, std::ptr::null_mut(), 0),
            LinkText::Arguments => link.GetArguments(&mut buffer),
            LinkText::Description => link.GetDescription(&mut buffer),
            LinkText::WorkingDirectory => link.GetWorkingDirectory(&mut buffer),
        }
    }
    .is_ok();
    if !ok {
        return String::new();
    }
    let len = buffer.iter().position(|&c| c == 0).unwrap_or(buffer.len());
    String::from_utf16_lossy(&buffer[..len])
}

/// Hedefi yol değil yalnız kimlik listesi olan kısayol (ör. paketli uygulama): kabuk ayrıştırma adı.
fn parsing_name_of_id_list(link: &IShellLinkW) -> Option<String> {
    // SAFETY: PIDL ve dönen ad CoTaskMem ile serbest bırakılır.
    unsafe {
        let pidl = link.GetIDList().ok()?;
        if pidl.is_null() {
            return None;
        }
        let name = SHGetNameFromIDList(pidl, SIGDN_DESKTOPABSOLUTEPARSING);
        CoTaskMemFree(Some(pidl as *const _));
        let name = name.ok()?;
        let s = name.to_string().ok();
        CoTaskMemFree(Some(name.0 as *const _));
        s
    }
}

fn get_name(item: &IShellItem, kind: SIGDN) -> Option<String> {
    // SAFETY: dönen ad CoTaskMem ile serbest bırakılır; bu biçim yoksa hata döner.
    unsafe {
        let name = item.GetDisplayName(kind).ok()?;
        let s = name.to_string().ok();
        CoTaskMemFree(Some(name.0 as *const _));
        s.and_then(blank_to_none)
    }
}

fn read_bool(store: &IPropertyStore, key: &PROPERTYKEY) -> bool {
    // SAFETY: PROPVARIANT temizlenir.
    unsafe {
        let Ok(mut value) = store.GetValue(key) else { return false };
        let b = PropVariantToBooleanWithDefault(&value, false).as_bool();
        let _ = PropVariantClear(&mut value);
        b
    }
}
