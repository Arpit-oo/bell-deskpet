//! HTTP link to the pet (see src/pet/main.cpp for the API).

use std::time::{Duration, Instant};

use serde_json::{json, Value};

static CURRENT_BASE: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

/// Pet base URL as last discovered (used by the app's /pet proxy).
pub fn current_base() -> Option<String> {
    let b = CURRENT_BASE.lock().ok()?.clone();
    (!b.is_empty()).then_some(b)
}

fn publish_base(b: &str) {
    if let Ok(mut g) = CURRENT_BASE.lock() {
        *g = b.to_string();
    }
}

pub struct Device {
    base: String,
    agent: ureq::Agent,
    online: Option<bool>,
    last_scan: Option<Instant>,
}

impl Device {
    pub fn new(host: &str) -> Self {
        Self::with_timeout(host, Duration::from_millis(3000))
    }

    pub fn with_timeout(host: &str, timeout: Duration) -> Self {
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(timeout.min(Duration::from_millis(1500)))
            .timeout(timeout)
            .build();
        Self { base: format!("http://{host}"), agent, online: None, last_scan: None }
    }

    pub fn is_online(&self) -> bool {
        self.online == Some(true)
    }

    pub fn post(&self, path: &str, body: &Value) -> Result<Value, String> {
        self.agent
            .post(&format!("{}{path}", self.base))
            .send_json(body.clone())
            .map_err(|e| e.to_string())?
            .into_json::<Value>()
            .map_err(|e| e.to_string())
    }

    /// Sends one state update; returns pet events (e.g. "pomo_break") on success.
    /// Logs only when reachability changes.
    pub fn send_state(&mut self, payload: &Value) -> Option<Vec<String>> {
        let res = self.post("/api/state", payload);
        let ok = res.is_ok();
        match (&res, self.online) {
            (Err(e), Some(true) | None) => crate::log(&format!("pet unreachable at {}: {e}", self.base)),
            (Ok(_), Some(false) | None) => {
                crate::log(&format!("pet connected at {}", self.base));
                publish_base(&self.base);
            }
            _ => {}
        }
        self.online = Some(ok);
        if !ok {
            self.rediscover();
        }
        let v = res.ok()?;
        Some(
            v["events"]
                .as_array()
                .map(|a| a.iter().filter_map(|e| e.as_str().map(str::to_string)).collect())
                .unwrap_or_default(),
        )
    }

    /// Robot's DHCP IP on the hotspot can change. When unreachable, look at
    /// the ARP table for 192.168.137.x neighbours and probe each for the pet API.
    fn rediscover(&mut self) {
        if self.last_scan.is_some_and(|t| t.elapsed() < Duration::from_secs(15)) {
            return;
        }
        self.last_scan = Some(Instant::now());
        let mut cmd = std::process::Command::new("arp");
        cmd.arg("-a");
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
        }
        let Ok(out) = cmd.output() else { return };
        let probe = ureq::AgentBuilder::new().timeout(Duration::from_millis(800)).build();
        for line in String::from_utf8_lossy(&out.stdout).lines() {
            let Some(ip) = line.split_whitespace().next() else { continue };
            if !ip.starts_with("192.168.137.") || ip.ends_with(".1") || ip.ends_with(".255") {
                continue;
            }
            let base = format!("http://{ip}");
            if base == self.base {
                continue;
            }
            // Pet answers GET /api/state with 404 + JSON {"ok":false}.
            let resp = match probe.get(&format!("{base}/api/state")).call() {
                Ok(r) | Err(ureq::Error::Status(_, r)) => Some(r),
                Err(_) => None,
            };
            let hit = resp
                .and_then(|r| r.into_json::<Value>().ok())
                .is_some_and(|v| v.get("ok").is_some());
            if hit {
                crate::log(&format!("pet found at {base} (was {})", self.base));
                self.base = base;
                return;
            }
        }
    }

    /// Switch the robot's screen: "next" cycles face -> music -> clock.
    /// Returns the view now showing.
    pub fn set_view(&self, view: &str) -> Option<String> {
        let v = self.post("/api/view", &json!({ "view": view })).ok()?;
        v["view"].as_str().map(str::to_string)
    }

    pub fn send_art(&self, b64: &str) -> bool {
        self.post("/api/art", &json!({ "data": b64 })).is_ok_and(|v| v["ok"] == true)
    }

    pub fn send_claude(&self, state: &str, project: &str) -> bool {
        self.post("/api/claude", &json!({ "state": state, "project": project })).is_ok()
    }

    /// (hype level, pomodoro focus minutes) from the pet's settings.
    pub fn settings(&self) -> Option<(u8, u64)> {
        let v: Value = self.agent.get(&format!("{}/api/settings", self.base)).call().ok()?.into_json().ok()?;
        Some((v["hype"].as_u64().unwrap_or(1) as u8, v["pomo_focus"].as_u64().unwrap_or(25)))
    }
}
