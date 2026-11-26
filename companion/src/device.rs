//! HTTP link to the pet (see src/pet/main.cpp for the API).

use std::time::Duration;

use serde_json::{json, Value};

pub struct Device {
    base: String,
    agent: ureq::Agent,
    online: Option<bool>,
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
        Self { base: format!("http://{host}"), agent, online: None }
    }

    pub fn is_online(&self) -> bool {
        self.online == Some(true)
    }

    fn post(&self, path: &str, body: &Value) -> Result<Value, String> {
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
            (Ok(_), Some(false) | None) => crate::log(&format!("pet connected at {}", self.base)),
            _ => {}
        }
        self.online = Some(ok);
        let v = res.ok()?;
        Some(
            v["events"]
                .as_array()
                .map(|a| a.iter().filter_map(|e| e.as_str().map(str::to_string)).collect())
                .unwrap_or_default(),
        )
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
