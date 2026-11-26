//! Everyday data (tasks, reminders, habits, prefs, stats) persisted as JSON in
//! %APPDATA%\deskpet\data.json.

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Task {
    pub id: u64,
    pub title: String,
    #[serde(default)]
    pub done: bool,
    #[serde(default)]
    pub done_on: Option<String>,
    /// "YYYY-MM-DD"
    #[serde(default)]
    pub due: Option<String>,
    #[serde(default = "default_est")]
    pub est_min: u32,
    /// 0 low, 1 normal, 2 high
    #[serde(default = "one")]
    pub priority: u8,
    /// Needs deep focus.
    #[serde(default)]
    pub hard: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Reminder {
    pub id: u64,
    pub text: String,
    /// "once" | "daily" | "every"
    pub kind: String,
    /// "HH:MM" for once/daily
    #[serde(default)]
    pub time: Option<String>,
    /// "YYYY-MM-DD" for once
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub every_min: u32,
    #[serde(default = "yes")]
    pub enabled: bool,
    /// unix seconds of last firing
    #[serde(default)]
    pub last_fired: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Habit {
    pub id: u64,
    pub name: String,
    /// optional nudge time "HH:MM"
    #[serde(default)]
    pub time: Option<String>,
    #[serde(default)]
    pub done_days: Vec<String>,
    #[serde(default)]
    pub nudged_on: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Prefs {
    pub name: String,
    /// hype | bro | coach | butler | chill | kawaii
    pub personality: String,
    pub sounds: bool,
    pub voice: bool,
    pub focus_block: Vec<String>,
    pub auto_minimize: bool,
    pub day_start: String,
    pub day_end: String,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            name: "Arpit".into(),
            personality: "hype".into(),
            sounds: true,
            voice: false,
            focus_block: ["youtube", "instagram", "reddit", "twitter", "x.com", "netflix", "facebook"]
                .map(String::from)
                .to_vec(),
            auto_minimize: true,
            day_start: "08:00".into(),
            day_end: "23:30".into(),
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DayStats {
    /// category -> seconds
    #[serde(default)]
    pub per_cat: HashMap<String, u32>,
    /// seconds of "code" per hour of day
    #[serde(default)]
    pub code_by_hour: Vec<u32>,
    #[serde(default)]
    pub tasks_done: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Store {
    #[serde(default)]
    pub tasks: Vec<Task>,
    #[serde(default)]
    pub reminders: Vec<Reminder>,
    #[serde(default)]
    pub habits: Vec<Habit>,
    #[serde(default)]
    pub prefs: Prefs,
    /// "YYYY-MM-DD" -> stats (last 30 days kept)
    #[serde(default)]
    pub stats: BTreeMap<String, DayStats>,
    #[serde(default)]
    pub next_id: u64,
}

fn default_est() -> u32 {
    30
}
fn one() -> u8 {
    1
}
fn yes() -> bool {
    true
}

pub fn path() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("deskpet").join("data.json")
}

impl Store {
    pub fn load() -> Self {
        let p = path();
        match std::fs::read_to_string(&p) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_else(|e| {
                crate::log(&format!("data.json unreadable ({e}); keeping a backup and starting fresh"));
                let _ = std::fs::copy(&p, p.with_extension("json.broken"));
                Store::default()
            }),
            Err(_) => Store::default(),
        }
    }

    /// Atomic write (tmp file + rename) so a crash never corrupts data.
    pub fn save(&self) {
        let p = path();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let tmp = p.with_extension("json.tmp");
        if let Ok(s) = serde_json::to_string_pretty(self) {
            if std::fs::write(&tmp, s).is_ok() {
                let _ = std::fs::rename(&tmp, &p);
            }
        }
    }

    pub fn new_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }

    pub fn day(&mut self, date: &str) -> &mut DayStats {
        let d = self.stats.entry(date.to_string()).or_default();
        if d.code_by_hour.len() != 24 {
            d.code_by_hour = vec![0; 24];
        }
        d
    }

    /// Drops stats older than 30 entries.
    pub fn prune(&mut self) {
        while self.stats.len() > 30 {
            let first = self.stats.keys().next().cloned();
            if let Some(k) = first {
                self.stats.remove(&k);
            }
        }
    }

    /// Top 3 hours of the day with the most coding over the stored history.
    pub fn best_hours(&self) -> Vec<u32> {
        let mut sum = [0u64; 24];
        for d in self.stats.values() {
            for (h, s) in d.code_by_hour.iter().enumerate().take(24) {
                sum[h] += *s as u64;
            }
        }
        let mut hours: Vec<u32> = (0..24).filter(|h| sum[*h as usize] > 600).collect();
        hours.sort_by_key(|h| std::cmp::Reverse(sum[*h as usize]));
        hours.truncate(3);
        hours.sort();
        hours
    }
}

impl Habit {
    pub fn done_on(&self, date: &str) -> bool {
        self.done_days.iter().any(|d| d == date)
    }

    /// Consecutive days done, counting back from today (or yesterday if today isn't done yet).
    pub fn streak(&self, today: &str) -> u32 {
        let mut d = if self.done_on(today) { today.to_string() } else { add_days(today, -1) };
        let mut n = 0;
        while self.done_on(&d) {
            n += 1;
            d = add_days(&d, -1);
        }
        n
    }
}

/// "HH:MM" -> minutes of day.
pub fn parse_hm(s: &str) -> Option<u32> {
    let (h, m) = s.trim().split_once(':')?;
    let (h, m) = (h.parse::<u32>().ok()?, m.parse::<u32>().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (m + if m > 2 { -3 } else { 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

/// "YYYY-MM-DD" + n days.
pub fn add_days(date: &str, n: i64) -> String {
    let parts: Vec<i64> = date.split('-').filter_map(|p| p.parse().ok()).collect();
    if parts.len() != 3 {
        return date.to_string();
    }
    let (y, m, d) = civil_from_days(days_from_civil(parts[0], parts[1], parts[2]) + n);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_math() {
        assert_eq!(add_days("2026-11-30", 1), "2026-12-01");
        assert_eq!(add_days("2026-01-01", -1), "2025-12-31");
        assert_eq!(add_days("2028-02-28", 1), "2028-02-29");
    }

    #[test]
    fn streaks() {
        let h = Habit {
            id: 1,
            name: "x".into(),
            time: None,
            done_days: vec!["2026-11-24".into(), "2026-11-25".into()],
            nudged_on: None,
        };
        assert_eq!(h.streak("2026-11-26"), 2); // today not done yet: streak still alive
        assert_eq!(h.streak("2026-11-27"), 0);
    }
}
