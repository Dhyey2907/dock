//! Windows Global System Media Transport Controls (GSMTC) adapter.
//!
//! Fully event-driven: we subscribe to the session manager's `CurrentSessionChanged` and the
//! current session's media / playback / timeline change events. There is no polling; the page
//! counts the playback position on between timeline updates.

use std::{
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex, OnceLock,
    },
    time::Duration,
};

use base64::Engine;
use chrono::Utc;
use windows::{
    Foundation::TypedEventHandler,
    Media::Control::{
        GlobalSystemMediaTransportControlsSession as Session,
        GlobalSystemMediaTransportControlsSessionManager as Manager,
        GlobalSystemMediaTransportControlsSessionMediaProperties as MediaProperties,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as WinStatus,
    },
    Storage::Streams::DataReader,
    Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED},
};

use super::{
    display_name, filetime_to_utc, mime_for, ms_to_ticks, ticks_to_ms, MediaCapabilities, MediaCommand,
    MediaSink, MediaState, PlaybackStatus,
};
use crate::error::{AppError, AppResult};

/// Largest artwork we will send; real thumbnails are far smaller.
const MAX_ARTWORK_BYTES: u64 = 2 * 1024 * 1024;
/// A new session often isn't readable for a moment ("device is not ready"). Retry a few
/// times, briefly, instead of reporting "nothing playing" and never looking again.
const READ_RETRIES: u32 = 3;
const READ_RETRY_DELAY: Duration = Duration::from_millis(400);

fn platform(error: windows_core::Error) -> AppError {
    AppError::Platform(error.message())
}

struct Subscription {
    session: Session,
    tokens: [i64; 3],
}

struct Inner {
    manager: OnceLock<Manager>,
    sink: OnceLock<MediaSink>,
    subscription: Mutex<Option<Subscription>>,
    /// (track identity, artwork data URL) so artwork is read once per track.
    artwork_cache: Mutex<Option<(String, Option<String>)>>,
    /// Serializes state reads; WinRT raises change events on arbitrary threads.
    read_lock: Mutex<()>,
    /// Bumped on every change notification so stale retries give up.
    generation: AtomicU64,
    /// App id of the session most recently seen playing; kept on screen after it pauses.
    sticky: Mutex<Option<String>>,
    /// Play/pause watchers on *every* session, so one that starts playing while another is
    /// shown is noticed (Windows doesn't always raise `CurrentSessionChanged` for that).
    watchers: Mutex<Vec<(Session, i64)>>,
}

pub struct WindowsMediaPort {
    inner: Arc<Inner>,
}

impl WindowsMediaPort {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Inner {
                manager: OnceLock::new(),
                sink: OnceLock::new(),
                subscription: Mutex::new(None),
                artwork_cache: Mutex::new(None),
                read_lock: Mutex::new(()),
                generation: AtomicU64::new(0),
                sticky: Mutex::new(None),
                watchers: Mutex::new(Vec::new()),
            }),
        }
    }

    /// Begin watching sessions; `sink` hears every change. Returns at once.
    pub fn start(&self, sink: MediaSink) -> AppResult<()> {
        let _ = self.inner.sink.set(sink);
        let inner = self.inner.clone();
        // A dedicated MTA thread owns setup, so blocking WinRT calls never touch the UI thread.
        std::thread::Builder::new()
            .name("media-session".into())
            .spawn(move || {
                // SAFETY: initializes the WinRT apartment for this thread only.
                if let Err(error) = unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
                    tracing::debug!(error = %error.message(), "RoInitialize (already initialized?)");
                }
                if let Err(error) = inner.clone().connect() {
                    tracing::warn!(%error, "media sessions unavailable");
                    inner.emit(None);
                    return;
                }
                // Keep this thread (and its apartment) alive for the life of the app.
                loop {
                    std::thread::park();
                }
            })
            .map_err(AppError::Io)?;
        Ok(())
    }

    pub fn command(&self, command: MediaCommand) -> AppResult<()> {
        // Always act on the session the dock is showing, never on whatever Windows
        // currently considers "current" (they can differ, e.g. after a pause).
        let session = self
            .inner
            .bound_session()
            .ok_or_else(|| AppError::Platform("nothing to control".into()))?;
        let accepted = match command {
            MediaCommand::PlayPause => session.TryTogglePlayPauseAsync(),
            MediaCommand::Next => session.TrySkipNextAsync(),
            MediaCommand::Previous => session.TrySkipPreviousAsync(),
            MediaCommand::Seek { position_ms } => {
                session.TryChangePlaybackPositionAsync(ms_to_ticks(position_ms))
            }
        }
        .and_then(|op| op.get())
        .map_err(platform)?;
        if accepted {
            Ok(())
        } else {
            Err(AppError::Platform("the media app declined the request".into()))
        }
    }
}

impl Inner {
    fn connect(self: Arc<Self>) -> AppResult<()> {
        let manager = Manager::RequestAsync()
            .and_then(|op| op.get())
            .map_err(platform)?;
        let (current, added_removed) = (Arc::downgrade(&self), Arc::downgrade(&self));
        manager
            .CurrentSessionChanged(&TypedEventHandler::new(move |_, _| {
                if let Some(inner) = current.upgrade() {
                    inner.rebind();
                }
                Ok(())
            }))
            .map_err(platform)?;
        manager
            .SessionsChanged(&TypedEventHandler::new(move |_, _| {
                if let Some(inner) = added_removed.upgrade() {
                    inner.watch_all_sessions();
                    inner.rebind();
                }
                Ok(())
            }))
            .map_err(platform)?;
        let _ = self.manager.set(manager);
        self.watch_all_sessions();
        self.rebind();
        Ok(())
    }

    /// Re-attach a play/pause watcher to every live session (called when sessions come and go).
    fn watch_all_sessions(self: &Arc<Self>) {
        let mut watchers = self.watchers.lock().unwrap_or_else(|e| e.into_inner());
        for (session, token) in watchers.drain(..) {
            let _ = session.RemovePlaybackInfoChanged(token);
        }
        let Some(sessions) = self.manager.get().and_then(|m| m.GetSessions().ok()) else {
            return;
        };
        for session in sessions {
            let weak = Arc::downgrade(self);
            let token = session.PlaybackInfoChanged(&TypedEventHandler::new(move |_, _| {
                if let Some(inner) = weak.upgrade() {
                    inner.rebind();
                }
                Ok(())
            }));
            match token {
                Ok(token) => watchers.push((session, token)),
                Err(error) => tracing::debug!(error = %error.message(), "could not watch a media session"),
            }
        }
    }

    fn bound_session(&self) -> Option<Session> {
        let subscription = self.subscription.lock().unwrap_or_else(|e| e.into_inner());
        subscription.as_ref().map(|s| s.session.clone())
    }

    /// Pick the session to show (see [`choose_session`]) from all live sessions.
    fn select_session(&self) -> Option<Session> {
        let manager = self.manager.get()?;
        let sessions: Vec<Session> = manager.GetSessions().ok()?.into_iter().collect();
        let candidates: Vec<(String, bool)> = sessions
            .iter()
            .map(|s| {
                let id = s
                    .SourceAppUserModelId()
                    .map(|h| h.to_string_lossy())
                    .unwrap_or_default();
                let playing = s
                    .GetPlaybackInfo()
                    .and_then(|p| p.PlaybackStatus())
                    .is_ok_and(|status| status == WinStatus::Playing);
                (id, playing)
            })
            .collect();
        let bound = self
            .bound_session()
            .and_then(|s| s.SourceAppUserModelId().ok())
            .map(|h| h.to_string_lossy());
        let current = manager
            .GetCurrentSession()
            .and_then(|s| s.SourceAppUserModelId())
            .map(|h| h.to_string_lossy())
            .ok();
        let sticky = self.sticky.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let index = choose_session(
            &candidates,
            bound.as_deref(),
            sticky.as_deref(),
            current.as_deref(),
        )?;
        if candidates[index].1 {
            *self.sticky.lock().unwrap_or_else(|e| e.into_inner()) = Some(candidates[index].0.clone());
        }
        sessions.into_iter().nth(index)
    }

    /// Re-select the session to show, move change subscriptions to it if it changed, and
    /// publish its state.
    fn rebind(self: &Arc<Self>) {
        let selected = self.select_session();
        let selected_id = selected
            .as_ref()
            .and_then(|s| s.SourceAppUserModelId().ok())
            .map(|h| h.to_string_lossy());

        let mut subscription = self.subscription.lock().unwrap_or_else(|e| e.into_inner());
        let bound_id = subscription
            .as_ref()
            .and_then(|s| s.session.SourceAppUserModelId().ok())
            .map(|h| h.to_string_lossy());
        if bound_id != selected_id || subscription.is_none() {
            if let Some(old) = subscription.take() {
                let _ = old.session.RemoveMediaPropertiesChanged(old.tokens[0]);
                let _ = old.session.RemovePlaybackInfoChanged(old.tokens[1]);
                let _ = old.session.RemoveTimelinePropertiesChanged(old.tokens[2]);
            }
            if let Some(session) = selected {
                match self.subscribe(&session) {
                    Ok(tokens) => *subscription = Some(Subscription { session, tokens }),
                    Err(error) => tracing::warn!(error = %error.message(), "could not watch media session"),
                }
            }
        }
        drop(subscription);
        self.refresh();
    }

    fn subscribe(self: &Arc<Self>, session: &Session) -> windows_core::Result<[i64; 3]> {
        let (a, b, c) = (Arc::downgrade(self), Arc::downgrade(self), Arc::downgrade(self));
        Ok([
            session.MediaPropertiesChanged(&TypedEventHandler::new(move |_, _| {
                if let Some(inner) = a.upgrade() {
                    inner.refresh();
                }
                Ok(())
            }))?,
            // Play/pause can change which session should be on screen, so re-select.
            session.PlaybackInfoChanged(&TypedEventHandler::new(move |_, _| {
                if let Some(inner) = b.upgrade() {
                    inner.rebind();
                }
                Ok(())
            }))?,
            session.TimelinePropertiesChanged(&TypedEventHandler::new(move |_, _| {
                if let Some(inner) = c.upgrade() {
                    inner.refresh();
                }
                Ok(())
            }))?,
        ])
    }

    fn refresh(self: &Arc<Self>) {
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        self.refresh_attempt(generation, 0);
    }

    fn refresh_attempt(self: &Arc<Self>, generation: u64, attempt: u32) {
        let _guard = self.read_lock.lock().unwrap_or_else(|e| e.into_inner());
        if self.generation.load(Ordering::SeqCst) != generation {
            return; // a newer change is already being handled
        }
        let Some(session) = self.bound_session() else {
            self.emit(None);
            return;
        };
        match self.read_state(&session) {
            Ok(state) => self.emit(Some(state)),
            Err(error) if attempt < READ_RETRIES => {
                tracing::debug!(error = %error.message(), attempt, "media session not readable yet; retrying");
                let inner = self.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(READ_RETRY_DELAY);
                    inner.refresh_attempt(generation, attempt + 1);
                });
            }
            Err(error) => {
                // Still unreadable (e.g. the session closed mid-read): report nothing playing.
                tracing::debug!(error = %error.message(), "could not read media session");
                self.emit(None);
            }
        }
    }

    fn emit(&self, state: Option<MediaState>) {
        if let Some(sink) = self.sink.get() {
            sink(state);
        }
    }

    fn read_state(&self, session: &Session) -> windows_core::Result<MediaState> {
        // Some apps publish playback state before (or without) track metadata; that read then
        // fails with "device not ready". Treat metadata as optional rather than dropping the session.
        let properties = match session.TryGetMediaPropertiesAsync().and_then(|op| op.get()) {
            Ok(properties) => Some(properties),
            Err(error) => {
                tracing::debug!(error = %error.message(), "media session has no readable metadata");
                None
            }
        };
        let playback = session.GetPlaybackInfo()?;
        let controls = playback.Controls()?;
        let timeline = session.GetTimelineProperties()?;

        let status = match playback.PlaybackStatus()? {
            WinStatus::Playing => PlaybackStatus::Playing,
            WinStatus::Paused => PlaybackStatus::Paused,
            WinStatus::Stopped | WinStatus::Closed => PlaybackStatus::Stopped,
            _ => PlaybackStatus::Other,
        };
        let text = |read: fn(&MediaProperties) -> windows_core::Result<windows_core::HSTRING>| {
            properties
                .as_ref()
                .and_then(|p| read(p).ok())
                .map(|h| h.to_string_lossy())
                .unwrap_or_default()
        };
        let title = text(MediaProperties::Title);
        let artist = text(MediaProperties::Artist);
        let album = text(MediaProperties::AlbumTitle);

        let duration = ticks_to_ms(timeline.EndTime()?.Duration - timeline.StartTime()?.Duration);
        let position = ticks_to_ms(timeline.Position()?.Duration - timeline.StartTime()?.Duration);
        let has_timeline = duration > 0;

        let source_id = session.SourceAppUserModelId()?.to_string_lossy();
        Ok(MediaState {
            // The Start menu's name ("Prime Video") beats a guess from the id ("PWA").
            source_app: crate::apps::win::installed_app_name(&source_id)
                .unwrap_or_else(|| display_name(&source_id)),
            source_icon: format!("aumid:{source_id}"),
            artwork: properties
                .as_ref()
                .and_then(|p| self.artwork_for(p, &format!("{title}\u{1f}{artist}\u{1f}{album}"))),
            title,
            artist,
            album,
            status,
            position_ms: has_timeline.then_some(position.min(duration)),
            duration_ms: has_timeline.then_some(duration),
            position_updated_at: filetime_to_utc(timeline.LastUpdatedTime()?.UniversalTime)
                .unwrap_or_else(Utc::now),
            // Some apps report 0 (or nothing) while playing; that would freeze the progress bar.
            rate: playback
                .PlaybackRate()
                .and_then(|r| r.Value())
                .ok()
                .filter(|rate| *rate > 0.0)
                .unwrap_or(1.0),
            capabilities: MediaCapabilities {
                play_pause: controls.IsPlayPauseToggleEnabled()?,
                next: controls.IsNextEnabled()?,
                previous: controls.IsPreviousEnabled()?,
                seek: controls.IsPlaybackPositionEnabled()? && has_timeline,
            },
        })
    }

    /// Artwork for the current track as a data URL; the thumbnail is read only when the track
    /// changes.
    fn artwork_for(&self, properties: &MediaProperties, track: &str) -> Option<String> {
        let mut cache = self.artwork_cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some((cached_track, url)) = cache.as_ref() {
            if cached_track == track {
                return url.clone();
            }
        }
        let url = match read_thumbnail(properties) {
            Ok((bytes, mime)) => Some(format!(
                "data:{mime};base64,{}",
                base64::engine::general_purpose::STANDARD.encode(bytes)
            )),
            Err(error) => {
                tracing::debug!(%error, "no artwork for current track");
                None
            }
        };
        // Some apps publish artwork a moment after the title; don't cache a miss forever.
        if url.is_some() {
            *cache = Some((track.to_owned(), url.clone()));
        }
        url
    }
}

fn read_thumbnail(properties: &MediaProperties) -> AppResult<(Vec<u8>, &'static str)> {
    // Sessions without artwork return a null reference, surfaced as an "error" with S_OK.
    let reference = properties
        .Thumbnail()
        .map_err(|_| AppError::InvalidInput("session has no artwork".into()))?;
    let stream = reference
        .OpenReadAsync()
        .and_then(|op| op.get())
        .map_err(platform)?;
    let size = stream.Size().map_err(platform)?;
    if size == 0 || size > MAX_ARTWORK_BYTES {
        return Err(AppError::InvalidInput(format!(
            "artwork size {size} out of range"
        )));
    }
    let content_type = stream
        .ContentType()
        .map(|t| t.to_string_lossy())
        .unwrap_or_default();
    let reader = DataReader::CreateDataReader(&stream).map_err(platform)?;
    reader
        .LoadAsync(size as u32)
        .and_then(|op| op.get())
        .map_err(platform)?;
    let mut bytes = vec![0u8; size as usize];
    reader.ReadBytes(&mut bytes).map_err(platform)?;
    let mime = mime_for(&content_type, &bytes);
    Ok((bytes, mime))
}

/// Choose which session to show from `(app id, is_playing)` candidates:
/// 1. something playing — preferring what is already shown, then the last one seen playing,
///    then Windows' current session;
/// 2. otherwise the last session seen playing, so a paused app stays put for resuming;
/// 3. otherwise what is already shown, then Windows' current session, then the first.
pub fn choose_session(
    candidates: &[(String, bool)],
    bound: Option<&str>,
    sticky: Option<&str>,
    current: Option<&str>,
) -> Option<usize> {
    let find = |id: Option<&str>, must_play: bool| {
        let id = id?;
        candidates
            .iter()
            .position(|(c, playing)| c == id && (!must_play || *playing))
    };
    find(bound, true)
        .or_else(|| find(sticky, true))
        .or_else(|| find(current, true))
        .or_else(|| candidates.iter().position(|(_, playing)| *playing))
        .or_else(|| find(sticky, false))
        .or_else(|| find(bound, false))
        .or_else(|| find(current, false))
        .or(if candidates.is_empty() { None } else { Some(0) })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn c(list: &[(&str, bool)]) -> Vec<(String, bool)> {
        list.iter()
            .map(|(id, playing)| (id.to_string(), *playing))
            .collect()
    }

    #[test]
    fn prefers_a_playing_session() {
        let sessions = c(&[("netflix", false), ("edge", true)]);
        assert_eq!(
            choose_session(&sessions, Some("netflix"), None, Some("netflix")),
            Some(1)
        );
    }

    #[test]
    fn keeps_a_paused_session_that_was_playing_instead_of_jumping() {
        // Edge was playing and is now paused; Windows moved "current" to an old Netflix session.
        let sessions = c(&[("netflix", false), ("edge", false)]);
        assert_eq!(
            choose_session(&sessions, Some("edge"), Some("edge"), Some("netflix")),
            Some(1)
        );
    }

    #[test]
    fn keeps_showing_the_bound_session_when_several_play() {
        let sessions = c(&[("spotify", true), ("edge", true)]);
        assert_eq!(
            choose_session(&sessions, Some("edge"), Some("spotify"), Some("spotify")),
            Some(1)
        );
    }

    #[test]
    fn falls_back_to_windows_current_then_first() {
        let sessions = c(&[("a", false), ("b", false)]);
        assert_eq!(choose_session(&sessions, None, None, Some("b")), Some(1));
        assert_eq!(choose_session(&sessions, None, None, None), Some(0));
        assert_eq!(choose_session(&[], None, None, None), None);
    }
}
