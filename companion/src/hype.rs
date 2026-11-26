//! The hype engine: decides when the pet should say something.

use std::time::{Duration, Instant};

use crate::activity::{Activity, Category, Transition};
use crate::sound::Sound;
use crate::spotify::Music;

pub struct Say {
    pub text: String,
    pub mood: &'static str,
    pub ms: u32,
    pub sound: Sound,
}

/// Line packs per personality. Placeholders: {name} {label} {m} {task}.
fn persona_lines(p: &str, kind: &str) -> &'static [&'static str] {
    match (p, kind) {
        ("bro", "greet") => &["Yo {name}! Let's get this bread.", "What's good {name}, we cooking today?"],
        ("bro", "back") => &["Welcome back king. Let's go!", "Bro's back! Lock in."],
        ("bro", "code_start") => &["Code time. Let's get it bro.", "Editor open. Absolute unit."],
        ("bro", "code_hype") => &["{m} min deep, you're HIM.", "Bro is cooking fr.", "No cap, this is the grind."],
        ("bro", "generic") => &["You got this bro.", "Stay locked, king.", "W mindset only."],
        ("bro", "nudge") => &["Bro... {label} for {m} min. Back to it?", "Ayo {name}, {m} min of {label}. Lock in."],
        ("bro", "done") => &["W! \"{task}\" done.", "Another one! {task} - gone."],
        ("coach", "greet") => &["Morning, {name}. Plan, execute, review.", "Let's set the pace, {name}."],
        ("coach", "back") => &["Back on the field. Next play.", "Rested? Good. Go."],
        ("coach", "code_start") => &["Head down. Execute.", "Deep work block starts now."],
        ("coach", "code_hype") => &["{m} min. Hold the pace.", "Consistency beats intensity. Keep going."],
        ("coach", "generic") => &["Discipline is a decision. Decide.", "One rep at a time."],
        ("coach", "nudge") => &["{label}, {m} min. That's not the plan, {name}.", "Clock's running. Back to work."],
        ("coach", "done") => &["Done: {task}. Next rep.", "Checked off. Momentum is yours."],
        ("butler", "greet") => &["Good day, {name}. Shall we begin?", "At your service, {name}."],
        ("butler", "back") => &["Welcome back, sir. Your work awaits.", "Ah, you've returned. Splendid."],
        ("butler", "code_start") => &["The editor is open, sir. Most productive.", "Shall I hold your calls? Coding time."],
        ("butler", "code_hype") => &["{m} minutes of focus. Exemplary, sir.", "Your diligence is noted, sir."],
        ("butler", "generic") => &["Might I suggest a glass of water, sir?", "Carry on, sir. Splendidly done."],
        ("butler", "nudge") => &["Pardon me, sir. {label} for {m} minutes...", "Perhaps the work, sir? {label} can wait."],
        ("butler", "done") => &["\"{task}\" is complete, sir. Bravo.", "Another task dispatched, sir."],
        ("chill", "greet") => &["hey {name}. easy pace today.", "morning {name}. one thing at a time."],
        ("chill", "back") => &["welcome back. no rush.", "oh hey, you're back."],
        ("chill", "code_start") => &["coding. nice and steady.", "ok, flow time."],
        ("chill", "code_hype") => &["{m} min in. you're doing great.", "nice flow. breathe."],
        ("chill", "generic") => &["drink some water.", "you're doing fine. really."],
        ("chill", "nudge") => &["{label} for {m} min. maybe wrap up?", "gentle reminder: the task is waiting."],
        ("chill", "done") => &["done. nice.", "{task}, checked. good job."],
        ("kawaii", "greet") => &["Ohayo {name}-kun! Ganbatte!", "Yay {name} is here! Let's do our best!"],
        ("kawaii", "back") => &["Okaeri {name}! I missed you!", "You're back! Yay!"],
        ("kawaii", "code_start") => &["Coding time! Sugoi!", "Let's write cute code together!"],
        ("kawaii", "code_hype") => &["{m} min! You're so cool, {name}!", "Kakkoii! Keep going!"],
        ("kawaii", "generic") => &["Ganbatte, {name}!", "You can do it! Fighting!"],
        ("kawaii", "nudge") => &["Ehh? {label} for {m} min... back to work?", "Mou~ {name}, focus please!"],
        ("kawaii", "done") => &["Yatta! {task} done!", "Sugoi! Another one cleared!"],
        (_, "greet") => &["Let's go, {name}! I've got your back.", "Hey {name}! Big day. Let's cook."],
        (_, "back") => &["Welcome back, {name}! Let's go!", "There you are! Let's get it."],
        (_, "code_start") => CODE_START,
        (_, "code_hype") => CODE_HYPE,
        (_, "nudge") => &["{label} for {m} min... back to the grind, {name}?"],
        (_, "done") => &["LET'S GO! \"{task}\" done!", "Crushed it: {task}!", "One down! {task}"],
        _ => GENERIC_HYPE,
    }
}


const CODE_START: &[&str] = &[
    "Back to code. LET'S GO!",
    "Locked in. Let's cook, {name}.",
    "Build mode: ON.",
    "Editor open. Greatness loading...",
    "Let's ship something today!",
];

const CODE_HYPE: &[&str] = &[
    "You're cooking, {name}!",
    "{m} min deep. Keep that streak!",
    "Bugs fear you.",
    "Clean code, clear mind. Keep going!",
    "This is the grind that pays off.",
];

const GENERIC_HYPE: &[&str] = &[
    "Let's go, {name}!",
    "You got this, {name}.",
    "Small steps. Big wins.",
    "Focus. Ship. Repeat.",
    "One more push!",
    "Proud of you, fr.",
    "Hydrate. Then dominate.",
];

pub struct Hype {
    name: String,
    rng: u64,
    started: bool,
    last_say: Option<Instant>,
    next_random: Instant,
    milestones_hit: Vec<u64>,
    last_nudge_min: Option<u64>,
    last_track: String,
    late_night_day: u32,
    /// 0 chill, 1 normal, 2 max (set from the pet's web app).
    level: u8,
    persona: String,
}

impl Hype {
    pub fn new(name: String, now: Instant) -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E3779B97F4A7C15)
            | 1;
        Self {
            name,
            rng: seed,
            started: false,
            last_say: None,
            next_random: now + Duration::from_secs(12 * 60),
            milestones_hit: Vec::new(),
            last_nudge_min: None,
            last_track: String::new(),
            late_night_day: 0,
            level: 1,
            persona: "hype".into(),
        }
    }

    pub fn set_persona(&mut self, persona: &str, name: &str) {
        self.persona = persona.to_string();
        if !name.is_empty() {
            self.name = name.to_string();
        }
    }

    /// A line of `kind` from the current personality, with placeholders filled.
    pub fn line(&mut self, kind: &str, act: Option<&Activity>, minutes: u64, task: &str) -> String {
        let lines = persona_lines(&self.persona.clone(), kind);
        let l = lines[self.rand(lines.len())];
        let mut s = l.replace("{name}", &self.name).replace("{m}", &minutes.to_string()).replace("{task}", task);
        if let Some(a) = act {
            s = s.replace("{label}", &a.label);
        }
        s
    }

    /// Celebration when a task is ticked off (always speaks).
    pub fn task_done(&mut self, now: Instant, title: &str) -> Say {
        let text = self.line("done", None, 0, title);
        self.last_say = Some(now);
        Say { text, mood: "excited", ms: 6000, sound: Sound::Chime }
    }

    pub fn set_level(&mut self, level: u8, name: &str) {
        self.level = level.min(2);
        if !name.is_empty() {
            self.name = name.to_string();
        }
    }

    /// Minimum gap between non-urgent lines so it never gets annoying.
    fn throttle(&self) -> Duration {
        Duration::from_secs([180, 60, 30][self.level as usize])
    }

    /// Minutes between random hype lines: (min, spread).
    fn random_window(&self) -> (u64, usize) {
        [(25, 16), (12, 9), (6, 5)][self.level as usize]
    }

    fn rand(&mut self, n: usize) -> usize {
        // xorshift64
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 7;
        self.rng ^= self.rng << 17;
        (self.rng % n as u64) as usize
    }

    fn can_talk(&self, now: Instant) -> bool {
        self.last_say.is_none_or(|t| now.duration_since(t) >= self.throttle())
    }

    fn emit(&mut self, now: Instant, text: String, mood: &'static str, ms: u32) -> Option<Say> {
        self.last_say = Some(now);
        Some(Say { text, mood, ms, sound: Sound::Blip })
    }

    /// Suggested resting mood for the pet given what the user is doing.
    pub fn mood_for(act: &Activity, minutes: u64) -> &'static str {
        match act.category {
            Category::Code => "focus",
            Category::Chat => "happy",
            Category::Music => "music",
            Category::Game => "excited",
            Category::Away => "sleepy",
            Category::Video | Category::Social if minutes >= 20 => "sus",
            _ => "idle",
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn tick(
        &mut self,
        now: Instant,
        act: &Activity,
        minutes: u64,
        transition: Option<&Transition>,
        music: &Music,
        hour: u32,
        day: u32,
    ) -> Option<Say> {
        // 1. Greeting on startup (always).
        if !self.started {
            self.started = true;
            self.last_track = music.track.clone();
            let text = self.line("greet", Some(act), minutes, "");
            return self.emit(now, text, "excited", 6000);
        }

        // 2. Category changes.
        if let Some(t) = transition {
            if t.from == Category::Away && t.to != Category::Away {
                let text = self.line("back", Some(act), minutes, "");
                return self.emit(now, text, "happy", 5000);
            }
            if t.to == Category::Code {
                self.milestones_hit.clear();
                if t.from != Category::Code && self.can_talk(now) {
                    let text = self.line("code_start", Some(act), minutes, "");
                    return self.emit(now, text, "excited", 5000);
                }
            }
            if matches!(t.to, Category::Video | Category::Social | Category::Game) {
                self.last_nudge_min = None;
            }
        }

        // 3. Focus milestones while coding.
        if act.category == Category::Code {
            let next_milestone = match self.milestones_hit.last().copied() {
                None => 25,
                Some(25) => 50,
                Some(50) => 90,
                Some(m) => m + 30,
            };
            if minutes >= next_milestone {
                self.milestones_hit.push(next_milestone);
                let (text, mood) = match next_milestone {
                    25 => ("25 min locked in. You're on fire!".to_string(), "excited"),
                    50 => (format!("50 minutes deep. Beast mode, {}.", self.name), "excited"),
                    90 => ("90 min straight! Legend. Stand up + drink water.".to_string(), "love"),
                    m => (format!("{}h {:02}m of focus. Unstoppable.", m / 60, m % 60), "love"),
                };
                return self.emit(now, text, mood, 7000);
            }
        }

        // 4. Gentle nudges when distracted for a while.
        let nudge_after = match act.category {
            Category::Video | Category::Social => Some(20),
            Category::Game => Some(60),
            _ => None,
        };
        if let Some(after) = nudge_after {
            let due = match self.last_nudge_min {
                None => minutes >= after,
                Some(last) => minutes >= last + 15,
            };
            if due && self.can_talk(now) {
                self.last_nudge_min = Some(minutes);
                let text = self.line("nudge", Some(act), minutes, "");
                return self.emit(now, text, "sus", 7000);
            }
        }

        // 5. New song.
        if music.playing && !music.track.is_empty() && music.track != self.last_track {
            self.last_track = music.track.clone();
            if self.level > 0 && self.can_talk(now) {
                let text = format!("Now vibing: {}", music.track);
                return self.emit(now, text, "music", 5000);
            }
        }

        // 6. Late night, once per night.
        if (1..=4).contains(&hour) && act.category != Category::Away && self.late_night_day != day {
            self.late_night_day = day;
            let text = format!("It's {} AM... sleep is a feature too, {}.", hour, self.name);
            return self.emit(now, text, "sleepy", 7000);
        }

        // 7. Random hype every 12-20 minutes.
        if now >= self.next_random && act.category != Category::Away {
            let (base, spread) = self.random_window();
            let mins = base + self.rand(spread) as u64;
            self.next_random = now + Duration::from_secs(mins * 60);
            if self.can_talk(now) {
                let text = if act.category == Category::Code {
                    self.line("code_hype", Some(act), minutes, "")
                } else {
                    self.line("generic", Some(act), minutes, "")
                };
                return self.emit(now, text, "excited", 6000);
            }
        }

        None
    }
}
