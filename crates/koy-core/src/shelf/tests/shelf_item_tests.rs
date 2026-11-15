//! Karşılığı: `ModelContractTests.cs` içindeki `ShelfItem_InitializesProperly` (dosyanın kalanı Audio/Display/Notch/
//! Power/Radio/Weather modüllerine bağlı; o testler ilgili modüllerle taşınır).

use crate::shelf::*;
use crate::time::{DateTimeOffset, SystemTimeProvider, TimeProvider};

#[test]
fn shelf_item_initializes_properly() {
    let now: DateTimeOffset = SystemTimeProvider.utc_now();
    let item = ShelfItem::new("id1", "file.txt", "C:\\file.txt", ShelfItemKind::File, now);

    assert_eq!("id1", item.id);
    assert_eq!("file.txt", item.display_name);
    assert_eq!("C:\\file.txt", item.path_or_content);
    assert_eq!(ShelfItemKind::File, item.kind);
    assert_eq!(now, item.added_at);
}
