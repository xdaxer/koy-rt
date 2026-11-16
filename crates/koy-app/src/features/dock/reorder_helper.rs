//! Sabitli öğeleri sürükleyerek sıralama ve bir uygulamayı başka uygulamanın / klasörün üstüne bekletip klasöre katma.
//! Karşılığı: `DockReorderHelper.cs`. Görünüm, şerit koordinatlarında (DIP) basma, hareket ve bırakma bildirir.

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use std::time::{Duration, Instant};

use koy_core::dock::{DockDropZone, DockDropZoneTracker, DockDropZones, DockReorderMath};
use koy_core::time::TimeSpan;

use super::dock_view_model::DockViewModel;
use super::entries::DockEntry;

// Windows'un en küçük sürükleme mesafesi (SM_CXDRAG varsayılanı, DIP).
const MIN_DRAG_DISTANCE: f64 = 4.0;

struct Drag {
    entry: DockEntry,
    start: (f64, f64),
    dragging: bool,
    original_index: i32,
    slot_width: f64,
    last_offset: f64,
}

pub struct DockReorderHelper {
    vm: Weak<DockViewModel>,
    tracker: RefCell<DockDropZoneTracker>,
    merge_timer: slint::Timer,
    drag: RefCell<Option<Drag>>,
    pressed: RefCell<Option<DockEntry>>,
    glowing: RefCell<Option<DockEntry>>,
    started: Instant,
    me: RefCell<Weak<Self>>,
    finished_drag: Cell<bool>,
}

impl DockReorderHelper {
    pub fn new(vm: &Rc<DockViewModel>) -> Rc<Self> {
        let h = Rc::new(DockReorderHelper {
            vm: Rc::downgrade(vm),
            tracker: RefCell::new(DockDropZoneTracker::new(None)),
            merge_timer: slint::Timer::default(),
            drag: RefCell::new(None),
            pressed: RefCell::new(None),
            glowing: RefCell::new(None),
            started: Instant::now(),
            me: RefCell::new(Weak::new()),
            finished_drag: Cell::new(false),
        });
        *h.me.borrow_mut() = Rc::downgrade(&h);
        h
    }

    fn now(&self) -> TimeSpan {
        TimeSpan::from_seconds_f64(self.started.elapsed().as_secs_f64())
    }

    /// Sol tuşla basıldı. `slot_width`: öğenin genişliği + kenar boşlukları (dikeyde yüksekliği).
    pub fn on_mouse_down(&self, index: usize, pos: (f64, f64), slot_width: f64) {
        let Some(vm) = self.vm.upgrade() else { return };
        let Some(entry) = vm.items().get(index).cloned() else { return };
        // Basma geri bildirimi sıralanabilir olmasa da çalışır.
        self.release_press();
        vm.set_entry_pressed(&entry, true);
        *self.pressed.borrow_mut() = Some(entry.clone());
        if !vm.can_start_reorder(&entry) {
            return;
        }
        *self.drag.borrow_mut() = Some(Drag { entry, start: pos, dragging: false, original_index: index as i32, slot_width, last_offset: 0.0 });
    }

    /// Fare hareket etti (tuş basılıyken).
    pub fn on_mouse_move(&self, pos: (f64, f64), left_down: bool) {
        let Some(vm) = self.vm.upgrade() else { return };
        if self.drag.borrow().is_none() {
            return;
        }
        if !left_down {
            let dragging = self.drag.borrow().as_ref().is_some_and(|d| d.dragging);
            self.reset(&vm, dragging);
            return;
        }
        let vertical = vm.is_vertical();
        let begin = {
            let mut d = self.drag.borrow_mut();
            let d = d.as_mut().expect("sürükleme");
            if !d.dragging {
                if !DockReorderMath::exceeds_threshold(pos.0 - d.start.0, pos.1 - d.start.1, MIN_DRAG_DISTANCE) {
                    return;
                }
                d.dragging = true;
                true
            } else {
                false
            }
        };
        if begin {
            // Sürükleme başladı: basma geri bildirimi iptal (öğe imleci izlerken içe çökük kalmaz).
            self.release_press();
            vm.begin_reorder();
        }
        let (entry, original, slot, offset) = {
            let mut d = self.drag.borrow_mut();
            let d = d.as_mut().expect("sürükleme");
            let delta = if vertical { pos.1 - d.start.1 } else { pos.0 - d.start.0 };
            // İmleç yalnız sabitli yuvalar arasında gezer: sınırın ötesine kayma kırpılır.
            let count = vm.pinned_count();
            let min = -(d.original_index as f64) * d.slot_width;
            let max = (count - 1 - d.original_index) as f64 * d.slot_width;
            d.last_offset = delta.clamp(min, max.max(min));
            (d.entry.clone(), d.original_index, d.slot_width, d.last_offset)
        };
        let count = vm.pinned_count();
        let target = DockReorderMath::target_index(count, original, offset, slot);
        let item_count = vm.items().len() as i32;
        // Yalnız uygulamalar başka uygulama ya da klasörle birleştirilebilir (klasörler iç içe geçmez).
        let can_merge = matches!(entry, DockEntry::App(_)) && target != original && target >= 0 && target < item_count;
        if can_merge {
            let relative = slot * 0.5 + (offset - (target - original) as f64 * slot);
            let zone = DockDropZones::detect_zone(slot, relative, 0.5);
            if zone == DockDropZone::Merge {
                let armed = self.tracker.borrow_mut().update(target, zone, self.now());
                if armed {
                    self.merge_timer.stop();
                    self.arm_merge(&vm, target);
                    return;
                }
                if !self.merge_timer.running() {
                    let me = self.me.borrow().clone();
                    let delay = Duration::from_millis(DockDropZones::merge_hover_delay().total_milliseconds().max(1.0) as u64);
                    self.merge_timer.start(slint::TimerMode::SingleShot, delay, move || {
                        if let Some(h) = me.upgrade() {
                            h.on_merge_timer();
                        }
                    });
                }
            } else {
                self.merge_timer.stop();
                self.tracker.borrow_mut().reset();
                self.clear_glow();
            }
        } else {
            self.merge_timer.stop();
            self.tracker.borrow_mut().reset();
            self.clear_glow();
        }
        if !self.tracker.borrow().is_merge_armed() {
            vm.update_reorder(&entry, original, offset, slot);
        }
    }

    fn on_merge_timer(&self) {
        let Some(vm) = self.vm.upgrade() else { return };
        let Some((original, slot, offset)) = self.drag.borrow().as_ref().filter(|d| d.dragging).map(|d| (d.original_index, d.slot_width, d.last_offset))
        else {
            return;
        };
        let target = DockReorderMath::target_index(vm.pinned_count(), original, offset, slot);
        if target != original && target >= 0 && target < vm.items().len() as i32 {
            let now = self.now();
            self.tracker.borrow_mut().update(target, DockDropZone::Merge, now);
            self.arm_merge(&vm, target);
        }
    }

    fn arm_merge(&self, vm: &Rc<DockViewModel>, target: i32) {
        let items = vm.items();
        let Some(target_entry) = items.get(target as usize).cloned() else { return };
        let same = self.glowing.borrow().as_ref().is_some_and(|g| g.same(&target_entry));
        if !same {
            self.clear_glow();
            target_entry.with_motion(|m| {
                m.is_target_glow = true;
                m.set_target_scale(1.1);
            });
            *self.glowing.borrow_mut() = Some(target_entry);
        }
        // Birleştirme hedeflenince öteki simgeler yer değiştirmez; sürüklenen imleçte kalır.
        if let Some(d) = self.drag.borrow().as_ref() {
            d.entry.with_motion(|m| m.snap_offset(d.last_offset));
            let pinned = vm.pinned_count() as usize;
            for (i, e) in items.iter().enumerate().take(pinned) {
                if i as i32 != d.original_index {
                    e.with_motion(|m| m.set_target_offset(0.0));
                }
            }
        }
        vm.changed.notify();
    }

    /// Bırakma: sürükleme sürmüşse tamamlar ve true döner (çağıran tıklamayı işlemez).
    pub fn try_finish(&self) -> bool {
        self.merge_timer.stop();
        self.release_press();
        let Some(drag) = self.drag.borrow_mut().take() else { return false };
        let Some(vm) = self.vm.upgrade() else { return false };
        if !drag.dragging {
            self.clear_glow();
            self.tracker.borrow_mut().reset();
            return false;
        }
        let (armed, slot) = {
            let t = self.tracker.borrow();
            (t.is_merge_armed(), t.target_slot())
        };
        self.clear_glow();
        self.tracker.borrow_mut().reset();
        let items = vm.items();
        if armed
            && let DockEntry::App(dragged) = &drag.entry
            && slot >= 0
            && (slot as usize) < items.len()
            && slot != drag.original_index
        {
            let dragged_id = dragged.borrow().id().to_string();
            match &items[slot as usize] {
                DockEntry::Folder(f) => {
                    let fid = f.borrow().folder.id.clone();
                    vm.add_app_to_folder(&dragged_id, &fid);
                }
                DockEntry::App(t) => {
                    let tid = t.borrow().id().to_string();
                    vm.create_folder_from_drop(&dragged_id, &tid);
                }
                DockEntry::Extra(_) => {}
            }
            return true;
        }
        vm.commit_reorder(&drag.entry, drag.original_index, drag.last_offset, drag.slot_width);
        self.finished_drag.set(true);
        true
    }

    /// Fare yakalaması kayboldu: sürükleme iptal.
    pub fn on_lost_capture(&self) {
        self.release_press();
        let dragging = self.drag.borrow().as_ref().is_some_and(|d| d.dragging);
        if dragging && let Some(vm) = self.vm.upgrade() {
            self.reset(&vm, true);
        }
    }

    /// Fare öğeden ayrıldı (sürükleme yokken): basılı kalmış görünüm bırakılır.
    pub fn on_mouse_leave(&self) {
        let dragging = self.drag.borrow().as_ref().is_some_and(|d| d.dragging);
        if !dragging {
            self.release_press();
        }
    }

    pub fn is_dragging(&self) -> bool {
        self.drag.borrow().as_ref().is_some_and(|d| d.dragging)
    }

    fn release_press(&self) {
        let entry = self.pressed.borrow_mut().take();
        if let (Some(e), Some(vm)) = (entry, self.vm.upgrade()) {
            vm.set_entry_pressed(&e, false);
        }
    }

    fn reset(&self, vm: &Rc<DockViewModel>, cancel: bool) {
        self.merge_timer.stop();
        self.release_press();
        self.clear_glow();
        *self.drag.borrow_mut() = None;
        self.tracker.borrow_mut().reset();
        if cancel {
            vm.cancel_reorder();
        }
    }

    fn clear_glow(&self) {
        if let Some(g) = self.glowing.borrow_mut().take() {
            g.with_motion(|m| {
                m.is_target_glow = false;
                m.set_target_scale(1.0);
            });
        }
    }
}
