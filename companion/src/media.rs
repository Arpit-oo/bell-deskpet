//! Spotify now-playing via Windows' media session API (the same data the
//! volume flyout shows): title, artist, position, duration and album art.
//! No Spotify account / API keys needed.

use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};
use windows::Storage::Streams::DataReader;
use windows::Win32::System::WinRT::{RoInitialize, RO_INIT_MULTITHREADED};
use windows_sys::Win32::System::SystemInformation::GetSystemTimePreciseAsFileTime;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Track {
    pub playing: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub position_ms: u64,
    pub duration_ms: u64,
}

impl Track {
    /// Identity of the song (ignores playback position).
    pub fn key(&self) -> String {
        format!("{}\u{1f}{}", self.artist, self.title)
    }
}

pub struct Media {
    mgr: Manager,
}

fn now_filetime() -> i64 {
    let mut ft = windows_sys::Win32::Foundation::FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 };
    unsafe { GetSystemTimePreciseAsFileTime(&mut ft) };
    ((ft.dwHighDateTime as i64) << 32) | ft.dwLowDateTime as i64
}

impl Media {
    pub fn new() -> Option<Self> {
        unsafe {
            let _ = RoInitialize(RO_INIT_MULTITHREADED); // already-initialised is fine
        }
        let mgr = Manager::RequestAsync().ok()?.get().ok()?;
        Some(Self { mgr })
    }

    fn spotify_session(&self) -> Option<Session> {
        let sessions = self.mgr.GetSessions().ok()?;
        for s in sessions {
            let id = s.SourceAppUserModelId().map(|h| h.to_string_lossy()).unwrap_or_default();
            if id.to_lowercase().contains("spotify") {
                return Some(s);
            }
        }
        None
    }

    /// Current Spotify track, or None if Spotify has no media session.
    pub fn track(&self) -> Option<Track> {
        let s = self.spotify_session()?;
        let props = s.TryGetMediaPropertiesAsync().ok()?.get().ok()?;
        let title = props.Title().map(|h| h.to_string_lossy()).unwrap_or_default();
        if title.is_empty() {
            return None;
        }
        let playing = s
            .GetPlaybackInfo()
            .and_then(|p| p.PlaybackStatus())
            .map(|st| st == Status::Playing)
            .unwrap_or(false);

        let (mut pos, mut dur) = (0i64, 0i64);
        if let Ok(tl) = s.GetTimelineProperties() {
            pos = tl.Position().map(|t| t.Duration).unwrap_or(0);
            dur = tl.EndTime().map(|t| t.Duration).unwrap_or(0);
            // Position is a snapshot; extrapolate from when it was taken.
            if playing {
                if let Ok(updated) = tl.LastUpdatedTime() {
                    let elapsed = now_filetime() - updated.UniversalTime;
                    if elapsed > 0 && elapsed < 10 * 60 * 10_000_000 {
                        pos += elapsed;
                    }
                }
            }
            if dur > 0 {
                pos = pos.clamp(0, dur);
            }
        }

        Some(Track {
            playing,
            title,
            artist: props.Artist().map(|h| h.to_string_lossy()).unwrap_or_default(),
            album: props.AlbumTitle().map(|h| h.to_string_lossy()).unwrap_or_default(),
            position_ms: (pos / 10_000).max(0) as u64,
            duration_ms: (dur / 10_000).max(0) as u64,
        })
    }

    /// Raw album-art image bytes (JPEG/PNG) for the current Spotify track.
    pub fn thumbnail(&self) -> Option<Vec<u8>> {
        let s = self.spotify_session()?;
        let props = s.TryGetMediaPropertiesAsync().ok()?.get().ok()?;
        let thumb = props.Thumbnail().ok()?;
        let stream = thumb.OpenReadAsync().ok()?.get().ok()?;
        let size = stream.Size().ok()? as u32;
        if size == 0 || size > 8 * 1024 * 1024 {
            return None;
        }
        let reader = DataReader::CreateDataReader(&stream.GetInputStreamAt(0).ok()?).ok()?;
        reader.LoadAsync(size).ok()?.get().ok()?;
        let mut buf = vec![0u8; size as usize];
        reader.ReadBytes(&mut buf).ok()?;
        Some(buf)
    }
}
