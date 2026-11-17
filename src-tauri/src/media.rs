//! Now Playing: the system media session (SMTC) that Spotify, browsers and players publish.

use crate::worker::{com_mta, Worker};
use base64::Engine;
use serde::Serialize;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};
use windows::Storage::Streams::DataReader;

#[derive(Serialize, Clone, PartialEq, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct MediaInfo {
    /// AppUserModelID of the app playing, and a friendly name and icon key for it.
    pub source: String,
    pub source_name: String,
    pub source_icon: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub playing: bool,
    pub can_prev: bool,
    pub can_next: bool,
    pub can_play_pause: bool,
    pub can_seek: bool,
    /// Seconds into the track at `updated_at` (unix ms), and the track length; 0 if unknown.
    pub position: f64,
    pub duration: f64,
    pub updated_at: f64,
    /// Cover art as a data URL.
    pub art: Option<String>,
}

static WORKER: OnceLock<Worker> = OnceLock::new();
static CURRENT: Mutex<Option<MediaInfo>> = Mutex::new(None);
static APP: OnceLock<AppHandle> = OnceLock::new();
/// After a play/pause click the icon flips at once; players can take a second to report the
/// new status, so hold the expected value until they agree (or a short timeout passes).
static EXPECT: Mutex<Option<(bool, Instant)>> = Mutex::new(None);
const EXPECT_FOR: Duration = Duration::from_millis(2500);
/// After a seek: the position asked for, when (unix ms), and when the hold started. Many
/// players only refresh their reported position every few seconds, so trust the seek until
/// they report something newer.
static SEEKED: Mutex<Option<(f64, f64, Instant)>> = Mutex::new(None);
const SEEK_FOR: Duration = Duration::from_millis(4000);

fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0)
}

struct Ctx {
    manager: Option<Manager>,
    art_key: String,
    art: Option<String>,
}

thread_local! {
    static CTX: std::cell::RefCell<Ctx> = const { std::cell::RefCell::new(Ctx { manager: None, art_key: String::new(), art: None }) };
}

pub fn start(app: AppHandle) {
    let _ = APP.set(app);
    WORKER.get_or_init(|| Worker::spawn(com_mta, Duration::from_millis(500), refresh));
}

fn refresh() {
    let mut info = read().ok().flatten();
    if let Some(i) = info.as_mut() {
        let mut ex = EXPECT.lock().unwrap();
        match *ex {
            Some((want, since)) if since.elapsed() < EXPECT_FOR && i.playing != want => i.playing = want,
            Some(_) => *ex = None,
            None => {}
        }
        let mut sk = SEEKED.lock().unwrap();
        if let Some((pos, at, since)) = *sk {
            if since.elapsed() < SEEK_FOR && i.updated_at < at {
                i.position = pos;
                i.updated_at = at;
            } else {
                *sk = None;
            }
        }
    }
    publish(info);
}

fn publish(info: Option<MediaInfo>) {
    let mut cur = CURRENT.lock().unwrap();
    if *cur != info {
        *cur = info.clone();
        if let Some(app) = APP.get() {
            let _ = app.emit("media-changed", info);
        }
    }
}

pub fn current() -> Option<MediaInfo> {
    CURRENT.lock().unwrap().clone()
}

fn session() -> windows::core::Result<Option<Session>> {
    CTX.with(|c| {
        let mut c = c.borrow_mut();
        if c.manager.is_none() {
            c.manager = Some(Manager::RequestAsync()?.get()?);
        }
        Ok(c.manager.as_ref().unwrap().GetCurrentSession().ok())
    })
}

fn source_name(aumid: &str) -> String {
    crate::icons::display_name(&format!("shell:AppsFolder\\{aumid}")).unwrap_or_else(|| {
        let s = aumid.rsplit(['\\', '!']).next().unwrap_or(aumid);
        s.trim_end_matches(".exe").to_string()
    })
}

fn read() -> windows::core::Result<Option<MediaInfo>> {
    let Some(s) = session()? else { return Ok(None) };
    let props = s.TryGetMediaPropertiesAsync()?.get()?;
    let info = s.GetPlaybackInfo()?;
    let controls = info.Controls()?;
    let timeline = s.GetTimelineProperties()?;
    let source = s.SourceAppUserModelId()?.to_string();

    let ticks = |t: windows::Foundation::TimeSpan| t.Duration as f64 / 10_000_000.0;
    let start = ticks(timeline.StartTime()?);
    let end = ticks(timeline.EndTime()?);
    let position = (ticks(timeline.Position()?) - start).max(0.0);
    let updated = timeline.LastUpdatedTime()?.UniversalTime;
    // 100 ns ticks since 1601 -> unix ms.
    let updated_at = (updated - 116_444_736_000_000_000) as f64 / 10_000.0;

    let title = props.Title()?.to_string();
    let artist = props.Artist()?.to_string();
    let album = props.AlbumTitle()?.to_string();

    // Cover art only changes with the track; fetch it once per track.
    let key = format!("{source}|{title}|{artist}|{album}");
    let art = CTX.with(|c| {
        let mut c = c.borrow_mut();
        if c.art_key != key {
            c.art_key = key;
            c.art = props.Thumbnail().ok().and_then(|t| read_art(&t).ok());
        }
        c.art.clone()
    });

    if title.is_empty() && artist.is_empty() {
        return Ok(None);
    }
    Ok(Some(MediaInfo {
        source_name: source_name(&source),
        source_icon: format!("aumid:{source}"),
        source,
        title,
        artist,
        album,
        playing: info.PlaybackStatus()? == Status::Playing,
        can_prev: controls.IsPreviousEnabled()?,
        can_next: controls.IsNextEnabled()?,
        can_play_pause: controls.IsPlayPauseToggleEnabled()? || controls.IsPlayEnabled()?,
        can_seek: controls.IsPlaybackPositionEnabled().unwrap_or(false),
        position,
        duration: (end - start).max(0.0),
        updated_at,
        art,
    }))
}

fn read_art(r: &windows::Storage::Streams::IRandomAccessStreamReference) -> windows::core::Result<String> {
    let stream = r.OpenReadAsync()?.get()?;
    let size = stream.Size()? as u32;
    let ct = stream.ContentType().map(|c| c.to_string()).unwrap_or_default();
    let reader = DataReader::CreateDataReader(&stream)?;
    reader.LoadAsync(size)?.get()?;
    let mut buf = vec![0u8; size as usize];
    reader.ReadBytes(&mut buf)?;
    let ct = if ct.starts_with("image/") { ct } else { "image/png".into() };
    Ok(format!("data:{ct};base64,{}", base64::engine::general_purpose::STANDARD.encode(buf)))
}

/// Jump to `seconds` into the current track.
pub fn seek(seconds: f64) {
    let Some(w) = WORKER.get() else { return };
    w.run(move || {
        let Ok(Some(s)) = session() else { return };
        let Ok(start) = s.GetTimelineProperties().and_then(|t| t.StartTime()) else { return };
        let target = start.Duration + (seconds.max(0.0) * 10_000_000.0) as i64;
        match s.TryChangePlaybackPositionAsync(target).and_then(|op| op.get()) {
            Ok(true) => {
                let at = now_ms();
                *SEEKED.lock().unwrap() = Some((seconds, at, Instant::now()));
                let cur = CURRENT.lock().unwrap().clone();
                if let Some(mut i) = cur {
                    i.position = seconds;
                    i.updated_at = at;
                    publish(Some(i));
                }
            }
            Ok(false) => crate::dlog!("media seek: refused by the player"),
            Err(e) => crate::dlog!("media seek: {e}"),
        }
    });
}

/// Play/pause, next or previous on the current session.
pub fn command(action: String) {
    let Some(w) = WORKER.get() else { return };
    w.run(move || {
        let Ok(Some(s)) = session() else { return };
        let r = match action.as_str() {
            "playPause" => s.TryTogglePlayPauseAsync().and_then(|op| op.get()),
            "next" => s.TrySkipNextAsync().and_then(|op| op.get()),
            "prev" => s.TrySkipPreviousAsync().and_then(|op| op.get()),
            _ => return,
        };
        match r {
            Ok(true) if action == "playPause" => {
                // Flip the icon now instead of waiting for the player to report it.
                let cur = CURRENT.lock().unwrap().clone();
                if let Some(mut i) = cur {
                    i.playing = !i.playing;
                    *EXPECT.lock().unwrap() = Some((i.playing, Instant::now()));
                    publish(Some(i));
                }
            }
            Ok(_) => {}
            Err(e) => crate::dlog!("media {action}: {e}"),
        }
        refresh();
    });
}
