//! Karşılığı: `koy/src/Core/Calendar/`.
//!
//! Ek dosya `iso_time.rs`: System.Text.Json'ın tarih biçimi ve API yanıtlarındaki ISO 8601 metni (C#'ta çatı yapar).
//! Not: bu modül `crate::clock` (DayOfWeek, TimeZoneInfo) kullanır; `calendar` özelliği `clock` özelliğine bağlıdır.

mod calendar_event;
mod calendar_event_parsers;
mod calendar_oauth;
mod calendar_sync_contracts;
mod event_agenda;
mod event_book;
mod event_store;
mod iso_time;
mod month_grid;
mod week_grid;

pub use calendar_event::{CalendarEvent, CalendarEventSource, CalendarViewMode, DayAgenda};
pub use calendar_event_parsers::{CalendarEventParsers, CalendarParseError};
pub use calendar_oauth::{
    escape_data_string, unescape_data_string, CalendarOAuthConfig, CalendarProvider, OAuthBuiltInClients, OAuthCallbackResult, OAuthClient,
    OAuthFlowState, OAuthPkce, OAuthSessionState, Uri, UriFormatError,
};
pub use calendar_sync_contracts::{
    BoxFuture, CalendarCacheSnapshot, CalendarCacheStore, CalendarSettings, CalendarSettingsStore, CalendarSyncResult, CalendarTokenStore,
    CancellationToken, ICalendarSyncService, ITokenProtector, OAuthConnectResult, OAuthTokenSet,
};
pub use event_agenda::EventAgenda;
pub use event_book::EventBook;
pub use event_store::EventStore;
pub use month_grid::{CalendarDay, MonthGrid};
pub use week_grid::WeekGrid;

#[cfg(test)]
mod tests;
