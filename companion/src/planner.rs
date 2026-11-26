//! "What can I do now / what fits today / what won't" from tasks + free time.

use serde::Serialize;

use crate::store::{parse_hm, Store, Task};
use crate::win::LocalTime;

#[derive(Serialize, Default)]
pub struct Plan {
    /// Minutes left until the end of your day.
    pub free_min: u32,
    /// Minutes until the next timed reminder (or end of day).
    pub window_min: u32,
    pub next_event: Option<(String, String)>,
    pub now: Option<Task>,
    pub today: Vec<Task>,
    pub later: Vec<Task>,
    pub best_hours: Vec<u32>,
    pub tip: String,
}

fn fmt_dur(m: u32) -> String {
    if m < 60 { format!("{m}m") } else { format!("{}h {:02}m", m / 60, m % 60) }
}

pub fn plan(store: &Store, now: &LocalTime) -> Plan {
    let today = now.date();
    let nowm = now.minutes_of_day();
    let end = parse_hm(&store.prefs.day_end).unwrap_or(23 * 60 + 30);
    let free = end.saturating_sub(nowm);

    // Next timed reminder later today bounds the "right now" window.
    let mut next: Option<(u32, String, String)> = None;
    for r in store.reminders.iter().filter(|r| r.enabled) {
        let applies = match r.kind.as_str() {
            "daily" => true,
            "once" => r.date.as_deref() == Some(today.as_str()),
            _ => false,
        };
        if !applies {
            continue;
        }
        if let Some(t) = r.time.as_deref().and_then(parse_hm) {
            if t > nowm && next.as_ref().is_none_or(|(nt, _, _)| t < *nt) {
                next = Some((t, r.text.clone(), r.time.clone().unwrap_or_default()));
            }
        }
    }
    let window = next.as_ref().map(|(t, _, _)| t - nowm).unwrap_or(free).min(free);

    let best = store.best_hours();
    let deep_time = best.is_empty() || best.contains(&now.hour);

    let mut open: Vec<Task> = store.tasks.iter().filter(|t| !t.done).cloned().collect();
    // Ordering: overdue, due today, priority, then (in your focus hours) hard first.
    open.sort_by_key(|t| {
        let due_rank = match t.due.as_deref() {
            Some(d) if d < today.as_str() => 0,
            Some(d) if d == today.as_str() => 1,
            None => 2,
            Some(_) => 3,
        };
        let hard_rank = if t.hard == deep_time { 0 } else { 1 };
        (due_rank, std::cmp::Reverse(t.priority), hard_rank, t.est_min)
    });

    // Tasks due later than today don't compete for today's time.
    let (future, candidates): (Vec<Task>, Vec<Task>) =
        open.into_iter().partition(|t| t.due.as_deref().is_some_and(|d| d > today.as_str()));

    let now_task = candidates
        .iter()
        .find(|t| t.est_min <= window.max(5))
        .or_else(|| candidates.iter().min_by_key(|t| t.est_min))
        .cloned();

    let mut today_list = Vec::new();
    let mut later = Vec::new();
    let mut used = 0u32;
    for t in candidates {
        if Some(t.id) == now_task.as_ref().map(|n| n.id) {
            used += t.est_min;
            continue;
        }
        if used + t.est_min <= free {
            used += t.est_min;
            today_list.push(t);
        } else {
            later.push(t);
        }
    }
    later.extend(future);

    let tip = if let Some(t) = &now_task {
        if t.est_min > window && window > 0 {
            format!("Only {} before {} - start \"{}\" and pick it up after.", fmt_dur(window),
                next.as_ref().map(|n| n.2.clone()).unwrap_or_default(), t.title)
        } else {
            format!("You've got {}. Do \"{}\" now (~{}).", fmt_dur(window), t.title, fmt_dur(t.est_min))
        }
    } else if free == 0 {
        "Day's done. Rest - tomorrow-you will thank you.".into()
    } else {
        "No tasks yet. Add one and I'll plan your day.".into()
    };

    Plan {
        free_min: free,
        window_min: window,
        next_event: next.map(|(_, text, time)| (time, text)),
        now: now_task,
        today: today_list,
        later,
        best_hours: best,
        tip,
    }
}
