//! Takvim durumunu Slint yapılarına çevirir ve sabit metinleri kurar.

use slint::{ComponentHandle, ModelRc, VecModel};

use koy_core::calendar::CalendarViewMode;

use super::calendar_view_model::CalendarState;
use crate::resources::strings::Strings;
use crate::{CalCellData, CalEventData, CalWeekRowData, CalendarData, CalendarText};

pub fn mode_index(m: CalendarViewMode) -> i32 {
    match m {
        CalendarViewMode::Month => 0,
        CalendarViewMode::Week => 1,
        CalendarViewMode::Day => 2,
    }
}

pub fn mode_from_index(i: i32) -> CalendarViewMode {
    match i {
        1 => CalendarViewMode::Week,
        2 => CalendarViewMode::Day,
        _ => CalendarViewMode::Month,
    }
}

fn model<T: Clone + 'static>(v: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(v))
}

pub fn to_data(s: &CalendarState, weekday_headers: &[String]) -> CalendarData {
    use chrono::Datelike;
    CalendarData {
        view_mode: mode_index(s.view_mode),
        header_title: s.header_title.clone().into(),
        day_title: s.day_title.clone().into(),
        weekday_headers: model(weekday_headers.iter().map(|h| slint::SharedString::from(h.as_str())).collect()),
        cells: model(
            s.cells
                .iter()
                .enumerate()
                .map(|(i, c)| CalCellData {
                    index: i as i32,
                    day: c.date.day() as i32,
                    current_month: c.is_current_month,
                    today: c.is_today,
                    selected: c.is_selected,
                    has_events: c.has_events,
                })
                .collect(),
        ),
        week_days: model(
            s.week_days
                .iter()
                .enumerate()
                .map(|(i, r)| CalWeekRowData {
                    index: i as i32,
                    label: r.label.clone().into(),
                    today: r.is_today,
                    selected: r.is_selected,
                    summary: r.summary.clone().into(),
                })
                .collect(),
        ),
        day_events: model(
            s.day_events
                .iter()
                .map(|e| CalEventData {
                    id: e.id.clone().into(),
                    time_text: e.time_text.clone().into(),
                    title: e.title.clone().into(),
                    can_delete: e.can_delete(),
                    badge: e.source_badge().into(),
                })
                .collect(),
        ),
        new_event_title: s.new_event_title.clone().into(),
        new_event_time: s.new_event_time.clone().into(),
        new_event_time_invalid: s.new_event_time_invalid,
    }
}

pub fn install_text<C: ComponentHandle>(c: &C)
where
    for<'a> CalendarText<'a>: slint::Global<'a, C>,
{
    let t = c.global::<CalendarText>();
    t.set_view_month(Strings::get("CalViewMonth").into());
    t.set_view_week(Strings::get("CalViewWeek").into());
    t.set_view_day(Strings::get("CalViewDay").into());
    t.set_no_events(Strings::get("CalNoEvents").into());
    t.set_title_hint(Strings::get("CalEventTitleHint").into());
    t.set_time_hint(Strings::get("TaskTimeHint").into());
}
