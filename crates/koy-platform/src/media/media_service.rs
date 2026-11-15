//! Windows Global System Media Transport Controls (GSMTC) servis uygulaması.
//! Spotify, Chrome, Edge ve diğer medya oynatıcılarını dinler ve yönetir.

use std::sync::{Arc, Mutex, MutexGuard, Weak};
use std::time::Duration;

use koy_core::event::Event;
use koy_core::media::{IMediaService, MediaInfo, MediaSessionInfo, PlaybackTimeline};
use koy_core::time::{DateTimeOffset, TimeSpan};
use koy_core::{text, win_path};
use windows::Foundation::{DateTime as WinDateTime, TypedEventHandler};
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session, GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
};
use windows::Storage::Streams::{DataReader, IRandomAccessStreamReference};
use windows::core::HSTRING;

use crate::bluetooth::support::ComApartment;

struct Bound {
    session: Session,
    tokens: [i64; 3],
}

struct State {
    manager: Option<(Manager, [i64; 2])>,
    current_session: Option<Bound>,
    selected_session_id: Option<String>,
    disposed: bool,
    current: Option<MediaInfo>,
    timeline: Option<PlaybackTimeline>,
    sessions: Vec<MediaSessionInfo>,
}

// SAFETY: WinRT nesneleri çeviktir; yalnız kilit altında kullanılır.
unsafe impl Send for State {}

struct Inner {
    state: Mutex<State>,
    changed: Event<Option<MediaInfo>>,
    timeline_changed: Event<Option<PlaybackTimeline>>,
    sessions_changed: Event<Vec<MediaSessionInfo>>,
}

pub struct MediaService {
    inner: Arc<Inner>,
}

impl Default for MediaService {
    fn default() -> Self {
        MediaService {
            inner: Arc::new(Inner {
                state: Mutex::new(State {
                    manager: None,
                    current_session: None,
                    selected_session_id: None,
                    disposed: false,
                    current: None,
                    timeline: None,
                    sessions: Vec::new(),
                }),
                changed: Event::new(),
                timeline_changed: Event::new(),
                sessions_changed: Event::new(),
            }),
        }
    }
}

/// WinRT `DateTime` (1601'den beri 100 ns) → `DateTimeOffset` (UTC).
fn to_offset(dt: WinDateTime) -> DateTimeOffset {
    const UNIX_EPOCH_TICKS: i64 = 116_444_736_000_000_000;
    let ticks = dt.UniversalTime - UNIX_EPOCH_TICKS;
    chrono::DateTime::from_timestamp(ticks.div_euclid(10_000_000), (ticks.rem_euclid(10_000_000) * 100) as u32)
        .unwrap_or_default()
        .fixed_offset()
}

fn clean_session_title(app_id: &str) -> String {
    if app_id.trim().is_empty() {
        return "Medya".to_string();
    }
    let lower = text::to_lower_invariant(app_id);
    let known = [
        ("spotify", "Spotify"),
        ("chrome", "Chrome"),
        ("edge", "Edge"),
        ("msedge", "Edge"),
        ("firefox", "Firefox"),
        ("brave", "Brave"),
        ("opera", "Opera"),
        ("vlc", "VLC"),
        ("musicbee", "MusicBee"),
        ("foobar", "foobar2000"),
        ("apple", "Apple Music"),
        ("itunes", "Apple Music"),
        ("wmplayer", "Media Player"),
        ("mediaplayer", "Media Player"),
    ];
    if let Some((_, name)) = known.iter().find(|(k, _)| lower.contains(k)) {
        return (*name).to_string();
    }
    let last = app_id.split('!').next_back().unwrap_or(app_id);
    let raw = win_path::get_file_name_without_extension(last);
    if raw.trim().is_empty() { "Medya".to_string() } else { raw.to_string() }
}

fn app_id_of(session: &Session) -> String {
    session.SourceAppUserModelId().map(|h| h.to_string()).unwrap_or_default()
}

fn is_playing(session: &Session) -> bool {
    session
        .GetPlaybackInfo()
        .and_then(|p| p.PlaybackStatus())
        .is_ok_and(|s| s == PlaybackStatus::Playing)
}

/// Kapak baytları; 2 sn içinde okunamazsa `None`.
fn read_thumbnail(reference: IRandomAccessStreamReference) -> Option<Vec<u8>> {
    let (tx, rx) = std::sync::mpsc::channel();
    let agile = windows::core::AgileReference::new(&reference).ok()?;
    std::thread::spawn(move || {
        let _com = ComApartment::enter_mta();
        let read = (|| -> windows::core::Result<Vec<u8>> {
            let reference = agile.resolve()?;
            let stream = reference.OpenReadAsync()?.join()?;
            let size = stream.Size()? as u32;
            let reader = DataReader::CreateDataReader(&stream)?;
            let loaded = reader.LoadAsync(size)?.join()?;
            let mut bytes = vec![0u8; loaded as usize];
            reader.ReadBytes(&mut bytes)?;
            Ok(bytes)
        })();
        let _ = tx.send(read.ok());
    });
    rx.recv_timeout(Duration::from_millis(2000)).ok().flatten()
}

impl Inner {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn manager(&self) -> Option<Manager> {
        self.lock().manager.as_ref().map(|(m, _)| m.clone())
    }

    fn target_session(&self) -> Option<Session> {
        let st = self.lock();
        st.current_session.as_ref().map(|b| b.session.clone()).or_else(|| st.manager.as_ref().and_then(|(m, _)| m.GetCurrentSession().ok()))
    }

    fn refresh_session_list(self: &Arc<Self>) {
        let Some(manager) = self.manager() else {
            self.lock().sessions = Vec::new();
            self.sessions_changed.invoke(&Vec::new());
            return;
        };
        let Ok(raw) = manager.GetSessions() else { return };
        let current_id = self.lock().current_session.as_ref().map(|b| app_id_of(&b.session));
        let selected = self.lock().selected_session_id.clone();
        let mut list = Vec::new();
        let mut has_selected = false;
        for s in &raw {
            let app_id = app_id_of(&s);
            let is_current = current_id.as_deref() == Some(app_id.as_str());
            list.push(MediaSessionInfo { id: app_id.clone(), source_app_id: app_id.clone(), title: clean_session_title(&app_id), is_current });
            if selected.as_deref() == Some(app_id.as_str()) {
                has_selected = true;
            }
        }
        // Seçilen oturum kapanmışsa otomatik takibe dön.
        if selected.is_some() && !has_selected {
            self.lock().selected_session_id = None;
            self.bind_to_session(manager.GetCurrentSession().ok());
        }
        self.lock().sessions = list.clone();
        self.sessions_changed.invoke(&list);
    }

    fn bind_to_session(self: &Arc<Self>, session: Option<Session>) {
        {
            let mut st = self.lock();
            if let Some(old) = st.current_session.take() {
                let _ = old.session.RemoveMediaPropertiesChanged(old.tokens[0]);
                let _ = old.session.RemovePlaybackInfoChanged(old.tokens[1]);
                let _ = old.session.RemoveTimelinePropertiesChanged(old.tokens[2]);
            }
        }
        match session {
            Some(session) => {
                let weak: Weak<Inner> = Arc::downgrade(self);
                let (w1, w2, w3) = (weak.clone(), weak.clone(), weak);
                let t1 = session
                    .MediaPropertiesChanged(&TypedEventHandler::new(move |_, _| {
                        if let Some(i) = w1.upgrade() {
                            i.update_media_properties();
                        }
                        Ok(())
                    }))
                    .unwrap_or(0);
                let t2 = session
                    .PlaybackInfoChanged(&TypedEventHandler::new(move |_, _| {
                        if let Some(i) = w2.upgrade() {
                            i.update_playback_info();
                        }
                        Ok(())
                    }))
                    .unwrap_or(0);
                let t3 = session
                    .TimelinePropertiesChanged(&TypedEventHandler::new(move |_, _| {
                        if let Some(i) = w3.upgrade() {
                            i.update_timeline_info();
                        }
                        Ok(())
                    }))
                    .unwrap_or(0);
                self.lock().current_session = Some(Bound { session, tokens: [t1, t2, t3] });
                self.update_media_properties();
                self.update_playback_info();
                self.update_timeline_info();
            }
            None => {
                {
                    let mut st = self.lock();
                    st.current = None;
                    st.timeline = None;
                }
                self.changed.invoke(&None);
                self.timeline_changed.invoke(&None);
            }
        }
    }

    fn update_media_properties(&self) {
        let session = self.lock().current_session.as_ref().map(|b| b.session.clone());
        let Some(session) = session else {
            self.lock().current = None;
            self.changed.invoke(&None);
            return;
        };
        // Okuma sırasında oturum kapanmış olabilir: sessizce geç.
        let Ok(props) = session.TryGetMediaPropertiesAsync().and_then(|op| op.join()) else { return };
        let thumbnail = props.Thumbnail().ok().and_then(read_thumbnail);
        let info = MediaInfo {
            title: props.Title().map(|h| h.to_string()).unwrap_or_default(),
            artist: props.Artist().map(|h| h.to_string()).unwrap_or_default(),
            source_app_id: app_id_of(&session),
            is_playing: is_playing(&session),
            thumbnail,
        };
        self.lock().current = Some(info.clone());
        self.changed.invoke(&Some(info));
    }

    fn update_playback_info(&self) {
        let updated = {
            let mut st = self.lock();
            let Some(session) = st.current_session.as_ref().map(|b| b.session.clone()) else { return };
            let Some(current) = st.current.as_mut() else { return };
            let playing = is_playing(&session);
            if current.is_playing == playing {
                return;
            }
            current.is_playing = playing;
            current.clone()
        };
        self.changed.invoke(&Some(updated));
    }

    fn update_timeline_info(&self) {
        let session = self.lock().current_session.as_ref().map(|b| b.session.clone());
        let Some(session) = session else {
            self.lock().timeline = None;
            self.timeline_changed.invoke(&None);
            return;
        };
        // Zaman çizelgesi okunamazsa sessiz geç.
        let Ok(t) = session.GetTimelineProperties() else { return };
        let read = (|| -> windows::core::Result<PlaybackTimeline> {
            Ok(PlaybackTimeline {
                position: TimeSpan::from_ticks(t.Position()?.Duration),
                start_time: TimeSpan::from_ticks(t.StartTime()?.Duration),
                end_time: TimeSpan::from_ticks(t.EndTime()?.Duration),
                last_updated: to_offset(t.LastUpdatedTime()?),
            })
        })();
        if let Ok(timeline) = read {
            self.lock().timeline = Some(timeline.clone());
            self.timeline_changed.invoke(&Some(timeline));
        }
    }

    fn dispose(&self) {
        let mut st = self.lock();
        if st.disposed {
            return;
        }
        st.disposed = true;
        if let Some(old) = st.current_session.take() {
            let _ = old.session.RemoveMediaPropertiesChanged(old.tokens[0]);
            let _ = old.session.RemovePlaybackInfoChanged(old.tokens[1]);
            let _ = old.session.RemoveTimelinePropertiesChanged(old.tokens[2]);
        }
        if let Some((manager, tokens)) = st.manager.take() {
            let _ = manager.RemoveCurrentSessionChanged(tokens[0]);
            let _ = manager.RemoveSessionsChanged(tokens[1]);
        }
    }
}

impl Drop for MediaService {
    fn drop(&mut self) {
        self.inner.dispose();
    }
}

impl IMediaService for MediaService {
    fn changed(&self) -> &Event<Option<MediaInfo>> {
        &self.inner.changed
    }

    fn timeline_changed(&self) -> &Event<Option<PlaybackTimeline>> {
        &self.inner.timeline_changed
    }

    fn sessions_changed(&self) -> &Event<Vec<MediaSessionInfo>> {
        &self.inner.sessions_changed
    }

    fn current(&self) -> Option<MediaInfo> {
        self.inner.lock().current.clone()
    }

    fn timeline(&self) -> Option<PlaybackTimeline> {
        self.inner.lock().timeline.clone()
    }

    fn sessions(&self) -> Vec<MediaSessionInfo> {
        self.inner.lock().sessions.clone()
    }

    fn start(&self) {
        // Windows GSMTC kullanılamıyorsa uygulama çökmemeli.
        let Ok(manager) = Manager::RequestAsync().and_then(|op| op.join()) else { return };
        let weak = Arc::downgrade(&self.inner);
        let w1 = weak.clone();
        let t1 = manager
            .CurrentSessionChanged(&TypedEventHandler::new(move |sender: windows::core::Ref<Manager>, _| -> windows::core::Result<()> {
                if let (Some(inner), Some(sender)) = (w1.upgrade(), sender.as_ref())
                    && inner.lock().selected_session_id.is_none()
                {
                    inner.bind_to_session(sender.GetCurrentSession().ok());
                    inner.refresh_session_list();
                }
                Ok(())
            }))
            .unwrap_or(0);
        let t2 = manager
            .SessionsChanged(&TypedEventHandler::new(move |_, _| {
                if let Some(inner) = weak.upgrade() {
                    inner.refresh_session_list();
                }
                Ok(())
            }))
            .unwrap_or(0);
        let current = manager.GetCurrentSession().ok();
        self.inner.lock().manager = Some((manager, [t1, t2]));
        self.inner.refresh_session_list();
        self.inner.bind_to_session(current);
    }

    fn stop(&self) {
        self.inner.dispose();
    }

    fn play_pause(&self) {
        if let Some(s) = self.inner.target_session() {
            let _ = s.TryTogglePlayPauseAsync().and_then(|op| op.join());
        }
    }

    fn next(&self) {
        if let Some(s) = self.inner.target_session() {
            let _ = s.TrySkipNextAsync().and_then(|op| op.join());
        }
    }

    fn previous(&self) {
        if let Some(s) = self.inner.target_session() {
            let _ = s.TrySkipPreviousAsync().and_then(|op| op.join());
        }
    }

    fn seek(&self, position: TimeSpan) {
        if let Some(s) = self.inner.target_session() {
            let _ = s.TryChangePlaybackPositionAsync(position.ticks()).and_then(|op| op.join());
        }
    }

    fn select_session(&self, session_id: &str) {
        let Some(manager) = self.inner.manager() else { return };
        // Halihazırda seçili oturuma tekrar tıklanırsa otomatik moda dön.
        if self.inner.lock().selected_session_id.as_deref() == Some(session_id) {
            self.inner.lock().selected_session_id = None;
            self.inner.bind_to_session(manager.GetCurrentSession().ok());
            self.inner.refresh_session_list();
            return;
        }
        self.inner.lock().selected_session_id = Some(session_id.to_string());
        let target = manager.GetSessions().ok().and_then(|all| all.into_iter().find(|s| s.SourceAppUserModelId().is_ok_and(|id| id == HSTRING::from(session_id))));
        self.inner.bind_to_session(target);
        self.inner.refresh_session_list();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_session_titles() {
        assert_eq!(clean_session_title("Spotify.exe"), "Spotify");
        assert_eq!(clean_session_title("Microsoft.ZuneMusic_8wekyb3d8bbwe!Microsoft.ZuneMusic"), "Microsoft"); // C# ile aynı: GetFileNameWithoutExtension son noktayı atar
        assert_eq!(clean_session_title(""), "Medya");
        assert_eq!(clean_session_title(r"C:\Apps\Player.exe"), "Player");
    }

    #[test]
    fn converts_winrt_datetime() {
        let t = to_offset(WinDateTime { UniversalTime: 116_444_736_000_000_000 + 10_000_000 });
        assert_eq!(t.timestamp(), 1);
    }

    #[test]
    fn starts_and_stops() {
        let service = MediaService::default();
        service.start();
        let _ = service.sessions();
        service.stop();
    }
}
