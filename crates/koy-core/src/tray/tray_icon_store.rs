use super::tray_notify_data::{TrayIcon, TrayIconFields, TrayIconKey, TrayMessage, TrayNotifyData};

/// `TrayIconStore::apply` sonucu. `released_icons` artık kullanılmayan simge tanıtıcılarıdır;
/// sahibi (Platform) bunları yok etmelidir.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayApplyResult {
    pub success: bool,
    pub released_icons: Vec<isize>,
}

impl TrayApplyResult {
    pub fn new(success: bool, released_icons: Vec<isize>) -> Self {
        TrayApplyResult { success, released_icons }
    }

    /// `TrayApplyResult.Ok(params released)`: 0 tanıtıcılar elenir.
    pub fn ok(released: &[isize]) -> Self {
        TrayApplyResult { success: true, released_icons: Self::filter(released) }
    }

    /// `TrayApplyResult.Fail(params released)`: 0 tanıtıcılar elenir.
    pub fn fail(released: &[isize]) -> Self {
        TrayApplyResult { success: false, released_icons: Self::filter(released) }
    }

    fn filter(handles: &[isize]) -> Vec<isize> {
        handles.iter().copied().filter(|&h| h != 0).collect()
    }
}

/// Tepsi simgelerinin listesi. Shell_NotifyIcon kurallarını izler; simgeler ekleniş sırasını korur.
/// Simge tanıtıcılarının sahibi çağırandır: depodan çıkan her tanıtıcı sonuçta geri verilir.
#[derive(Debug, Default)]
pub struct TrayIconStore {
    icons: Vec<TrayIcon>,
}

impl TrayIconStore {
    /// Aynı anda tutulan en çok simge. Her kayıt bir simge kopyası (GDI nesnesi) tutar; hatalı ya da kötü niyetli
    /// bir uygulama sınırsız kayıt ekleyip belleği büyütemesin. Gerçek kullanımda simge sayısı onlarla ölçülür.
    pub const MAX_ICONS: usize = 256;

    const STATE_HIDDEN: u32 = 0x1;
    const DEFAULT_VERSION: u32 = 0;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn all(&self) -> &[TrayIcon] {
        &self.icons
    }

    /// Gösterilecek simgeler, ekleniş sırasıyla.
    pub fn visible(&self) -> Vec<TrayIcon> {
        self.icons.iter().filter(|i| i.is_visible()).cloned().collect()
    }

    pub fn apply(&mut self, data: &TrayNotifyData) -> TrayApplyResult {
        let key = TrayIconKey::from_data(data);
        let index = self.icons.iter().position(|i| i.key == key);
        let incoming_icon = if data.fields.has(TrayIconFields::ICON) { data.icon_handle } else { 0 };

        match data.message {
            TrayMessage::Add => {
                if let Some(index) = index {
                    // Koy tepsiyi devraldığında "TaskbarCreated" yayınıyla uygulamalar simgelerini yeniden ekler;
                    // zaten bilinen simgeyi güncelleme gibi ele almak, uygulamaya boşuna hata döndürmekten iyidir.
                    return self.update(index, data, incoming_icon);
                }

                if self.icons.len() >= Self::MAX_ICONS {
                    return TrayApplyResult::fail(&[incoming_icon]);
                }

                self.icons.push(TrayIcon {
                    key,
                    owner_hwnd: data.owner_hwnd,
                    id: data.id,
                    callback_message: if data.fields.has(TrayIconFields::MESSAGE) { data.callback_message } else { 0 },
                    icon_handle: incoming_icon,
                    tooltip: if data.fields.has(TrayIconFields::TIP) { data.tooltip.clone() } else { String::new() },
                    is_hidden: Self::apply_hidden(false, data),
                    version: Self::DEFAULT_VERSION,
                });
                TrayApplyResult::ok(&[])
            }

            TrayMessage::Modify => match index {
                Some(index) => self.update(index, data, incoming_icon),
                None => TrayApplyResult::fail(&[incoming_icon]),
            },

            TrayMessage::Delete => {
                let Some(index) = index else {
                    return TrayApplyResult::fail(&[incoming_icon]);
                };

                let removed = self.icons.remove(index);
                TrayApplyResult::ok(&[removed.icon_handle, incoming_icon])
            }

            TrayMessage::SetVersion => {
                let Some(index) = index else {
                    return TrayApplyResult::fail(&[incoming_icon]);
                };

                self.icons[index].version = data.version;
                TrayApplyResult::ok(&[incoming_icon])
            }

            TrayMessage::SetFocus => {
                TrayApplyResult::new(index.is_some(), if incoming_icon != 0 { vec![incoming_icon] } else { Vec::new() })
            }
        }
    }

    /// Sahibi kapanmış (NIM_DELETE göndermeden çöken) uygulamaların simgelerini temizler.
    pub fn remove_dead(&mut self, is_window_alive: impl Fn(isize) -> bool) -> Vec<isize> {
        let mut released = Vec::new();
        self.icons.retain(|icon| {
            if is_window_alive(icon.owner_hwnd) {
                return true;
            }

            if icon.icon_handle != 0 {
                released.push(icon.icon_handle);
            }
            false
        });
        released
    }

    /// Bütün simgeleri bırakır (tepsi kapanırken).
    pub fn clear(&mut self) -> Vec<isize> {
        let released = self.icons.iter().filter(|i| i.icon_handle != 0).map(|i| i.icon_handle).collect();
        self.icons.clear();
        released
    }

    fn update(&mut self, index: usize, data: &TrayNotifyData, incoming_icon: isize) -> TrayApplyResult {
        let current = self.icons[index].clone();
        let mut released: isize = 0;

        let mut icon_handle = current.icon_handle;
        if data.fields.has(TrayIconFields::ICON) {
            released = current.icon_handle;
            icon_handle = incoming_icon;
        }

        self.icons[index] = TrayIcon {
            // Pencere değişebilir (GUID'li simgede uygulama yeni pencereyle kaydolur); tıklamalar yeni pencereye gitmeli.
            owner_hwnd: if data.owner_hwnd != 0 { data.owner_hwnd } else { current.owner_hwnd },
            id: data.id,
            callback_message: if data.fields.has(TrayIconFields::MESSAGE) { data.callback_message } else { current.callback_message },
            icon_handle,
            tooltip: if data.fields.has(TrayIconFields::TIP) { data.tooltip.clone() } else { current.tooltip.clone() },
            is_hidden: Self::apply_hidden(current.is_hidden, data),
            ..current
        };

        TrayApplyResult::ok(&[if released == icon_handle { 0 } else { released }])
    }

    fn apply_hidden(current: bool, data: &TrayNotifyData) -> bool {
        if !data.fields.has(TrayIconFields::STATE) || (data.state_mask & Self::STATE_HIDDEN) == 0 {
            return current;
        }

        (data.state & Self::STATE_HIDDEN) != 0
    }
}
