use std::sync::Arc;

use crate::notch::NotchCardKind;
use crate::notifications::*;
use crate::time::testing::FakeClock;
use crate::time::TimeProvider;
use crate::tray::{Guid, TrayBalloonFlags, TrayBalloonNotification, TrayIconKey};

fn labels() -> RelativeTimeLabels {
    RelativeTimeLabels::new("şimdi", "{0} dk önce", "{0} sa önce")
}

#[test]
fn balloon_flows_through_notification_queue_to_notch_card() {
    let clock = Arc::new(FakeClock::new());
    let mut queue = NotificationQueue::new(Some(clock.clone()));
    let icon = vec![1u8, 2, 3, 4];

    let balloon = TrayBalloonNotification {
        key: TrayIconKey::new(100, 1, Guid::EMPTY),
        app_name: "Steam".to_owned(),
        title: "İndirme tamamlandı".to_owned(),
        body: "Oyun oynamaya hazır.".to_owned(),
        flags: TrayBalloonFlags::INFO,
        time: clock.utc_now(),
        icon: Some(icon.clone()),
    };

    let item = NotificationPresentation::from_tray_balloon(&balloon, 0x8000_0001);
    assert!(queue.enqueue(item, false));

    let next = queue.try_take_next(false);
    assert!(next.is_some());

    let card = NotificationPresentation::to_card(&next.unwrap(), clock.utc_now(), false, &labels());
    assert_eq!(card.kind, NotchCardKind::Notification);
    assert_eq!(card.header, "Steam · şimdi");
    assert_eq!(card.title, "İndirme tamamlandı");
    assert_eq!(card.subtitle, "Oyun oynamaya hazır.");
    assert_eq!(card.icon, Some(icon));
}

#[test]
fn balloon_respects_do_not_disturb_and_hide_body() {
    let clock = Arc::new(FakeClock::new());
    let mut queue = NotificationQueue::new(Some(clock.clone()));
    let balloon = TrayBalloonNotification {
        key: TrayIconKey::new(100, 1, Guid::EMPTY),
        app_name: "Mail".to_owned(),
        title: "Yeni ileti".to_owned(),
        body: "Gizli içerik".to_owned(),
        flags: TrayBalloonFlags::INFO,
        time: clock.utc_now(),
        icon: None,
    };

    let item = NotificationPresentation::from_tray_balloon(&balloon, 0x8000_0002);

    assert!(!queue.enqueue(item.clone(), true));
    assert!(queue.enqueue(item, false));

    let next = queue.try_take_next(false).unwrap();
    let card = NotificationPresentation::to_card(&next, clock.utc_now(), true, &labels());
    assert_eq!(card.title, "Yeni ileti");
    assert_eq!(card.subtitle, "");
}
