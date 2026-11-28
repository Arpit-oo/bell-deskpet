//! Bell: a desktop pet that lives on top of your screen, plus the companion for
//! the ESP32 desk pet. Watches what you're doing on Windows (focused app,
//! Spotify via the Windows media session, idle time), shows it on the desktop
//! overlay, and streams it to the robot with hype lines, album art and lyrics.
//!
//! Usage:
//!   bell [--host 192.168.137.243] [--name Arpit] [--no-overlay]
//!   bell --once                 print what it detects right now
//!   bell --claude <state>       Claude Code hook: working|waiting|done|none
//!   bell --install|--uninstall  autostart at login (HKCU Run key)

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod activity;
mod appwin;
mod art;
mod device;
mod face;
mod hype;
mod lyrics;
mod media;
mod overlay;
mod planner;
mod server;
mod sound;
mod spotify;
mod store;
mod win;

use std::fs::{File, OpenOptions};
use std::io::{IsTerminal, Read, Write};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::json;

use activity::{classify, Tracker};
use device::Device;
use hype::{Hype, Say};
use win::ExeCache;

const DEFAULT_HOST: &str = "192.168.137.243";
const DEFAULT_NAME: &str = "Arpit";
const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "Bell";
const CHILD_ENV: &str = "DESKPET_HOOK_CHILD";

static LOG: OnceLock<Mutex<Option<File>>> = OnceLock::new();

pub fn log(msg: &str) {
    let line = format!("[{}] {msg}\n", unix_secs());
    #[cfg(debug_assertions)]
    eprint!("{line}");
    let lock = LOG.get_or_init(|| {
        let path = std::env::temp_dir().join("deskpet-companion.log");
        Mutex::new(OpenOptions::new().create(true).write(true).truncate(true).open(path).ok())
    });
    if let Ok(mut f) = lock.lock() {
        if let Some(f) = f.as_mut() {
            let _ = f.write_all(line.as_bytes());
        }
    }
}

fn unix_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Release builds have no console; borrow the parent terminal for CLI output.
fn attach_console() {
    #[cfg(not(debug_assertions))]
    unsafe {
        windows_sys::Win32::System::Console::AttachConsole(windows_sys::Win32::System::Console::ATTACH_PARENT_PROCESS);
    }
}

struct Config {
    host: String,
    name: String,
    overlay: bool,
}

enum Cmd {
    Run(Config),
    App,
    Once,
    Claude(Config, String, Option<String>),
    Install(Config),
    Uninstall,
    Help,
}

fn parse_args() -> Result<Cmd, String> {
    let mut host = std::env::var("DESKPET_HOST").unwrap_or_else(|_| DEFAULT_HOST.to_string());
    let mut name = DEFAULT_NAME.to_string();
    let mut mode = "run";
    let mut claude_state = String::new();
    let mut project = None;
    let mut overlay = true;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--host" => host = args.next().ok_or("--host needs a value")?,
            "--name" => name = args.next().ok_or("--name needs a value")?,
            "--project" => project = Some(args.next().ok_or("--project needs a value")?),
            "--once" => mode = "once",
            "--no-overlay" => overlay = false,
            "--claude" => {
                mode = "claude";
                claude_state = args.next().ok_or("--claude needs a state")?;
            }
            "--app" => mode = "app",
            "--install" => mode = "install",
            "--uninstall" => mode = "uninstall",
            "-h" | "--help" => mode = "help",
            other => return Err(format!("unknown argument: {other}")),
        }
    }
    let cfg = Config { host, name, overlay };
    Ok(match mode {
        "once" => Cmd::Once,
        "claude" => Cmd::Claude(cfg, claude_state, project),
        "install" => Cmd::Install(cfg),
        "app" => Cmd::App,
        "uninstall" => Cmd::Uninstall,
        "help" => Cmd::Help,
        _ => Cmd::Run(cfg),
    })
}

fn install(cfg: &Config) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut cmdline = format!("\"{}\" --host {} --name {}", exe.display(), cfg.host, cfg.name);
    if !cfg.overlay {
        cmdline.push_str(" --no-overlay");
    }
    let status = std::process::Command::new("reg")
        .args(["add", RUN_KEY, "/v", RUN_VALUE, "/t", "REG_SZ", "/d", &cmdline, "/f"])
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() { Ok(()) } else { Err(format!("reg add failed: {status}")) }
}

fn uninstall() -> Result<(), String> {
    let status = std::process::Command::new("reg")
        .args(["delete", RUN_KEY, "/v", RUN_VALUE, "/f"])
        .status()
        .map_err(|e| e.to_string())?;
    if status.success() { Ok(()) } else { Err(format!("reg delete failed: {status}")) }
}

fn print_once() {
    let mut cache = ExeCache::new();
    let idle = Duration::from_millis(win::idle_ms());
    let fg = win::foreground(&mut cache);
    println!("foreground : {:?}", fg);
    println!("activity   : {:?}", classify(fg, idle));
    println!("idle       : {}s", idle.as_secs());
    match media::Media::new() {
        Some(m) => {
            let t = m.track();
            println!("media      : {:?}", t);
            if t.is_some() {
                let art = m.thumbnail().and_then(|b| art::process(&b));
                println!("album art  : {}", art.map(|a| format!("ok, accent {}", a.color)).unwrap_or("none".into()));
            }
        }
        None => println!("media      : session manager unavailable"),
    }
    println!("spotify win: {:?}", spotify::now_playing(&mut cache));
}

/// Claude Code hook. Returns instantly: the HTTP call happens in a detached
/// child so Claude is never slowed down, even if the pet is offline.
fn claude_hook(cfg: Config, state: String, project: Option<String>) {
    if std::env::var_os(CHILD_ENV).is_some() {
        let project = project.unwrap_or_default();
        // Prefer the running companion (plays laptop sounds); fall back to the pet directly.
        let local = ureq::AgentBuilder::new().timeout(Duration::from_millis(600)).build();
        let via_app = local
            .post(&format!("http://127.0.0.1:{}/api/claude", server::PORT))
            .send_json(json!({ "state": state, "project": project }))
            .is_ok();
        if !via_app {
            let dev = Device::with_timeout(&cfg.host, Duration::from_millis(2500));
            dev.send_claude(&state, &project);
        }
        return;
    }

    // Hook input arrives as JSON on stdin; take the project folder name from "cwd".
    let mut cwd = None;
    if !std::io::stdin().is_terminal() {
        let mut buf = String::new();
        let _ = std::io::stdin().take(1 << 20).read_to_string(&mut buf);
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&buf) {
            cwd = v["cwd"].as_str().map(str::to_string);
        }
    }
    let project = project.or_else(|| {
        let dir = cwd.map(std::path::PathBuf::from).or_else(|| std::env::current_dir().ok())?;
        dir.file_name().map(|n| n.to_string_lossy().into_owned())
    });

    if let Ok(exe) = std::env::current_exe() {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        let mut cmd = std::process::Command::new(exe);
        cmd.args(["--host", &cfg.host, "--claude", &state]);
        if let Some(p) = &project {
            cmd.args(["--project", p]);
        }
        let _ = cmd
            .env(CHILD_ENV, "1")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW)
            .spawn();
    }
}

/// Now-playing info merged from the media session (preferred) or the window title.
struct NowPlaying {
    music: spotify::Music,
    track: Option<media::Track>,
}

fn now_playing(media: Option<&media::Media>, cache: &mut ExeCache) -> NowPlaying {
    if let Some(t) = media.and_then(|m| m.track()) {
        let music = spotify::Music { playing: t.playing, track: t.title.clone(), artist: t.artist.clone() };
        return NowPlaying { music, track: Some(t) };
    }
    NowPlaying { music: spotify::now_playing(cache), track: None }
}

fn run(cfg: Config) {
    if !win::single_instance("Local\\DeskPetCompanion") {
        log("another instance is already running; exiting");
        return;
    }
    log(&format!("deskpet-companion started (pet at {}, name {})", cfg.host, cfg.name));

    let shared: server::SharedRef = std::sync::Arc::new(std::sync::Mutex::new(server::Shared {
        store: store::Store::load(),
        live: server::Live::default(),
    }));
    let (tx, rx) = std::sync::mpsc::channel::<server::AppEvent>();
    server::start(shared.clone(), tx, cfg.host.clone());

    let pet = cfg.overlay.then(overlay::start);
    let mut overlay_said: Option<Instant> = None;

    let media = media::Media::new();
    if media.is_none() {
        log("media session unavailable; falling back to Spotify window title");
    }
    let mut lyr = lyrics::Lyrics::new();
    let mut cache = ExeCache::new();
    let mut device = Device::new(&cfg.host);
    let start = Instant::now();
    let mut tracker = Tracker::new(start);
    let mut hype = Hype::new(cfg.name.clone(), start);
    let mut last_tick = start;
    let mut last_save = start;
    let mut last_checks: Option<Instant> = None;
    let mut pomo_focus_min: u64 = 25;
    let mut focus_warned: Option<Instant> = None;

    let mut np = NowPlaying { music: spotify::Music::default(), track: None };
    let mut last_music_poll: Option<Instant> = None;
    let mut last_payload = String::new();
    let mut last_send: Option<Instant> = None;
    let mut last_ok = true;
    let mut last_pos_sent: Option<(u64, Instant)> = None;
    let mut pending_say: Option<(Say, Instant)> = None;

    let mut art_key = String::new(); // track whose art the pet has
    let mut art_color: Option<String> = None;
    let mut art_tries = 0u32;
    let mut last_settings_poll: Option<Instant> = None;
    let mut was_online = false;

    loop {
        let now = Instant::now();
        let idle = Duration::from_millis(win::idle_ms());
        let seen = classify(win::foreground(&mut cache), idle);
        let transition = tracker.update(seen.clone(), now, idle);

        let music_every = if np.music.playing { Duration::from_millis(500) } else { Duration::from_secs(2) };
        if last_music_poll.is_none_or(|t| now.duration_since(t) >= music_every) {
            np = now_playing(media.as_ref(), &mut cache);
            last_music_poll = Some(now);
        }

        // Pull hype level / name from the pet's web app every 30 s.
        if device.is_online() && last_settings_poll.is_none_or(|t| now.duration_since(t) >= Duration::from_secs(30)) {
            last_settings_poll = Some(now);
            if let Some((level, pomo)) = device.settings() {
                hype.set_level(level, "");
                pomo_focus_min = pomo;
            }
        }

        let committed = tracker.current().clone();
        let minutes = tracker.minutes(now);
        let (hour, day) = win::local_hour_and_day();

        if let Some(s) = hype.tick(now, &committed, minutes, transition.as_ref(), &np.music, hour, day) {
            log(&format!("say [{}]: {}", s.mood, s.text));
            pending_say = Some((s, now));
        }
        // ---- everyday app: prefs, stats, app events, reminders, habits, focus ----
        let lt = win::local_now();
        let today = lt.date();
        let (prefs, plan) = {
            let mut sh = shared.lock().unwrap();
            let dt = now.duration_since(last_tick).as_secs_f32().min(5.0);
            if committed.category != activity::Category::Away {
                let d = sh.store.day(&today);
                *d.per_cat.entry(committed.category.as_str().to_string()).or_insert(0) += dt.round() as u32;
                if committed.category == activity::Category::Code {
                    d.code_by_hour[lt.hour as usize] += dt.round() as u32;
                }
            }
            sh.live.label = seen.label.clone();
            sh.live.category = seen.category.as_str().to_string();
            sh.live.minutes = if seen.category == committed.category { minutes } else { 0 };
            sh.live.playing = np.music.playing;
            sh.live.track = np.music.track.clone();
            sh.live.artist = np.music.artist.clone();
            sh.live.pet_online = device.is_online();
            if now.duration_since(last_save) >= Duration::from_secs(60) {
                sh.store.prune();
                sh.store.save();
                last_save = now;
            }
            (sh.store.prefs.clone(), planner::plan(&sh.store, &lt))
        };
        last_tick = now;
        hype.set_persona(&prefs.personality, &prefs.name);

        while let Ok(ev) = rx.try_recv() {
            match ev {
                server::AppEvent::TaskDone(title) => pending_say = Some((hype.task_done(now, &title), now)),
                server::AppEvent::Say(text) => {
                    pending_say = Some((Say { text, mood: "happy", ms: 5000, sound: sound::Sound::Blip }, now))
                }
                server::AppEvent::FocusStarted(m) => {
                    let text = format!("Focus mode: {m} min. Distractions blocked. Let's go!");
                    pending_say = Some((Say { text, mood: "focus", ms: 5000, sound: sound::Sound::Start }, now));
                }
                server::AppEvent::Claude { state, project } => {
                    if let Some(p) = &pet {
                        p.set_claude(&state, &project);
                    }
                    device.send_claude(&state, &project);
                    if prefs.sounds {
                        match state.as_str() {
                            "waiting" => sound::play(sound::Sound::ClaudeWait),
                            "done" => sound::play(sound::Sound::ClaudeDone),
                            _ => {}
                        }
                    }
                    if prefs.voice && state == "waiting" {
                        sound::speak(&format!("{}, Claude needs you.", prefs.name));
                    }
                }
            }
        }

        if last_checks.is_none_or(|t| now.duration_since(t) >= Duration::from_secs(10)) {
            last_checks = Some(now);
            let nowm = lt.minutes_of_day();
            let unix = server::unix_now() as i64;
            let in_day = match (store::parse_hm(&prefs.day_start), store::parse_hm(&prefs.day_end)) {
                (Some(a), Some(b)) => nowm >= a && nowm <= b,
                _ => true,
            };
            let mut sh = shared.lock().unwrap();
            let mut fired: Option<String> = None;
            for r in sh.store.reminders.iter_mut().filter(|r| r.enabled) {
                let at = r.time.as_deref().and_then(store::parse_hm);
                let fired_recently = r.last_fired > 0 && unix - r.last_fired < 20 * 3600;
                let due = match r.kind.as_str() {
                    "every" => in_day && unix - r.last_fired >= r.every_min as i64 * 60,
                    "daily" => at.is_some_and(|t| nowm >= t && nowm < t + 30) && !fired_recently,
                    "once" => r.date.as_deref() == Some(today.as_str()) && at.is_some_and(|t| nowm >= t),
                    _ => false,
                };
                if due && fired.is_none() {
                    r.last_fired = unix;
                    if r.kind == "once" {
                        r.enabled = false;
                    }
                    fired = Some(r.text.clone());
                }
            }
            if fired.is_none() {
                for h in sh.store.habits.iter_mut() {
                    let at = h.time.as_deref().and_then(store::parse_hm);
                    if at.is_some_and(|t| nowm >= t)
                        && !h.done_on(&today)
                        && h.nudged_on.as_deref() != Some(today.as_str())
                    {
                        h.nudged_on = Some(today.clone());
                        let streak = h.streak(&today);
                        fired = Some(if streak > 0 {
                            format!("{}? Keep your {}-day streak!", h.name, streak)
                        } else {
                            format!("Habit time: {}", h.name)
                        });
                        break;
                    }
                }
            }
            if let Some(text) = fired {
                sh.store.save();
                log(&format!("reminder: {text}"));
                pending_say = Some((Say { text, mood: "surprised", ms: 9000, sound: sound::Sound::Bell }, now));
            }
        }

        // Focus mode: warn, then minimise distracting windows.
        let focus_on = shared.lock().unwrap().live.focus_until > server::unix_now();
        if focus_on {
            let distracting = matches!(seen.category, activity::Category::Video | activity::Category::Social)
                || prefs.focus_block.iter().any(|b| seen.label.to_lowercase().contains(b.as_str()));
            if distracting {
                match focus_warned {
                    None => {
                        focus_warned = Some(now);
                        let text = format!("Focus mode! Not now, {}.", seen.label);
                        pending_say = Some((Say { text, mood: "angry", ms: 4000, sound: sound::Sound::Buzz }, now));
                    }
                    Some(t) if prefs.auto_minimize && now.duration_since(t) >= Duration::from_secs(4) => {
                        win::minimize_foreground();
                        focus_warned = None;
                    }
                    _ => {}
                }
            } else {
                focus_warned = None;
            }
        }

        if pending_say.as_ref().is_some_and(|(_, t)| now.duration_since(*t) > Duration::from_secs(15)) {
            pending_say = None;
        }


        let (disp, disp_min) = if seen.category == committed.category {
            (committed.clone(), minutes)
        } else {
            (seen.clone(), 0)
        };

        // Album art: push once per track (retry a few times; Spotify sets it late).
        let track_key = np.track.as_ref().map(|t| t.key()).unwrap_or_default();
        if device.is_online() && !was_online {
            art_key.clear(); // pet may have rebooted
        }
        was_online = device.is_online();
        if let (Some(m), Some(_)) = (media.as_ref(), np.track.as_ref()) {
            if track_key != art_key && device.is_online() {
                if art_tries == 0 || art_tries.is_multiple_of(4) {
                    if let Some(a) = m.thumbnail().and_then(|b| art::process(&b)) {
                        if device.send_art(&a.b64) {
                            art_key = track_key.clone();
                            art_color = Some(a.color);
                            art_tries = 0;
                        }
                    }
                }
                art_tries += 1;
                if art_tries > 40 {
                    art_key = track_key.clone(); // give up on this track
                    art_color = None;
                    art_tries = 0;
                }
            }
        }

        // Synced lyrics.
        let (pos_ms, dur_ms) = np.track.as_ref().map(|t| (t.position_ms, t.duration_ms)).unwrap_or((0, 0));
        let (lyric_cur, lyric_next, line_ms) = match np.track.as_ref() {
            Some(t) if t.playing => {
                let req = lyrics::Request {
                    key: t.key(),
                    title: t.title.clone(),
                    artist: t.artist.clone(),
                    album: t.album.clone(),
                    duration_s: t.duration_ms / 1000,
                };
                lyr.get(req).map(|lines| lyrics::at(lines, pos_ms)).unwrap_or_default()
            }
            _ => Default::default(),
        };

        if let Some(p) = &pet {
            p.set_mood(Hype::mood_for(&disp, disp_min));
            p.set_music(np.music.playing, &np.music.track, &np.music.artist, &lyric_cur, line_ms as u32);
            if let Some((s, t)) = &pending_say {
                if overlay_said != Some(*t) {
                    overlay_said = Some(*t);
                    p.say(&s.text, s.mood, s.ms);
                }
            }
        }

        let mut payload = json!({
            "name": prefs.name,
            "activity": { "label": disp.label, "category": disp.category.as_str(), "minutes": disp_min },
            "music": {
                "playing": np.music.playing,
                "track": np.music.track,
                "artist": np.music.artist,
                "duration_ms": dur_ms,
                "color": art_color.clone().unwrap_or_default(),
            },
            "lyrics": { "cur": lyric_cur, "next": lyric_next, "line_ms": line_ms },
            "task": {
                "title": plan.now.as_ref().map(|t| t.title.clone()).unwrap_or_default(),
                "est_min": plan.now.as_ref().map(|t| t.est_min).unwrap_or(0),
                "left": plan.today.len() + usize::from(plan.now.is_some()),
            },
            "mood": Hype::mood_for(&disp, disp_min),
        });
        // Position isn't part of the change key (it changes constantly).
        let key = payload.to_string();
        payload["music"]["position_ms"] = json!(pos_ms);

        // Resync when the user seeks (position jumps vs. what the pet extrapolates).
        let drift = last_pos_sent.is_some_and(|(p, t)| {
            let expected = p + if np.music.playing { now.duration_since(t).as_millis() as u64 } else { 0 };
            expected.abs_diff(pos_ms) > 2000
        });

        // Clicking the desktop pet flips the robot's screen.
        let clicks = overlay::take_view_clicks();
        if clicks > 0 && device.is_online() {
            let mut shown = None;
            for _ in 0..clicks.min(3) {
                shown = device.set_view("next");
            }
            if let (Some(v), Some(p)) = (shown, &pet) {
                let (label, mood) = match v.as_str() {
                    "music" => ("Spotify", "music"),
                    "clock" => ("Clock", "happy"),
                    "focus" => ("Focus timer", "focus"),
                    _ => ("Face", "love"),
                };
                p.say(&format!("Robot: {label}"), mood, 1600);
            }
        }

        let since_send = last_send.map_or(Duration::MAX, |t| now.duration_since(t));
        let changed = key != last_payload || pending_say.is_some() || drift;
        let backoff = if last_ok { Duration::ZERO } else { Duration::from_secs(5) };
        if (changed && since_send >= backoff) || since_send >= Duration::from_secs(5) {
            if let Some((s, _)) = &pending_say {
                payload["say"] = json!({ "text": s.text, "mood": s.mood, "ms": s.ms });
            }
            let events = device.send_state(&payload);
            last_ok = events.is_some();
            last_send = Some(now);
            for ev in events.iter().flatten() {
                let mut sh = shared.lock().unwrap();
                match ev.as_str() {
                    "pomo_start" | "pomo_focus" => {
                        sh.live.focus_until = server::unix_now() + pomo_focus_min * 60;
                        if prefs.sounds {
                            sound::play(sound::Sound::Start);
                        }
                    }
                    "timer_done" => {
                        if prefs.sounds {
                            sound::play(sound::Sound::Bell);
                        }
                    }
                    "pomo_break" => {
                        sh.live.focus_until = 0;
                        if prefs.sounds {
                            sound::play(sound::Sound::Break);
                        }
                    }
                    _ => {}
                }
            }
            if last_ok {
                if let Some((s, _)) = &pending_say {
                    if prefs.sounds {
                        sound::play(s.sound);
                    }
                    if prefs.voice {
                        sound::speak(&s.text);
                    }
                }
                last_payload = key;
                last_pos_sent = Some((pos_ms, now));
                pending_say = None;
            }
        }

        let tick = if np.music.playing { 250 } else { 1000 };
        std::thread::sleep(Duration::from_millis(tick));
    }
}

fn main() {
    let cmd = match parse_args() {
        Ok(c) => c,
        Err(e) => {
            attach_console();
            eprintln!("error: {e}\nrun with --help for usage");
            std::process::exit(2);
        }
    };
    match cmd {
        Cmd::Run(cfg) => run(cfg),
        Cmd::App => appwin::run(),
        Cmd::Claude(cfg, state, project) => claude_hook(cfg, state, project),
        Cmd::Once => {
            attach_console();
            print_once();
        }
        Cmd::Install(cfg) => {
            attach_console();
            match install(&cfg) {
                Ok(()) => println!("autostart installed ({RUN_VALUE})"),
                Err(e) => eprintln!("install failed: {e}"),
            }
        }
        Cmd::Uninstall => {
            attach_console();
            match uninstall() {
                Ok(()) => println!("autostart removed"),
                Err(e) => eprintln!("uninstall failed: {e}"),
            }
        }
        Cmd::Help => {
            attach_console();
            println!(
                "deskpet-companion [--host IP] [--name NAME] | --once | --claude STATE | --install | --uninstall\n\
                 default host {DEFAULT_HOST} (or DESKPET_HOST env), default name {DEFAULT_NAME}"
            );
        }
    }
}
