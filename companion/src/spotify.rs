//! Spotify now-playing without any API keys: the desktop app puts
//! "Artist - Track" in its window title while playing, and "Spotify ..." when paused.

use crate::win::{self, ExeCache};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Music {
    pub playing: bool,
    pub track: String,
    pub artist: String,
}

const IGNORED_TITLES: &[&str] = &["GDI+ Window", "MSCTFIME UI", "Default IME", "Chrome Legacy Window"];

pub fn now_playing(cache: &mut ExeCache) -> Music {
    let titles = win::titles_for_exe("spotify.exe", cache);
    for t in &titles {
        if IGNORED_TITLES.iter().any(|ig| t.starts_with(ig)) || t.starts_with("Spotify") {
            continue;
        }
        if let Some((artist, track)) = t.split_once(" - ") {
            return Music { playing: true, track: track.trim().to_string(), artist: artist.trim().to_string() };
        }
    }
    Music::default()
}
