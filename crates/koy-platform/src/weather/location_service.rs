//! Cihaz konumu (Windows.Devices.Geolocation). Her çağrıda tek seferlik, düşük doğruluklu sorgu yapılır; sürekli
//! konum izleme yoktur. Ret, zaman aşımı ve hatalar durum olarak döner.

use koy_core::cancel::{CancellationToken, OperationCanceled};
use koy_core::time::TimeSpan;
use koy_core::weather::{ILocationService, LocationResult, LocationStatus};
use windows::Devices::Geolocation::{GeolocationAccessStatus, Geolocator, PositionAccuracy};
use windows::Foundation::TimeSpan as WinTimeSpan;

const QUERY_TIMEOUT: TimeSpan = TimeSpan::from_secs(12);
// Bir günlük önbellekli konum yeterli: hava durumu için kilometrelik doğruluk gerekmez.
const MAXIMUM_AGE: TimeSpan = TimeSpan::from_hours(24);

/// E_ACCESSDENIED (C# `UnauthorizedAccessException`).
const E_ACCESSDENIED: i32 = 0x8007_0005_u32 as i32;
/// 0x800705B4: WAIT_TIMEOUT, konum bulunamadı.
const HRESULT_WAIT_TIMEOUT: i32 = 0x8007_05B4_u32 as i32;

#[derive(Default)]
pub struct LocationService;

impl LocationService {
    fn query() -> windows::core::Result<LocationResult> {
        // RequestAccessAsync kullanıcıya Windows'un konum iznini ilk çağrıda sorar; ön plandaki bir pencereden,
        // arayüz iş parçacığında çağrılmalı (çağıran bunu sağlar).
        let access = Geolocator::RequestAccessAsync()?.join()?;
        if access == GeolocationAccessStatus::Denied {
            return Ok(LocationResult::of(LocationStatus::Denied));
        }
        if access != GeolocationAccessStatus::Allowed {
            return Ok(LocationResult::of(LocationStatus::Unavailable));
        }

        let locator = Geolocator::new()?;
        locator.SetDesiredAccuracy(PositionAccuracy::Default)?;
        locator.SetReportInterval(0)?;
        let position = locator
            .GetGeopositionAsyncWithAgeAndTimeout(
                WinTimeSpan { Duration: MAXIMUM_AGE.ticks() },
                WinTimeSpan { Duration: QUERY_TIMEOUT.ticks() },
            )?
            .join()?;
        let point = position.Coordinate()?.Point()?.Position()?;
        Ok(LocationResult::new(LocationStatus::Success, point.Latitude, point.Longitude))
    }
}

impl ILocationService for LocationService {
    fn get_location(&self, ct: &CancellationToken) -> Result<LocationResult, OperationCanceled> {
        if ct.is_cancellation_requested() {
            // C#: OperationCanceledException → TimedOut.
            return Ok(LocationResult::of(LocationStatus::TimedOut));
        }
        Ok(match Self::query() {
            Ok(r) => r,
            // Gizlilik ayarı sorgu sırasında kapatıldı ya da izin sonradan reddedildi.
            Err(e) if e.code().0 == E_ACCESSDENIED => LocationResult::of(LocationStatus::Denied),
            Err(e) if e.code().0 == HRESULT_WAIT_TIMEOUT => LocationResult::of(LocationStatus::TimedOut),
            Err(_) => LocationResult::of(LocationStatus::Error),
        })
    }
}
