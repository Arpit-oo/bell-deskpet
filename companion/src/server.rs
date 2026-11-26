//! Local everyday app: http://localhost:7777 (bound to 127.0.0.1 only).
//! Serves the UI, a small JSON API over the store, and proxies /pet/* to the robot.

use std::io::Read;
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{json, Value};
use tiny_http::{Header, Method, Request, Response, Server};

use crate::planner;
use crate::sound::{self, Sound};
use crate::store::{add_days, parse_hm, Habit, Reminder, Store, Task};
use crate::win;

pub const PORT: u16 = 7777;
const APP_HTML: &str = include_str!("app.html");

#[derive(Default, Clone)]
pub struct Live {
    pub label: String,
    pub category: String,
    pub minutes: u64,
    pub playing: bool,
    pub track: String,
    pub artist: String,
    pub pet_online: bool,
    /// unix seconds when focus mode ends (0 = off)
    pub focus_until: u64,
}

pub struct Shared {
    pub store: Store,
    pub live: Live,
}

pub type SharedRef = Arc<Mutex<Shared>>;

pub enum AppEvent {
    TaskDone(String),
    Claude { state: String, project: String },
    Say(String),
    FocusStarted(u32),
}

pub fn unix_now() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

pub fn start(shared: SharedRef, tx: Sender<AppEvent>, pet_host: String) {
    let server = match Server::http(("127.0.0.1", PORT)) {
        Ok(s) => s,
        Err(e) => {
            crate::log(&format!("app server failed to start on {PORT}: {e}"));
            return;
        }
    };
    crate::log(&format!("app at http://localhost:{PORT}"));
    let proxy = ureq::AgentBuilder::new().timeout(Duration::from_secs(4)).build();
    std::thread::spawn(move || {
        for req in server.incoming_requests() {
            handle(req, &shared, &tx, &proxy, &pet_host);
        }
    });
}

fn json_resp(v: Value, code: u16) -> Response<std::io::Cursor<Vec<u8>>> {
    Response::from_string(v.to_string())
        .with_status_code(code)
        .with_header(Header::from_bytes("Content-Type", "application/json").unwrap())
}

fn body_json(req: &mut Request) -> Value {
    let mut s = String::new();
    let _ = req.as_reader().take(1 << 20).read_to_string(&mut s);
    serde_json::from_str(&s).unwrap_or(Value::Null)
}

/// "Finish lab report 40m ! tomorrow #hard" -> Task fields.
pub fn quick_add(input: &str, today: &str, id: u64) -> Option<Task> {
    let mut title = Vec::new();
    let mut est = 30;
    let mut prio = 1;
    let mut due = None;
    let mut hard = false;
    for tok in input.split_whitespace() {
        let lower = tok.to_lowercase();
        let num = |suffix: char| lower.strip_suffix(suffix).and_then(|n| n.parse::<u32>().ok());
        if let Some(m) = num('m') {
            est = m.clamp(5, 600);
        } else if let Some(h) = num('h') {
            est = (h * 60).clamp(5, 600);
        } else if lower == "!" || lower == "!!" {
            prio = 2;
        } else if lower == "today" {
            due = Some(today.to_string());
        } else if lower == "tomorrow" || lower == "tmrw" {
            due = Some(add_days(today, 1));
        } else if lower == "#hard" || lower == "#deep" {
            hard = true;
        } else {
            title.push(tok);
        }
    }
    let title = title.join(" ");
    (!title.is_empty()).then_some(Task { id, title, done: false, done_on: None, due, est_min: est, priority: prio, hard })
}

fn state_json(sh: &Shared) -> Value {
    let now = win::local_now();
    let today = now.date();
    let plan = planner::plan(&sh.store, &now);
    let habits: Vec<Value> = sh
        .store
        .habits
        .iter()
        .map(|h| json!({"id": h.id, "name": h.name, "time": h.time, "done_today": h.done_on(&today), "streak": h.streak(&today)}))
        .collect();
    let day = sh.store.stats.get(&today).cloned().unwrap_or_default();
    let focus_left = sh.live.focus_until.saturating_sub(unix_now());
    json!({
        "today": today,
        "hour": now.hour,
        "prefs": sh.store.prefs,
        "tasks": sh.store.tasks,
        "reminders": sh.store.reminders,
        "habits": habits,
        "plan": plan,
        "stats": { "per_cat": day.per_cat, "tasks_done": day.tasks_done, "code_by_hour": day.code_by_hour },
        "live": {
            "label": sh.live.label, "category": sh.live.category, "minutes": sh.live.minutes,
            "playing": sh.live.playing, "track": sh.live.track, "artist": sh.live.artist,
            "pet_online": sh.live.pet_online, "focus_left_s": focus_left,
        },
    })
}

fn id_from(path: &str, prefix: &str) -> Option<u64> {
    path.strip_prefix(prefix)?.split('/').next()?.parse().ok()
}

fn handle(mut req: Request, shared: &SharedRef, tx: &Sender<AppEvent>, proxy: &ureq::Agent, pet_host: &str) {
    let url = req.url().to_string();
    let path = url.split('?').next().unwrap_or("").to_string();
    let method = req.method().clone();

    if method == Method::Get && (path == "/" || path == "/index.html") {
        let resp = Response::from_string(APP_HTML)
            .with_header(Header::from_bytes("Content-Type", "text/html; charset=utf-8").unwrap());
        let _ = req.respond(resp);
        return;
    }

    if let Some(rest) = path.strip_prefix("/pet/") {
        let target = format!("http://{pet_host}/{rest}");
        let res = if method == Method::Post {
            let body = body_json(&mut req);
            proxy.post(&target).send_json(body)
        } else {
            proxy.get(&target).call()
        };
        let resp = match res {
            Ok(r) => json_resp(r.into_json::<Value>().unwrap_or(json!({"ok": true})), 200),
            Err(e) => json_resp(json!({"ok": false, "err": e.to_string()}), 502),
        };
        let _ = req.respond(resp);
        return;
    }

    let body = if method == Method::Post { body_json(&mut req) } else { Value::Null };
    let today = win::local_now().date();
    let mut sh = shared.lock().unwrap();
    let mut changed = true;
    let sounds = sh.store.prefs.sounds;

    let result: Result<(), &str> = (|| {
        match (&method, path.as_str()) {
            (Method::Get, "/api/state") => {
                changed = false;
            }
            (Method::Post, "/api/tasks") => {
                let id = sh.store.new_id();
                let task = if let Some(q) = body["quick"].as_str() {
                    quick_add(q, &today, id).ok_or("empty task")?
                } else {
                    let title = body["title"].as_str().unwrap_or("").trim().to_string();
                    if title.is_empty() {
                        return Err("title required");
                    }
                    Task {
                        id,
                        title,
                        done: false,
                        done_on: None,
                        due: body["due"].as_str().map(str::to_string),
                        est_min: body["est_min"].as_u64().unwrap_or(30).clamp(5, 600) as u32,
                        priority: body["priority"].as_u64().unwrap_or(1).min(2) as u8,
                        hard: body["hard"].as_bool().unwrap_or(false),
                    }
                };
                sh.store.tasks.push(task);
            }
            (Method::Post, p) if p.starts_with("/api/tasks/") => {
                let id = id_from(p, "/api/tasks/").ok_or("bad id")?;
                let mut done_title = None;
                {
                    let t = sh.store.tasks.iter_mut().find(|t| t.id == id).ok_or("no task")?;
                    if let Some(d) = body["done"].as_bool() {
                        if d && !t.done {
                            done_title = Some(t.title.clone());
                            t.done_on = Some(today.clone());
                        }
                        if !d {
                            t.done_on = None;
                        }
                        t.done = d;
                    }
                    if let Some(v) = body["title"].as_str() {
                        t.title = v.to_string();
                    }
                    if let Some(v) = body["est_min"].as_u64() {
                        t.est_min = v.clamp(5, 600) as u32;
                    }
                    if body.get("due").is_some() {
                        t.due = body["due"].as_str().map(str::to_string);
                    }
                    if let Some(v) = body["priority"].as_u64() {
                        t.priority = v.min(2) as u8;
                    }
                    if let Some(v) = body["hard"].as_bool() {
                        t.hard = v;
                    }
                }
                if let Some(title) = done_title {
                    sh.store.day(&today).tasks_done += 1;
                    let _ = tx.send(AppEvent::TaskDone(title));
                }
            }
            (Method::Delete, p) if p.starts_with("/api/tasks/") => {
                let id = id_from(p, "/api/tasks/").ok_or("bad id")?;
                sh.store.tasks.retain(|t| t.id != id);
            }
            (Method::Post, "/api/tasks-clear-done") => {
                sh.store.tasks.retain(|t| !t.done);
            }
            (Method::Post, "/api/reminders") => {
                let text = body["text"].as_str().unwrap_or("").trim().to_string();
                let kind = body["kind"].as_str().unwrap_or("daily").to_string();
                if text.is_empty() || !["once", "daily", "every"].contains(&kind.as_str()) {
                    return Err("text and kind required");
                }
                let time = body["time"].as_str().map(str::to_string);
                if kind != "every" && time.as_deref().and_then(parse_hm).is_none() {
                    return Err("time HH:MM required");
                }
                let id = sh.store.new_id();
                sh.store.reminders.push(Reminder {
                    id,
                    text,
                    date: if kind == "once" { Some(body["date"].as_str().map(str::to_string).unwrap_or(today.clone())) } else { None },
                    kind,
                    time,
                    every_min: body["every_min"].as_u64().unwrap_or(60).clamp(5, 24 * 60) as u32,
                    enabled: true,
                    last_fired: unix_now() as i64,
                });
            }
            (Method::Post, p) if p.starts_with("/api/reminders/") => {
                let id = id_from(p, "/api/reminders/").ok_or("bad id")?;
                let r = sh.store.reminders.iter_mut().find(|r| r.id == id).ok_or("no reminder")?;
                if let Some(e) = body["enabled"].as_bool() {
                    r.enabled = e;
                }
            }
            (Method::Delete, p) if p.starts_with("/api/reminders/") => {
                let id = id_from(p, "/api/reminders/").ok_or("bad id")?;
                sh.store.reminders.retain(|r| r.id != id);
            }
            (Method::Post, "/api/habits") => {
                let name = body["name"].as_str().unwrap_or("").trim().to_string();
                if name.is_empty() {
                    return Err("name required");
                }
                let id = sh.store.new_id();
                let time = body["time"].as_str().filter(|t| parse_hm(t).is_some()).map(str::to_string);
                sh.store.habits.push(Habit { id, name, time, done_days: vec![], nudged_on: None });
            }
            (Method::Post, p) if p.starts_with("/api/habits/") && p.ends_with("/check") => {
                let id = id_from(p, "/api/habits/").ok_or("bad id")?;
                let h = sh.store.habits.iter_mut().find(|h| h.id == id).ok_or("no habit")?;
                if h.done_on(&today) {
                    h.done_days.retain(|d| d != &today);
                } else {
                    h.done_days.push(today.clone());
                    h.done_days.sort();
                    let streak = h.streak(&today);
                    let _ = tx.send(AppEvent::Say(format!("{} done! {}-day streak.", h.name, streak)));
                    if sounds {
                        sound::play(Sound::Chime);
                    }
                }
            }
            (Method::Delete, p) if p.starts_with("/api/habits/") => {
                let id = id_from(p, "/api/habits/").ok_or("bad id")?;
                sh.store.habits.retain(|h| h.id != id);
            }
            (Method::Post, "/api/prefs") => {
                let mut prefs = serde_json::to_value(&sh.store.prefs).unwrap_or(json!({}));
                if let (Some(dst), Some(src)) = (prefs.as_object_mut(), body.as_object()) {
                    for (k, v) in src {
                        if dst.contains_key(k) {
                            dst.insert(k.clone(), v.clone());
                        }
                    }
                }
                sh.store.prefs = serde_json::from_value(prefs).map_err(|_| "bad prefs")?;
            }
            (Method::Post, "/api/focus") => {
                let mins = body["minutes"].as_u64().unwrap_or(0).min(240);
                sh.live.focus_until = if mins == 0 { 0 } else { unix_now() + mins * 60 };
                changed = false;
                if mins > 0 {
                    let _ = tx.send(AppEvent::FocusStarted(mins as u32));
                }
            }
            (Method::Post, "/api/claude") => {
                changed = false;
                let _ = tx.send(AppEvent::Claude {
                    state: body["state"].as_str().unwrap_or("none").to_string(),
                    project: body["project"].as_str().unwrap_or("").to_string(),
                });
            }
            (Method::Post, "/api/test-sound") => {
                changed = false;
                sound::play(Sound::Chime);
            }
            _ => return Err("not found"),
        }
        Ok(())
    })();

    let resp = match result {
        Ok(()) => {
            if changed {
                sh.store.save();
            }
            json_resp(state_json(&sh), 200)
        }
        Err("not found") => json_resp(json!({"ok": false, "err": "not found"}), 404),
        Err(e) => json_resp(json!({"ok": false, "err": e}), 400),
    };
    drop(sh);
    let _ = req.respond(resp);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_add_parses_tokens() {
        let t = quick_add("Finish lab report 40m ! tomorrow #hard", "2026-11-26", 1).unwrap();
        assert_eq!(t.title, "Finish lab report");
        assert_eq!(t.est_min, 40);
        assert_eq!(t.priority, 2);
        assert_eq!(t.due.as_deref(), Some("2026-11-27"));
        assert!(t.hard);
        assert!(quick_add("  1h ", "2026-11-26", 2).is_none());
    }
}
