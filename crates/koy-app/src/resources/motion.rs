//! Hareket ayarları (C# `Motion`). Animasyonların kendisi .slint dosyalarında `animate` bloklarıdır; süreleri Slint
//! `Motion` global'indeki çarpanlarla ölçeklenir. Bu tür çarpanları ayarlardan ve Windows durumundan hesaplar.
//!
//! WPF'teki ekli özellikler (`FadeTo`, `ScaleTo`, `KnobOn`, `PageEnter`) .slint tarafında aynı sürelerle
//! (140 / 110 / 260 / 200 ms) `animate` olarak yazılır; bkz. `ui/motion.slint`.

use std::cell::{Cell, RefCell};

use koy_core::appearance::{MotionKind, MotionPolicy};
use koy_core::power::{BatterySaverPolicy, BatteryStatus};
use koy_core::settings::{AppSettings, MotionSettings};
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::UI::WindowsAndMessaging::{SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW};

thread_local! {
    static SCALE: Cell<f64> = const { Cell::new(1.0) };
    static TOGGLES: RefCell<MotionSettings> = RefCell::new(MotionSettings::default());
}

fn power_line_offline() -> bool {
    let mut status = SYSTEM_POWER_STATUS::default();
    // SAFETY: çıktı yerel yapıya yazılır. ACLineStatus 0 = pilde (C# PowerLineStatus.Offline).
    unsafe { GetSystemPowerStatus(&mut status) }.is_ok() && status.ACLineStatus == 0
}

pub fn client_area_animation() -> bool {
    let mut value = windows::core::BOOL(1);
    // SAFETY: çıktı BOOL'a yazılır.
    let ok = unsafe { SystemParametersInfoW(SPI_GETCLIENTAREAANIMATION, 0, Some(&mut value as *mut _ as *mut _), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0)) };
    ok.is_err() || value.as_bool()
}

pub struct Motion;

impl Motion {
    pub fn scale() -> f64 {
        SCALE.with(Cell::get)
    }

    pub fn enabled() -> bool {
        Self::scale() > 0.0
    }

    pub fn scale_for(kind: MotionKind) -> f64 {
        TOGGLES.with(|t| MotionPolicy::scale_for(kind, &*t.borrow(), Self::scale()))
    }

    pub fn enabled_for(kind: MotionKind) -> bool {
        Self::scale_for(kind) > 0.0
    }

    /// Taban süreyi genel çarpanla ölçekler (C# `D(ms)`).
    pub fn duration_ms(base_ms: f64) -> i32 {
        MotionPolicy::scaled_milliseconds(base_ms, Self::scale())
    }

    pub fn refresh(settings: &AppSettings) {
        let offline = power_line_offline();
        let battery_reduce = BatterySaverPolicy::should_reduce_motion(
            settings.appearance.battery_saver_reduce_motion,
            offline,
            if offline { BatteryStatus::Discharging } else { BatteryStatus::Idle },
        );
        let scale = MotionPolicy::scale(settings.appearance.animation_speed, settings.appearance.respect_reduce_motion, client_area_animation(), battery_reduce);
        SCALE.with(|s| s.set(scale));
        TOGGLES.with(|t| *t.borrow_mut() = settings.appearance.motion.clone());
    }
}
