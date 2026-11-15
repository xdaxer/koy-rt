//! Karşılığı: `Rules/RuleCondition.cs`.

use chrono::NaiveTime;

/// Ön plandaki süreç adı. Değer `RuleSetValidator::normalize_process_name` ile normalleştirilmiş olmalıdır.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessCondition {
    pub process_name: String,
}

impl ProcessCondition {
    pub fn new(process_name: impl Into<String>) -> Self {
        ProcessCondition { process_name: process_name.into() }
    }
}

/// Yerel saat aralığı: `from` dahil, `to` hariç. `from`, `to`'dan büyükse aralık gece yarısını aşar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimeWindowCondition {
    pub from: NaiveTime,
    pub to: NaiveTime,
}

impl TimeWindowCondition {
    pub fn new(from: NaiveTime, to: NaiveTime) -> Self {
        TimeWindowCondition { from, to }
    }
}

/// Bağlı ekran sayısı tam olarak bu değere eşitse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayCountCondition {
    pub count: i32,
}

impl DisplayCountCondition {
    pub fn new(count: i32) -> Self {
        DisplayCountCondition { count }
    }
}

/// Ön plandaki pencerenin tam ekran durumu bu değere eşitse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FullscreenCondition {
    pub is_fullscreen: bool,
}

impl FullscreenCondition {
    pub fn new(is_fullscreen: bool) -> Self {
        FullscreenCondition { is_fullscreen }
    }
}

/// Bir kuralın koşulu. Kuraldaki bütün koşullar birlikte sağlanmalıdır (VE).
///
/// C#'ta `abstract record RuleCondition` ve dört alt kayıt; Rust'ta kapalı bir enum. `Unknown`, C#'ta başka bir alt
/// sınıfın (testlerdeki `UnknownCondition`) karşılığıdır: hiçbir zaman eşleşmez (fail-closed).
#[derive(Debug, Clone, PartialEq)]
pub enum RuleCondition {
    Process(ProcessCondition),
    TimeWindow(TimeWindowCondition),
    DisplayCount(DisplayCountCondition),
    Fullscreen(FullscreenCondition),
    /// Tanınmayan koşul türü; değerlendirici eşleştirmez, `RuleSources` hiçbir kaynak istemez.
    Unknown,
}

impl From<ProcessCondition> for RuleCondition {
    fn from(c: ProcessCondition) -> Self {
        RuleCondition::Process(c)
    }
}
impl From<TimeWindowCondition> for RuleCondition {
    fn from(c: TimeWindowCondition) -> Self {
        RuleCondition::TimeWindow(c)
    }
}
impl From<DisplayCountCondition> for RuleCondition {
    fn from(c: DisplayCountCondition) -> Self {
        RuleCondition::DisplayCount(c)
    }
}
impl From<FullscreenCondition> for RuleCondition {
    fn from(c: FullscreenCondition) -> Self {
        RuleCondition::Fullscreen(c)
    }
}
