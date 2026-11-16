//! Odak / görev görünüm modellerinin durumunu Slint yapılarına çevirir (WPF bağlamalarının karşılığı) ve panelin
//! sabit metinlerini kurar.

use slint::{ComponentHandle, ModelRc, VecModel};

use koy_core::tasks::TaskScope;
use koy_core::timers::FocusKind;

use super::focus_view_model::{FocusPreset, FocusState};
use super::tasks_view_model::TasksState;
use crate::resources::strings::Strings;
use crate::{FocusData, FocusLapData, FocusPresetData, FocusTabData, FocusText, TaskRowData, TasksData};

pub fn kind_index(kind: FocusKind) -> i32 {
    match kind {
        FocusKind::Focus => 0,
        FocusKind::Break => 1,
        FocusKind::Countdown => 2,
        FocusKind::Stopwatch => 3,
    }
}

pub fn kind_from_index(i: i32) -> FocusKind {
    match i {
        1 => FocusKind::Break,
        2 => FocusKind::Countdown,
        3 => FocusKind::Stopwatch,
        _ => FocusKind::Focus,
    }
}

pub fn scope_index(scope: TaskScope) -> i32 {
    match scope {
        TaskScope::Week => 1,
        TaskScope::Month => 2,
        _ => 0,
    }
}

pub fn scope_from_index(i: i32) -> TaskScope {
    match i {
        1 => TaskScope::Week,
        2 => TaskScope::Month,
        _ => TaskScope::Day,
    }
}

fn model<T: Clone + 'static>(v: Vec<T>) -> ModelRc<T> {
    ModelRc::new(VecModel::from(v))
}

pub fn to_focus_data(s: &FocusState, presets: &[FocusPreset]) -> FocusData {
    FocusData {
        tabs: model(
            s.tabs
                .iter()
                .map(|t| FocusTabData { kind: kind_index(t.kind), label: t.label.clone().into(), selected: t.is_selected })
                .collect(),
        ),
        selected: kind_index(s.selected),
        time_text: s.time_text.clone().into(),
        sub_text: s.sub_text.clone().into(),
        progress: s.progress as f32,
        is_running: s.is_running,
        can_edit_countdown: s.can_edit_countdown,
        countdown_duration_text: s.countdown_duration_text.clone().into(),
        presets: model(presets.iter().map(|p| FocusPresetData { minutes: p.minutes, label: p.label.clone().into() }).collect()),
        laps: model(
            s.laps
                .iter()
                .map(|l| FocusLapData { label: l.label.clone().into(), total: l.total.clone().into(), split: l.split.clone().into() })
                .collect(),
        ),
    }
}

pub fn to_tasks_data(s: &TasksState) -> TasksData {
    TasksData {
        rows: model(
            s.rows
                .iter()
                .enumerate()
                .map(|(i, r)| TaskRowData {
                    index: i as i32,
                    title: r.title.clone().into(),
                    done: r.done,
                    is_next: r.is_next,
                    is_now: r.is_now,
                    time_text: r.time_text.clone().into(),
                    day_text: r.day_text.clone().into(),
                })
                .collect(),
        ),
        scope: scope_index(s.scope),
        title: s.title.clone().into(),
        is_empty: s.is_empty,
        new_title: s.new_title.clone().into(),
        new_time: s.new_time.clone().into(),
        new_time_invalid: s.new_time_invalid,
    }
}

/// Panelin sabit metinleri (C# `Loc.Instance[...]` bağlamaları).
pub fn install_text<C: ComponentHandle>(c: &C)
where
    for<'a> FocusText<'a>: slint::Global<'a, C>,
{
    let t = c.global::<FocusText>();
    let g = |k: &str| slint::SharedString::from(Strings::get(k));
    t.set_reset(g("Reset"));
    t.set_play_pause(g("MusicPlayPause"));
    t.set_skip(g("FocusSkip"));
    t.set_scope_day(g("TaskScopeDay"));
    t.set_scope_week(g("TaskScopeWeek"));
    t.set_scope_month(g("TaskScopeMonth"));
    t.set_task_empty(g("TaskEmpty"));
    t.set_task_now(g("TaskNow"));
    t.set_task_add(g("TaskAdd"));
    t.set_task_time_hint(g("TaskTimeHint"));
    t.set_timer(g("FocusTabTimer"));
    t.set_lap(g("FocusLap"));
    t.set_laps_empty(g("FocusLapsEmpty"));
}
