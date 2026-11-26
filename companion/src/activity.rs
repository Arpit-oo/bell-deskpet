//! Turn "which window is focused" into something a pet can talk about.

use std::time::{Duration, Instant};

/// After this much keyboard/mouse silence the user counts as away.
pub const AWAY_AFTER: Duration = Duration::from_secs(5 * 60);
/// A new category must hold this long before the session timer resets.
const SWITCH_DEBOUNCE: Duration = Duration::from_secs(20);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Code,
    Browse,
    Video,
    Social,
    Chat,
    Music,
    Game,
    Away,
    Other,
}

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Category::Code => "code",
            Category::Browse => "browse",
            Category::Video => "video",
            Category::Social => "social",
            Category::Chat => "chat",
            Category::Music => "music",
            Category::Game => "game",
            Category::Away => "away",
            Category::Other => "other",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Activity {
    pub label: String,
    pub category: Category,
}

impl Activity {
    fn new(label: &str, category: Category) -> Self {
        Self { label: label.to_string(), category }
    }
}

fn browser_name(exe: &str) -> Option<&'static str> {
    Some(match exe {
        "chrome.exe" => "Chrome",
        "msedge.exe" => "Edge",
        "firefox.exe" => "Firefox",
        "brave.exe" => "Brave",
        "opera.exe" | "opera_gx.exe" => "Opera",
        "vivaldi.exe" => "Vivaldi",
        "arc.exe" => "Arc",
        "zen.exe" => "Zen",
        _ => return None,
    })
}

fn classify_browser_title(title: &str, browser: &str) -> Activity {
    let t = title.to_lowercase();
    let rules: &[(&[&str], &str, Category)] = &[
        (&["youtube"], "YouTube", Category::Video),
        (&["netflix"], "Netflix", Category::Video),
        (&["prime video"], "Prime Video", Category::Video),
        (&["hotstar"], "Hotstar", Category::Video),
        (&["twitch"], "Twitch", Category::Video),
        (&["instagram"], "Instagram", Category::Social),
        (&["reddit"], "Reddit", Category::Social),
        (&["twitter", "/ x", "x.com"], "X", Category::Social),
        (&["facebook"], "Facebook", Category::Social),
        (&["linkedin"], "LinkedIn", Category::Social),
        (&["whatsapp"], "WhatsApp", Category::Chat),
        (&["discord"], "Discord", Category::Chat),
        (&["github"], "GitHub", Category::Code),
        (&["stack overflow"], "Stack Overflow", Category::Code),
        (&["leetcode"], "LeetCode", Category::Code),
        (&["codeforces"], "Codeforces", Category::Code),
        (&["chatgpt"], "ChatGPT", Category::Code),
        (&["claude"], "Claude", Category::Code),
        (&["localhost", "127.0.0.1"], "Localhost", Category::Code),
        (&["spotify"], "Spotify", Category::Music),
    ];
    for (needles, label, cat) in rules {
        if needles.iter().any(|n| t.contains(n)) {
            return Activity::new(label, *cat);
        }
    }
    Activity::new(browser, Category::Browse)
}

/// Classify the focused window. `idle` = time since last input.
pub fn classify(foreground: Option<(String, String)>, idle: Duration) -> Activity {
    if idle >= AWAY_AFTER {
        return Activity::new("Away", Category::Away);
    }
    let Some((exe, title)) = foreground else {
        return Activity::new("Desktop", Category::Other);
    };

    if let Some(b) = browser_name(&exe) {
        return classify_browser_title(&title, b);
    }

    let (label, cat) = match exe.as_str() {
        "code.exe" => ("VS Code", Category::Code),
        "cursor.exe" => ("Cursor", Category::Code),
        "windsurf.exe" => ("Windsurf", Category::Code),
        "zed.exe" => ("Zed", Category::Code),
        "devenv.exe" => ("Visual Studio", Category::Code),
        "idea64.exe" => ("IntelliJ", Category::Code),
        "pycharm64.exe" => ("PyCharm", Category::Code),
        "clion64.exe" => ("CLion", Category::Code),
        "rustrover64.exe" => ("RustRover", Category::Code),
        "webstorm64.exe" => ("WebStorm", Category::Code),
        "studio64.exe" => ("Android Studio", Category::Code),
        "sublime_text.exe" => ("Sublime Text", Category::Code),
        "notepad++.exe" => ("Notepad++", Category::Code),
        "arduino ide.exe" => ("Arduino IDE", Category::Code),
        "windowsterminal.exe" | "wt.exe" | "powershell.exe" | "pwsh.exe" | "cmd.exe"
        | "alacritty.exe" | "wezterm-gui.exe" | "mintty.exe" => ("Terminal", Category::Code),
        "claude.exe" => ("Claude", Category::Code),
        "figma.exe" => ("Figma", Category::Code),
        "obsidian.exe" => ("Obsidian", Category::Code),
        "notion.exe" => ("Notion", Category::Code),
        "winword.exe" => ("Word", Category::Code),
        "excel.exe" => ("Excel", Category::Code),
        "powerpnt.exe" => ("PowerPoint", Category::Code),
        "acrobat.exe" | "acrord32.exe" | "sumatrapdf.exe" => ("Reading PDF", Category::Code),
        "spotify.exe" => ("Spotify", Category::Music),
        "discord.exe" => ("Discord", Category::Chat),
        "whatsapp.exe" | "whatsapp.root.exe" => ("WhatsApp", Category::Chat),
        "telegram.exe" => ("Telegram", Category::Chat),
        "slack.exe" => ("Slack", Category::Chat),
        "ms-teams.exe" | "teams.exe" => ("Teams", Category::Chat),
        "steam.exe" | "steamwebhelper.exe" => ("Steam", Category::Game),
        "epicgameslauncher.exe" => ("Epic Games", Category::Game),
        "valorant.exe" | "valorant-win64-shipping.exe" => ("Valorant", Category::Game),
        "cs2.exe" => ("CS2", Category::Game),
        "minecraft.exe" | "javaw.exe" if title.to_lowercase().contains("minecraft") => {
            ("Minecraft", Category::Game)
        }
        "vlc.exe" => ("VLC", Category::Video),
        "explorer.exe" => ("File Explorer", Category::Other),
        "lockapp.exe" => ("Away", Category::Away),
        _ => ("", Category::Other),
    };

    if label.is_empty() {
        // Unknown app: prettify the exe name ("blender.exe" -> "Blender").
        let stem = exe.trim_end_matches(".exe");
        let mut c = stem.chars();
        let pretty = match c.next() {
            Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
            None => "Desktop".to_string(),
        };
        return Activity { label: pretty, category: Category::Other };
    }
    Activity::new(label, cat)
}

/// Tracks how long the user has been in the current kind of activity.
pub struct Tracker {
    current: Activity,
    since: Instant,
    pending: Option<(Category, Instant)>,
}

pub struct Transition {
    pub from: Category,
    pub to: Category,
}

impl Tracker {
    pub fn new(now: Instant) -> Self {
        Self { current: Activity::new("Desktop", Category::Other), since: now, pending: None }
    }

    pub fn current(&self) -> &Activity {
        &self.current
    }

    pub fn minutes(&self, now: Instant) -> u64 {
        now.duration_since(self.since).as_secs() / 60
    }

    /// Feed the latest observation; returns a transition when the category changes.
    pub fn update(&mut self, seen: Activity, now: Instant, idle: Duration) -> Option<Transition> {
        if seen.category == self.current.category {
            self.current.label = seen.label; // e.g. VS Code -> Terminal keeps the timer
            self.pending = None;
            return None;
        }

        let instant = seen.category == Category::Away || self.current.category == Category::Away;
        let commit_since = if instant {
            if seen.category == Category::Away { now.checked_sub(idle).unwrap_or(now) } else { now }
        } else {
            match self.pending {
                Some((cat, t)) if cat == seen.category => {
                    if now.duration_since(t) < SWITCH_DEBOUNCE {
                        return None;
                    }
                    t
                }
                _ => {
                    self.pending = Some((seen.category, now));
                    return None;
                }
            }
        };

        let from = self.current.category;
        self.current = seen;
        self.since = commit_since;
        self.pending = None;
        Some(Transition { from, to: self.current.category })
    }
}
