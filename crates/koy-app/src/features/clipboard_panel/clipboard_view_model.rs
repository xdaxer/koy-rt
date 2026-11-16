//! Pano sayfası: arama, süzgeç, kart listesi, geri kopyala / sabitle / sil / temizle. Servis NotchFeature'da (ayar
//! açıksa) alınır; bu model yalnız sayfa görünürken servis olayına abone olur, böylece gizliyken iş yapmaz.
//! `changed` arka plan iş parçacığından gelir; arayüz iş parçacığına geçirilir.
//! Karşılığı: `Features/ClipboardPanel/ClipboardViewModel.cs`.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use koy_core::clipboard::{AgeUnit, ClipboardDisplay, ClipboardFilter, ClipboardItem, ClipboardKind, Guid, IClipboardHistoryService};
use koy_core::event::Subscription;
use koy_core::time::DateTimeOffset;

use crate::infra::dispatch;
use crate::infra::observable::Notifier;
use crate::infra::ui_ref::UiRef;
use crate::resources::strings::Strings;

const COPIED_FLASH: Duration = Duration::from_millis(1400);

/// Süzgeç düğmesi (Tümü / Sabit / Bağlantı / Renk / Kod).
#[derive(Debug, Clone)]
pub struct ClipboardFilterChip {
    pub filter: ClipboardFilter,
    pub label: String,
    pub is_selected: bool,
}

/// Listedeki tek pano kartı.
#[derive(Debug, Clone)]
pub struct ClipboardCard {
    pub id: Guid,
    pub is_pinned: bool,
    pub kind: ClipboardKind,
    /// "Bağlantı · sabit", "Renk · 2 dk".
    pub header: String,
    /// Metnin ilk satırları (renkte renk metninin kendisi).
    pub preview: String,
    /// Renk öğesinde örnek (a, r, g, b); diğerlerinde None.
    pub swatch: Option<(u8, u8, u8, u8)>,
    /// Az önce geri kopyalandı: başlıkta kısa süre "Kopyalandı" yazar.
    pub is_copied: bool,
}

impl ClipboardCard {
    pub fn new(item: &ClipboardItem, now: DateTimeOffset) -> Self {
        let kind = Strings::get(match item.kind {
            ClipboardKind::Link => "ClipKindLink",
            ClipboardKind::Color => "ClipKindColor",
            ClipboardKind::Code => "ClipKindCode",
            _ => "ClipKindText",
        });
        let tail = if item.pinned { Strings::get("ClipPinnedLabel") } else { format_age(item.copied_at, now) };
        let swatch = if item.kind == ClipboardKind::Color {
            ClipboardDisplay::try_parse_color(Some(&item.text)).map(|(r, g, b, a)| (a, r, g, b))
        } else {
            None
        };
        ClipboardCard {
            id: item.id,
            is_pinned: item.pinned,
            kind: item.kind,
            header: format!("{kind} · {tail}"),
            preview: ClipboardDisplay::preview(Some(&item.text), 3, 90),
            swatch,
            is_copied: false,
        }
    }

    /// Sabitle / Sabitlemeyi kaldır ipucu.
    pub fn pin_tip(&self) -> String {
        Strings::get(if self.is_pinned { "ClipUnpin" } else { "ClipPin" })
    }

    pub fn is_link(&self) -> bool {
        self.kind == ClipboardKind::Link
    }
}

fn format_age(copied_at: DateTimeOffset, now: DateTimeOffset) -> String {
    let (unit, value) = ClipboardDisplay::age(copied_at, now);
    match unit {
        AgeUnit::Minutes => Strings::format("AgeMinutes", &[&value.to_string()]),
        AgeUnit::Hours => Strings::format("AgeHours", &[&value.to_string()]),
        AgeUnit::Days => Strings::format("AgeDays", &[&value.to_string()]),
        _ => Strings::get("AgeNow"),
    }
}

#[derive(Debug, Clone, Default)]
pub struct ClipboardState {
    pub items: Vec<ClipboardCard>,
    pub filters: Vec<ClipboardFilterChip>,
    pub search_text: String,
    pub filter: ClipboardFilter,
    pub is_empty: bool,
    /// Liste boşken yazı: geçmiş hiç yok / süzgeç ve arama eşleşmedi.
    pub empty_text: String,
}

pub struct ClipboardViewModel {
    service: Arc<dyn IClipboardHistoryService>,
    state: RefCell<ClipboardState>,
    pub changed: Notifier,
    sub: Cell<Option<Subscription>>,
    flash_timer: RefCell<Option<slint::Timer>>,
    disposed: Cell<bool>,
}

impl ClipboardViewModel {
    pub fn new(service: Arc<dyn IClipboardHistoryService>) -> Rc<Self> {
        let chip = |filter, key: &str| ClipboardFilterChip { filter, label: Strings::get(key), is_selected: filter == ClipboardFilter::All };
        Rc::new(ClipboardViewModel {
            service,
            state: RefCell::new(ClipboardState {
                filters: vec![
                    chip(ClipboardFilter::All, "ClipFilterAll"),
                    chip(ClipboardFilter::Pinned, "ClipFilterPinned"),
                    chip(ClipboardFilter::Link, "ClipFilterLink"),
                    chip(ClipboardFilter::Color, "ClipFilterColor"),
                    chip(ClipboardFilter::Code, "ClipFilterCode"),
                ],
                is_empty: true,
                ..Default::default()
            }),
            changed: Notifier::default(),
            sub: Cell::new(None),
            flash_timer: RefCell::new(None),
            disposed: Cell::new(false),
        })
    }

    pub fn state(&self) -> std::cell::Ref<'_, ClipboardState> {
        self.state.borrow()
    }

    fn is_active(&self) -> bool {
        let s = self.sub.take();
        let r = s.is_some();
        self.sub.set(s);
        r
    }

    /// Sayfa görünürken true: olaya abone olur ve listeyi tazeler; false: aboneliği keser.
    pub fn set_active(self: &Rc<Self>, active: bool) {
        if self.disposed.get() || active == self.is_active() {
            return;
        }
        if active {
            let me = UiRef::new(self);
            let sub = self.service.changed().subscribe(move |_| {
                dispatch::post(move || {
                    if let Some(vm) = me.upgrade()
                        && !vm.disposed.get()
                        && vm.is_active()
                    {
                        vm.reload();
                        vm.changed.notify();
                    }
                });
            });
            self.sub.set(Some(sub));
            self.reload();
            self.changed.notify();
        } else {
            if let Some(sub) = self.sub.take() {
                self.service.changed().unsubscribe(sub);
            }
            *self.flash_timer.borrow_mut() = None;
            self.clear_flash();
        }
    }

    fn reload(&self) {
        let (filter, search) = {
            let st = self.state.borrow();
            (st.filter, st.search_text.trim().to_string())
        };
        let all = self.service.query(filter, if search.is_empty() { None } else { Some(&search) });
        let now = chrono::Local::now().fixed_offset();
        let mut st = self.state.borrow_mut();
        st.items = all.iter().map(|i| ClipboardCard::new(i, now)).collect();
        st.is_empty = st.items.is_empty();
        let filtering = filter != ClipboardFilter::All || !search.is_empty();
        st.empty_text = Strings::get(if filtering { "ClipNoMatch" } else { "ClipEmpty" });
    }

    pub fn set_search_text(&self, text: &str) {
        if self.state.borrow().search_text == text {
            return;
        }
        self.state.borrow_mut().search_text = text.to_string();
        self.reload();
        self.changed.notify();
    }

    pub fn set_filter(&self, filter: ClipboardFilter) {
        {
            let mut st = self.state.borrow_mut();
            if st.filter == filter {
                return;
            }
            st.filter = filter;
            for f in st.filters.iter_mut() {
                f.is_selected = f.filter == filter;
            }
        }
        self.reload();
        self.changed.notify();
    }

    /// Karta tıklayınca öğe panoya geri yazılır (listeye yeniden eklenmez, sıra değişmez).
    pub fn copy_back(self: &Rc<Self>, id: Guid) {
        if !self.service.copy_back(id) {
            return;
        }
        self.clear_flash();
        if let Some(card) = self.state.borrow_mut().items.iter_mut().find(|c| c.id == id) {
            card.is_copied = true;
        }
        let weak = Rc::downgrade(self);
        *self.flash_timer.borrow_mut() = Some(dispatch::delay(COPIED_FLASH, move || {
            if let Some(vm) = weak.upgrade() {
                vm.clear_flash();
                vm.changed.notify();
            }
        }));
        self.changed.notify();
    }

    pub fn toggle_pin(&self, id: Guid) {
        let pinned = self.state.borrow().items.iter().find(|c| c.id == id).map(|c| c.is_pinned);
        if let Some(p) = pinned {
            self.service.set_pinned(id, !p);
        }
    }

    pub fn remove(&self, id: Guid) {
        self.service.remove(id);
    }

    /// Sabitlenmeyenleri siler.
    pub fn clear(&self) {
        self.service.clear();
    }

    fn clear_flash(&self) {
        for c in self.state.borrow_mut().items.iter_mut() {
            c.is_copied = false;
        }
    }

    pub fn dispose(self: &Rc<Self>) {
        if self.disposed.get() {
            return;
        }
        self.set_active(false);
        self.disposed.set(true);
    }
}
