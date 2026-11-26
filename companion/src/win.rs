//! Thin safe wrappers over the few Win32 calls we need.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HWND, LPARAM, SYSTEMTIME};
use windows_sys::Win32::System::SystemInformation::{GetLocalTime, GetTickCount};
use windows_sys::Win32::System::Threading::{
    CreateMutexW, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetForegroundWindow, GetWindowTextLengthW, GetWindowTextW, GetWindowThreadProcessId,
};

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

pub fn window_title(hwnd: HWND) -> String {
    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; len as usize + 1];
        let n = GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32);
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }
}

pub fn window_pid(hwnd: HWND) -> u32 {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, &mut pid) };
    pid
}

/// Lower-case exe file name (e.g. "code.exe") for a process id.
pub fn exe_for_pid(pid: u32) -> Option<String> {
    if pid == 0 {
        return None;
    }
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if h.is_null() {
            return None;
        }
        let mut buf = [0u16; 1024];
        let mut size = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, buf.as_mut_ptr(), &mut size);
        CloseHandle(h);
        if ok == 0 {
            return None;
        }
        let path = String::from_utf16_lossy(&buf[..size as usize]);
        path.rsplit(['\\', '/']).next().map(|s| s.to_lowercase())
    }
}

/// pid -> exe cache so window enumeration stays cheap.
pub struct ExeCache {
    map: HashMap<u32, Option<String>>,
    cleared: Instant,
}

impl ExeCache {
    pub fn new() -> Self {
        Self { map: HashMap::new(), cleared: Instant::now() }
    }

    pub fn get(&mut self, pid: u32) -> Option<String> {
        if self.cleared.elapsed() > Duration::from_secs(60) {
            self.map.clear();
            self.cleared = Instant::now();
        }
        self.map.entry(pid).or_insert_with(|| exe_for_pid(pid)).clone()
    }
}

/// (exe, title) of the window the user is currently looking at.
pub fn foreground(cache: &mut ExeCache) -> Option<(String, String)> {
    let hwnd = unsafe { GetForegroundWindow() };
    if hwnd.is_null() {
        return None;
    }
    let exe = cache.get(window_pid(hwnd))?;
    Some((exe, window_title(hwnd)))
}

/// Titles of all top-level windows (including hidden ones) owned by `exe_name`.
pub fn titles_for_exe(exe_name: &str, cache: &mut ExeCache) -> Vec<String> {
    struct Ctx<'a> {
        exe: &'a str,
        cache: &'a mut ExeCache,
        out: Vec<String>,
    }

    unsafe extern "system" fn cb(hwnd: HWND, lparam: LPARAM) -> i32 {
        let ctx = &mut *(lparam as *mut Ctx);
        let title = window_title(hwnd);
        if !title.is_empty() {
            if let Some(exe) = ctx.cache.get(window_pid(hwnd)) {
                if exe == ctx.exe {
                    ctx.out.push(title);
                }
            }
        }
        1 // continue
    }

    let mut ctx = Ctx { exe: exe_name, cache, out: Vec::new() };
    unsafe { EnumWindows(Some(cb), &mut ctx as *mut Ctx as LPARAM) };
    ctx.out
}

/// Milliseconds since the last keyboard/mouse input.
pub fn idle_ms() -> u64 {
    unsafe {
        let mut lii = LASTINPUTINFO { cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32, dwTime: 0 };
        if GetLastInputInfo(&mut lii) == 0 {
            return 0;
        }
        GetTickCount().wrapping_sub(lii.dwTime) as u64
    }
}

/// Local hour (0-23) and a day key (yyyymmdd).
pub fn local_hour_and_day() -> (u32, u32) {
    unsafe {
        let mut st: SYSTEMTIME = std::mem::zeroed();
        GetLocalTime(&mut st);
        (st.wHour as u32, st.wYear as u32 * 10000 + st.wMonth as u32 * 100 + st.wDay as u32)
    }
}

/// Returns false if another instance already holds the mutex.
pub fn single_instance(name: &str) -> bool {
    let w = wide(name);
    unsafe {
        let h = CreateMutexW(std::ptr::null(), 0, w.as_ptr());
        // Intentionally leak the handle: it must live for the whole process.
        !h.is_null() && GetLastError() != ERROR_ALREADY_EXISTS
    }
}

/// Local wall-clock time.
#[derive(Clone, Copy, Debug)]
pub struct LocalTime {
    pub year: u32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
}

impl LocalTime {
    /// "YYYY-MM-DD"
    pub fn date(&self) -> String {
        format!("{:04}-{:02}-{:02}", self.year, self.month, self.day)
    }
    pub fn minutes_of_day(&self) -> u32 {
        self.hour * 60 + self.minute
    }
}

pub fn local_now() -> LocalTime {
    unsafe {
        let mut st: SYSTEMTIME = std::mem::zeroed();
        GetLocalTime(&mut st);
        LocalTime {
            year: st.wYear as u32,
            month: st.wMonth as u32,
            day: st.wDay as u32,
            hour: st.wHour as u32,
            minute: st.wMinute as u32,
        }
    }
}

/// Minimises the window the user is looking at (focus mode).
pub fn minimize_foreground() {
    use windows_sys::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_MINIMIZE};
    unsafe {
        let hwnd = GetForegroundWindow();
        if !hwnd.is_null() {
            ShowWindow(hwnd, SW_MINIMIZE);
        }
    }
}
