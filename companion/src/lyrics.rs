//! Time-synced lyrics from LRCLIB (https://lrclib.net, free, no key).
//! Fetched on a background thread so the main loop never blocks.

use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::Duration;

use serde_json::Value;

/// (start ms, text) sorted by time. Empty text = instrumental gap.
pub type Lines = Vec<(u64, String)>;

pub struct Request {
    pub key: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration_s: u64,
}

pub struct Lyrics {
    tx: Sender<Request>,
    rx: Receiver<(String, Option<Lines>)>,
    cache: HashMap<String, Option<Lines>>,
    pending: Option<String>,
}

impl Lyrics {
    pub fn new() -> Self {
        let (req_tx, req_rx) = channel::<Request>();
        let (res_tx, res_rx) = channel();
        std::thread::spawn(move || {
            let agent = ureq::AgentBuilder::new()
                .timeout(Duration::from_secs(8))
                .user_agent("deskpet-companion/0.2")
                .build();
            for req in req_rx {
                let lines = fetch(&agent, &req);
                if res_tx.send((req.key, lines)).is_err() {
                    break;
                }
            }
        });
        Self { tx: req_tx, rx: res_rx, cache: HashMap::new(), pending: None }
    }

    /// Lyrics for a track if known; kicks off a fetch the first time it's seen.
    pub fn get(&mut self, req: Request) -> Option<&Lines> {
        while let Ok((key, lines)) = self.rx.try_recv() {
            if lines.is_some() {
                crate::log(&format!("lyrics found for {}", key.replace('\u{1f}', " - ")));
            }
            self.cache.insert(key.clone(), lines);
            if self.pending.as_deref() == Some(key.as_str()) {
                self.pending = None;
            }
        }
        if !self.cache.contains_key(&req.key) && self.pending.as_deref() != Some(req.key.as_str()) {
            self.pending = Some(req.key.clone());
            let _ = self.tx.send(req);
            return None;
        }
        self.cache.get(&req.key).and_then(|l| l.as_ref())
    }
}

/// Current line, next non-empty line, and how long the current line lasts (ms).
/// Shown slightly early to feel in sync.
pub fn at(lines: &Lines, pos_ms: u64) -> (String, String, u64) {
    let t = pos_ms + 250;
    let idx = match lines.binary_search_by(|(ms, _)| ms.cmp(&t)) {
        Ok(i) => Some(i),
        Err(0) => None,
        Err(i) => Some(i - 1),
    };
    let Some(i) = idx else {
        let first = lines.iter().find(|(_, s)| !s.is_empty()).map(|(_, s)| s.clone()).unwrap_or_default();
        return (String::new(), first, 0);
    };
    let cur = lines[i].1.clone();
    let next = lines[i + 1..].iter().find(|(_, s)| !s.is_empty()).map(|(_, s)| s.clone()).unwrap_or_default();
    let line_ms = lines.get(i + 1).map(|(ms, _)| ms.saturating_sub(lines[i].0)).unwrap_or(4000).min(15_000);
    (cur, next, line_ms)
}

fn fetch(agent: &ureq::Agent, r: &Request) -> Option<Lines> {
    let exact = agent
        .get("https://lrclib.net/api/get")
        .query("track_name", &r.title)
        .query("artist_name", &r.artist)
        .query("album_name", &r.album)
        .query("duration", &r.duration_s.to_string())
        .call()
        .ok()
        .and_then(|resp| resp.into_json::<Value>().ok());
    if let Some(lines) = exact.as_ref().and_then(synced) {
        return Some(lines);
    }

    // Fallback: fuzzy search, pick the closest-duration synced result.
    let results = agent
        .get("https://lrclib.net/api/search")
        .query("track_name", &r.title)
        .query("artist_name", &r.artist)
        .call()
        .ok()?
        .into_json::<Value>()
        .ok()?;
    let mut best: Option<(u64, Lines)> = None;
    for item in results.as_array()? {
        let Some(lines) = synced(item) else { continue };
        let d = item["duration"].as_f64().unwrap_or(0.0) as u64;
        let diff = d.abs_diff(r.duration_s);
        if best.as_ref().is_none_or(|(bd, _)| diff < *bd) {
            best = Some((diff, lines));
        }
    }
    best.filter(|(diff, _)| *diff <= 5 || r.duration_s == 0).map(|(_, l)| l)
}

fn synced(v: &Value) -> Option<Lines> {
    parse_lrc(v["syncedLyrics"].as_str()?)
}

/// Parses "[mm:ss.xx] text" lines (multiple tags per line allowed).
pub fn parse_lrc(lrc: &str) -> Option<Lines> {
    let mut out: Lines = Vec::new();
    for line in lrc.lines() {
        let mut rest = line.trim();
        let mut stamps = Vec::new();
        while let Some(stripped) = rest.strip_prefix('[') {
            let Some(end) = stripped.find(']') else { break };
            let tag = &stripped[..end];
            rest = stripped[end + 1..].trim_start();
            if let Some((m, s)) = tag.split_once(':') {
                if let (Ok(m), Ok(s)) = (m.parse::<u64>(), s.parse::<f64>()) {
                    stamps.push(m * 60_000 + (s * 1000.0) as u64);
                }
            }
        }
        for t in stamps {
            out.push((t, rest.to_string()));
        }
    }
    if out.is_empty() {
        return None;
    }
    out.sort_by_key(|(t, _)| *t);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_finds_lines() {
        let l = parse_lrc("[00:01.00] one\n[00:03.50] two\n[00:05.00]\n[00:06.00] three").unwrap();
        assert_eq!(l.len(), 4);
        assert_eq!(at(&l, 0), (String::new(), "one".into(), 0));
        assert_eq!(at(&l, 1200), ("one".into(), "two".into(), 2500));
        assert_eq!(at(&l, 5100), (String::new(), "three".into(), 1000));
        assert_eq!(at(&l, 9000), ("three".into(), String::new(), 4000));
    }
}
