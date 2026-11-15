//! `UserNotificationListener` sarmalayıcısı. Paketsiz süreçte `NotificationChanged` aboneliği 0x80070490 ile reddedilir,
//! liste okuması çalışır; bu yüzden olay yok, `NotificationPoller` ile yoklanır. Yoklama `start` ile başlar, `stop` ile
//! söküldüğünde zamanlayıcı kalmaz. İş arka plan iş parçacığında yürür, olaylar arayüz iş parçacığında gelmez.

use std::collections::HashMap;

use koy_core::cancel::CancellationToken;
use koy_core::event::Event;
use koy_core::notifications::{
    INotificationListener, NotificationAccessStatus, NotificationItem, NotificationPoller, NotificationReadError, NotificationSnapshot,
    NotificationStamp,
};
use koy_core::time::DateTimeOffset;
use windows::ApplicationModel::AppDisplayInfo;
use windows::Foundation::{DateTime as WinDateTime, Size};
use windows::Storage::Streams::DataReader;
use windows::UI::Notifications::Management::{UserNotificationListener, UserNotificationListenerAccessStatus};
use windows::UI::Notifications::{KnownNotificationBindings, NotificationKinds, UserNotification};

use crate::bluetooth::support::ComApartment;
use crate::shell::IconHelper;

const LOGO_SIZE: f32 = 40.0;

pub struct WinRtNotificationListener {
    poller: NotificationPoller,
}

impl Default for WinRtNotificationListener {
    fn default() -> Self {
        WinRtNotificationListener { poller: NotificationPoller::new(Box::new(read), None) }
    }
}

fn map(status: UserNotificationListenerAccessStatus) -> NotificationAccessStatus {
    match status {
        UserNotificationListenerAccessStatus::Allowed => NotificationAccessStatus::Allowed,
        UserNotificationListenerAccessStatus::Denied => NotificationAccessStatus::Denied,
        UserNotificationListenerAccessStatus::Unspecified => NotificationAccessStatus::Unspecified,
        _ => NotificationAccessStatus::Unavailable,
    }
}

fn to_offset(dt: WinDateTime) -> DateTimeOffset {
    const UNIX_EPOCH_TICKS: i64 = 116_444_736_000_000_000;
    let ticks = dt.UniversalTime - UNIX_EPOCH_TICKS;
    chrono::DateTime::from_timestamp(ticks.div_euclid(10_000_000), (ticks.rem_euclid(10_000_000) * 100) as u32)
        .unwrap_or_default()
        .fixed_offset()
}

impl WinRtNotificationListener {
    /// Ayar sayfasının dinleyici kurmadan izni okuyabilmesi için ilişkisiz işlev.
    pub fn get_access_status() -> NotificationAccessStatus {
        let _com = ComApartment::enter_mta();
        UserNotificationListener::Current().and_then(|l| l.GetAccessStatus()).map_or(NotificationAccessStatus::Unavailable, map)
    }

    pub fn dispose(&self) {
        self.poller.stop();
    }
}

impl Drop for WinRtNotificationListener {
    fn drop(&mut self) {
        self.dispose();
    }
}

impl INotificationListener for WinRtNotificationListener {
    fn access_status(&self) -> NotificationAccessStatus {
        Self::get_access_status()
    }

    fn request_access(&self) -> NotificationAccessStatus {
        let _com = ComApartment::enter_mta();
        UserNotificationListener::Current()
            .and_then(|l| l.RequestAccessAsync())
            .and_then(|op| op.join())
            .map_or(NotificationAccessStatus::Unavailable, map)
    }

    fn notification_arrived(&self) -> &Event<NotificationItem> {
        self.poller.arrived()
    }

    fn start(&self) {
        self.poller.start();
    }

    fn stop(&self) {
        self.poller.stop();
    }
}

/// `known` `None`: temel çizgi okuması, yalnız kimlikler. Aksi halde ayrıntı (simge dahil) yalnız yeni ya da güncellenmiş
/// bildirimler için üretilir.
fn read(known: Option<&HashMap<u32, DateTimeOffset>>, token: &CancellationToken) -> Result<NotificationSnapshot, NotificationReadError> {
    let _com = ComApartment::enter_mta();
    let fail = |e: windows::core::Error| NotificationReadError::Failed(e.message());
    let list = UserNotificationListener::Current()
        .and_then(|l| l.GetNotificationsAsync(NotificationKinds::Toast))
        .and_then(|op| op.join())
        .map_err(fail)?;
    if token.is_cancellation_requested() {
        return Err(NotificationReadError::Canceled);
    }
    let mut stamps = Vec::new();
    let mut items = Vec::new();
    for notification in &list {
        let (Ok(id), Ok(created)) = (notification.Id(), notification.CreationTime()) else { continue };
        let time = to_offset(created);
        stamps.push(NotificationStamp::new(id, time));
        // Aynı kimlikle yerine konan bildirimin CreationTime'ı ilerler: o da yeni sayılır.
        match known {
            None => continue,
            Some(k) if k.get(&id).is_some_and(|&kt| time <= kt) => continue,
            _ => {}
        }
        if token.is_cancellation_requested() {
            return Err(NotificationReadError::Canceled);
        }
        if let Some(item) = try_build(&notification, id, time) {
            items.push(item);
        }
    }
    Ok(NotificationSnapshot::new(stamps, items))
}

fn try_build(notification: &UserNotification, id: u32, time: DateTimeOffset) -> Option<NotificationItem> {
    let mut app_name = String::new();
    let mut icon = None;
    let mut fallback = None;
    // Uygulama bilgisi çözülemeyen bildirim yine de gösterilir (adsız, simgesiz).
    if let Ok(info) = notification.AppInfo() {
        if let Ok(display) = info.DisplayInfo() {
            app_name = display.DisplayName().map(|h| h.to_string()).unwrap_or_default();
            // Her simge bu bildirimin kendi AppInfo'sundan okunur; öğeler arasında paylaşılan durum yok.
            icon = read_logo(&display);
        }
        fallback = read_shell_icon(info.AppUserModelId().ok().map(|h| h.to_string()).as_deref());
    }

    let mut title = String::new();
    let mut body: Vec<String> = Vec::new();
    let binding = notification
        .Notification()
        .and_then(|n| n.Visual())
        .and_then(|v| v.GetBinding(&KnownNotificationBindings::ToastGeneric()?))
        .ok();
    if let Some(binding) = binding
        && let Ok(elements) = binding.GetTextElements()
    {
        for element in &elements {
            let text = element.Text().map(|h| h.to_string()).unwrap_or_default();
            if text.trim().is_empty() {
                continue;
            }
            if title.is_empty() {
                title = text;
            } else {
                body.push(text);
            }
        }
    }

    let mut item = NotificationItem::new(id, app_name, title, body.join(" "), time);
    item.icon = icon;
    item.fallback_icon = fallback;
    Some(item)
}

// Klasik Win32 uygulamaların AppDisplayInfo logosu çoğu zaman boştur: kabuğun uygulama simgesi (AUMID anahtarlı önbellek).
fn read_shell_icon(app_user_model_id: Option<&str>) -> Option<Vec<u8>> {
    let aumid = app_user_model_id.filter(|a| !a.trim().is_empty())?;
    IconHelper::get_icon_bytes(&format!(r"shell:AppsFolder\{aumid}"))
}

// GetLogo simgesiz uygulamada boş/hata verebilir.
fn read_logo(display: &AppDisplayInfo) -> Option<Vec<u8>> {
    let reference = display.GetLogo(Size { Width: LOGO_SIZE, Height: LOGO_SIZE }).ok()?;
    let stream = reference.OpenReadAsync().ok()?.join().ok()?;
    let size = stream.Size().ok()?;
    if size == 0 || size > 512 * 1024 {
        return None;
    }
    let reader = DataReader::CreateDataReader(&stream).ok()?;
    reader.LoadAsync(size as u32).ok()?.join().ok()?;
    let mut buffer = vec![0u8; size as usize];
    reader.ReadBytes(&mut buffer).ok()?;
    Some(buffer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_access_status_without_prompting() {
        let _ = WinRtNotificationListener::get_access_status();
    }
}
