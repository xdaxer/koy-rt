//! Karşılığı: `tests/Core.Tests/LyricsSessionTests.cs`.
//!
//! C#'taki `Task` / `TaskCompletionSource` yerine: sağlayıcı bir iş parçacığında bloklanır, `ManualProvider`
//! sonucu `set_result` ile verir; `await load` yerine `LoadTask::wait()`.

use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use crate::lyrics::{
    CancellationToken, ILyricsProvider, ILyricsSource, LyricLine, LyricsCache, LyricsError, LyricsLoadResult,
    LyricsLoadStatus, LyricsLookup, LyricsProvider, LyricsQuery, LyricsSession, LyricsState,
};
use crate::time::TimeSpan;

const LRC: &str = "[00:01.00]line";

fn query_a() -> LyricsQuery {
    LyricsQuery::new("A", "Artist", None)
}

fn query_b() -> LyricsQuery {
    LyricsQuery::new("B", "Artist", None)
}

struct TempDir(std::path::PathBuf);

impl TempDir {
    fn new() -> Self {
        static COUNTER: AtomicI32 = AtomicI32::new(0);
        TempDir(std::env::temp_dir().join(format!(
            "koy-lyrics-session-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        )))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct FakeSource {
    results: Vec<LyricsLookup>,
    next: Mutex<usize>,
    calls: AtomicI32,
}

impl FakeSource {
    fn new(results: Vec<LyricsLookup>) -> Arc<Self> {
        Arc::new(FakeSource { results, next: Mutex::new(0), calls: AtomicI32::new(0) })
    }

    fn calls(&self) -> i32 {
        self.calls.load(Ordering::SeqCst)
    }
}

impl ILyricsSource for FakeSource {
    fn find(&self, _query: &LyricsQuery, _token: &CancellationToken) -> Result<LyricsLookup, LyricsError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let mut next = self.next.lock().unwrap();
        let result = self.results[(*next).min(self.results.len() - 1)].clone();
        *next += 1;
        Ok(result)
    }
}

/// `InvalidOperationException` karşılığı.
#[derive(Debug)]
struct InvalidOperation(String);

impl fmt::Display for InvalidOperation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl Error for InvalidOperation {}

/// `TaskCompletionSource<LyricsLoadResult>` karşılığı.
struct Completion {
    result: Mutex<Option<Result<LyricsLoadResult, String>>>,
    signal: Condvar,
}

impl Completion {
    fn set_result(&self, result: LyricsLoadResult) {
        *self.result.lock().unwrap() = Some(Ok(result));
        self.signal.notify_all();
    }

    fn set_exception(&self, message: &str) {
        *self.result.lock().unwrap() = Some(Err(message.to_string()));
        self.signal.notify_all();
    }

    fn wait(&self) -> Result<LyricsLoadResult, String> {
        let mut guard = self.result.lock().unwrap();
        while guard.is_none() {
            guard = self.signal.wait(guard).unwrap();
        }
        guard.clone().unwrap()
    }
}

#[derive(Default)]
struct ManualProvider {
    pending: Mutex<HashMap<String, Arc<Completion>>>,
    calls: AtomicI32,
}

impl ManualProvider {
    fn new() -> Arc<Self> {
        Arc::new(ManualProvider::default())
    }

    fn for_query(&self, query: &LyricsQuery) -> Arc<Completion> {
        let mut pending = self.pending.lock().unwrap();
        Arc::clone(pending.entry(query.key()).or_insert_with(|| {
            Arc::new(Completion { result: Mutex::new(None), signal: Condvar::new() })
        }))
    }

    fn clear_pending(&self) {
        self.pending.lock().unwrap().clear();
    }

    fn calls(&self) -> i32 {
        self.calls.load(Ordering::SeqCst)
    }
}

impl ILyricsProvider for ManualProvider {
    fn load(&self, query: &LyricsQuery, _token: &CancellationToken) -> Result<LyricsLoadResult, LyricsError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self.for_query(query).wait() {
            Ok(result) => Ok(result),
            Err(message) => Err(Box::new(InvalidOperation(message))),
        }
    }
}

fn found(text: &str) -> LyricsLoadResult {
    LyricsLoadResult::new(LyricsLoadStatus::Found, vec![LyricLine::new(TimeSpan::from_secs(1), text)])
}

fn provider(dir: &TempDir, local: Arc<FakeSource>, online: Arc<FakeSource>) -> LyricsProvider {
    LyricsProvider::new(local, LyricsCache::new(&dir.0), online)
}

fn load(provider: &LyricsProvider, query: &LyricsQuery) -> LyricsLoadResult {
    provider.load(query, &CancellationToken::none()).unwrap()
}

#[test]
fn provider_prefers_local_and_stays_offline() {
    let dir = TempDir::new();
    let online = FakeSource::new(vec![LyricsLookup::found(LRC)]);

    let result = load(&provider(&dir, FakeSource::new(vec![LyricsLookup::found(LRC)]), online.clone()), &query_a());

    assert_eq!(LyricsLoadStatus::Found, result.status);
    assert_eq!(0, online.calls());
}

#[test]
fn provider_downloads_once_then_uses_cache() {
    let dir = TempDir::new();
    let online = FakeSource::new(vec![LyricsLookup::found(LRC)]);
    let provider = provider(&dir, FakeSource::new(vec![LyricsLookup::not_found()]), online.clone());

    load(&provider, &query_a());
    let second = load(&provider, &query_a());

    assert_eq!(LyricsLoadStatus::Found, second.status);
    assert_eq!(1, online.calls());
}

#[test]
fn provider_remembers_tracks_without_lyrics() {
    let dir = TempDir::new();
    let online = FakeSource::new(vec![LyricsLookup::not_found()]);
    let provider = provider(&dir, FakeSource::new(vec![LyricsLookup::not_found()]), online.clone());

    load(&provider, &query_a());
    let second = load(&provider, &query_a());

    assert_eq!(LyricsLoadStatus::NotFound, second.status);
    assert_eq!(1, online.calls());
}

#[test]
fn provider_does_not_cache_temporary_failures() {
    let dir = TempDir::new();
    let online = FakeSource::new(vec![LyricsLookup::failed(), LyricsLookup::found(LRC)]);
    let provider = provider(&dir, FakeSource::new(vec![LyricsLookup::not_found()]), online);

    assert_eq!(LyricsLoadStatus::Failed, load(&provider, &query_a()).status);
    assert_eq!(LyricsLoadStatus::Found, load(&provider, &query_a()).status);
}

#[test]
fn provider_treats_unusable_lrc_as_not_found() {
    let dir = TempDir::new();
    let provider = provider(
        &dir,
        FakeSource::new(vec![LyricsLookup::not_found()]),
        FakeSource::new(vec![LyricsLookup::found("no stamps")]),
    );

    assert_eq!(LyricsLoadStatus::NotFound, load(&provider, &query_a()).status);
}

#[test]
fn session_goes_from_loading_to_ready() {
    let provider = ManualProvider::new();
    let session = LyricsSession::new(provider.clone());

    let load = session.load(Some(&query_a()));
    assert_eq!(LyricsState::Loading, session.state());

    provider.for_query(&query_a()).set_result(found("a"));
    load.wait();

    assert_eq!(LyricsState::Ready, session.state());
    let lines = session.lines();
    assert_eq!(1, lines.len());
    assert_eq!("a", lines[0].text);
}

#[test]
fn late_result_of_previous_track_is_ignored() {
    let provider = ManualProvider::new();
    let session = LyricsSession::new(provider.clone());

    let first = session.load(Some(&query_a()));
    let second = session.load(Some(&query_b()));

    provider.for_query(&query_b()).set_result(found("b"));
    second.wait();
    provider.for_query(&query_a()).set_result(found("a"));
    first.wait();

    let lines = session.lines();
    assert_eq!(1, lines.len());
    assert_eq!("b", lines[0].text);
    assert_eq!(LyricsState::Ready, session.state());
}

#[test]
fn same_track_is_not_loaded_twice() {
    let provider = ManualProvider::new();
    let session = LyricsSession::new(provider.clone());
    provider.for_query(&query_a()).set_result(found("a"));

    session.load(Some(&query_a())).wait();
    session.load(Some(&LyricsQuery { duration: Some(TimeSpan::from_mins(3)), ..query_a() })).wait();

    assert_eq!(1, provider.calls());
}

#[test]
fn failed_track_is_retried() {
    let provider = ManualProvider::new();
    let session = LyricsSession::new(provider.clone());
    provider.for_query(&query_a()).set_result(LyricsLoadResult::failed());
    session.load(Some(&query_a())).wait();
    assert_eq!(LyricsState::Failed, session.state());

    provider.clear_pending();
    provider.for_query(&query_a()).set_result(found("a"));
    session.load(Some(&query_a())).wait();

    assert_eq!(LyricsState::Ready, session.state());
}

#[test]
fn provider_exception_becomes_failed_state() {
    let provider = ManualProvider::new();
    let logged: Arc<Mutex<Option<bool>>> = Arc::new(Mutex::new(None));
    let sink = Arc::clone(&logged);
    let session = LyricsSession::with_error_handler(provider.clone(), move |_, ex| {
        *sink.lock().unwrap() = Some(ex.is::<InvalidOperation>());
    });
    let load = session.load(Some(&query_a()));

    provider.for_query(&query_a()).set_exception("boom");
    load.wait();

    assert_eq!(LyricsState::Failed, session.state());
    // Assert.IsType<InvalidOperationException>(logged)
    assert_eq!(Some(true), *logged.lock().unwrap());
}

#[test]
fn suspend_cancels_loading_but_keeps_finished_lyrics() {
    let provider = ManualProvider::new();
    let session = LyricsSession::new(provider.clone());

    let _ = session.load(Some(&query_a()));
    session.suspend();
    assert_eq!(LyricsState::Idle, session.state());

    provider.for_query(&query_b()).set_result(found("b"));
    session.load(Some(&query_b())).wait();
    session.suspend();

    assert_eq!(LyricsState::Ready, session.state());
}

#[test]
fn null_query_clears() {
    let provider = ManualProvider::new();
    let session = LyricsSession::new(provider.clone());
    provider.for_query(&query_a()).set_result(found("a"));
    session.load(Some(&query_a())).wait();

    session.load(None).wait();

    assert_eq!(LyricsState::Idle, session.state());
    assert!(session.lines().is_empty());
}

#[test]
fn update_position_reports_only_real_changes() {
    let provider = ManualProvider::new();
    let session = LyricsSession::new(provider.clone());
    provider.for_query(&query_a()).set_result(found("a"));
    session.load(Some(&query_a())).wait();

    assert!(session.update_position(TimeSpan::from_secs(2)));
    assert!(!session.update_position(TimeSpan::from_secs(3)));
    assert_eq!(0, session.active_index());
}
