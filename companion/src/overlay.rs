//! Bell on your desktop: a borderless, always-on-top, per-pixel-alpha window
//! with the pet's face. Transparent pixels are click-through. Drag to move,
//! click to pet, double-click to open the app, right-click for the menu.
//!
//! Runs on its own thread with its own message loop and renders in step with
//! the desktop compositor (DwmFlush), so it stays smooth even while the main
//! companion loop is waiting on the network.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tiny_skia::{
    BlendMode, Color, FillRule, GradientStop, LinearGradient, Mask, Paint, Pixmap, PixmapPaint, Point,
    RadialGradient, SpreadMode, Stroke, Transform,
};
use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows_sys::Win32::Graphics::Dwm::DwmFlush;
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, GetMonitorInfoW, MonitorFromPoint,
    MonitorFromWindow, ReleaseDC, SelectObject, AC_SRC_ALPHA, AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER,
    BLENDFUNCTION, DIB_RGB_COLORS, HBITMAP, HDC, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::HiDpi::{GetDpiForWindow, SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows_sys::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows_sys::Win32::UI::WindowsAndMessaging::*;

use crate::face::{self, Anim, Face, Mood};

const APP_URL: &str = "http://localhost:7777";

// Logical layout (96-dpi pixels, before the size preset).
const WIN_W: f32 = 290.0;
const WIN_H: f32 = 262.0;
const BODY_W: f32 = 200.0;
const BODY_H: f32 = 124.0;
const BODY_BOTTOM_PAD: f32 = 22.0;
const BODY_R: f32 = 40.0;
const BEZEL: f32 = 7.0;
const BUBBLE_MAX_W: f32 = 254.0;

const SIZES: [(&str, f32); 3] = [("Small", 0.8), ("Medium", 1.0), ("Large", 1.3)];

// ---------------------------------------------------------------- feed (main thread -> overlay)

#[derive(Default)]
struct Feed {
    mood: String,
    say_seq: u64,
    say_text: String,
    say_mood: String,
    say_ms: u32,
    claude: String,
    claude_project: String,
    playing: bool,
    track: String,
    artist: String,
    lyric: String,
    line_ms: u32,
}

#[derive(Clone)]
pub struct Overlay(Arc<Mutex<Feed>>);

impl Overlay {
    pub fn set_mood(&self, mood: &str) {
        let mut f = self.0.lock().unwrap();
        if f.mood != mood {
            f.mood = mood.to_string();
        }
    }
    pub fn say(&self, text: &str, mood: &str, ms: u32) {
        let mut f = self.0.lock().unwrap();
        f.say_seq += 1;
        f.say_text = text.to_string();
        f.say_mood = mood.to_string();
        f.say_ms = ms;
    }
    pub fn set_claude(&self, state: &str, project: &str) {
        let mut f = self.0.lock().unwrap();
        f.claude = state.to_string();
        f.claude_project = project.to_string();
    }
    pub fn set_music(&self, playing: bool, track: &str, artist: &str, lyric: &str, line_ms: u32) {
        let mut f = self.0.lock().unwrap();
        f.playing = playing;
        if f.track != track {
            f.track = track.to_string();
        }
        if f.artist != artist {
            f.artist = artist.to_string();
        }
        if f.lyric != lyric {
            f.lyric = lyric.to_string();
        }
        f.line_ms = line_ms;
    }
}

pub fn start() -> Overlay {
    let feed = Arc::new(Mutex::new(Feed { mood: "idle".into(), ..Default::default() }));
    let f2 = feed.clone();
    std::thread::Builder::new()
        .name("overlay".into())
        .spawn(move || {
            if let Err(e) = run(f2) {
                crate::log(&format!("overlay stopped: {e}"));
            }
        })
        .expect("spawn overlay thread");
    Overlay(feed)
}

// ---------------------------------------------------------------- persisted prefs

#[derive(Serialize, Deserialize, Clone)]
struct Prefs {
    x: Option<i32>,
    y: Option<i32>,
    size: usize,
    lyrics: bool,
    follow_mouse: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs { x: None, y: None, size: 1, lyrics: true, follow_mouse: true }
    }
}

fn prefs_path() -> std::path::PathBuf {
    let base = std::env::var_os("APPDATA").map(std::path::PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("deskpet").join("overlay.json")
}

impl Prefs {
    fn load() -> Prefs {
        std::fs::read_to_string(prefs_path()).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
    }
    fn save(&self) {
        let p = prefs_path();
        if let Some(d) = p.parent() {
            let _ = std::fs::create_dir_all(d);
        }
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(p, s);
        }
    }
}

// ---------------------------------------------------------------- text

struct Text {
    font: fontdue::Font,
    cache: HashMap<(char, u32), (fontdue::Metrics, Vec<u8>)>,
}

impl Text {
    fn load() -> Option<Text> {
        let dir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
        for f in ["seguisb.ttf", "segoeuib.ttf", "segoeui.ttf", "arial.ttf"] {
            if let Ok(bytes) = std::fs::read(format!("{dir}\\Fonts\\{f}")) {
                if let Ok(font) = fontdue::Font::from_bytes(bytes, fontdue::FontSettings::default()) {
                    return Some(Text { font, cache: HashMap::new() });
                }
            }
        }
        None
    }

    fn glyph(&mut self, ch: char, px: f32) -> &(fontdue::Metrics, Vec<u8>) {
        let key = (ch, (px * 4.0) as u32);
        if self.cache.len() > 4000 {
            self.cache.clear();
        }
        let font = &self.font;
        self.cache.entry(key).or_insert_with(|| font.rasterize(ch, key.1 as f32 / 4.0))
    }

    fn width(&mut self, s: &str, px: f32) -> f32 {
        s.chars().map(|c| self.glyph(c, px).0.advance_width).sum()
    }

    /// Draws `s` with its baseline at `y`. `col` is straight (non-premultiplied) RGBA.
    fn draw(&mut self, pm: &mut Pixmap, s: &str, x: f32, y: f32, px: f32, col: [u8; 4]) {
        let (pw, ph) = (pm.width() as i32, pm.height() as i32);
        let mut pen = x;
        for ch in s.chars() {
            let (m, bmp) = {
                let g = self.glyph(ch, px);
                (g.0, g.1.clone())
            };
            let gx = (pen + m.xmin as f32).round() as i32;
            let gy = (y - (m.height as i32 + m.ymin) as f32).round() as i32;
            let data = pm.data_mut();
            for row in 0..m.height as i32 {
                let yy = gy + row;
                if yy < 0 || yy >= ph {
                    continue;
                }
                for colx in 0..m.width as i32 {
                    let xx = gx + colx;
                    if xx < 0 || xx >= pw {
                        continue;
                    }
                    let cov = bmp[(row * m.width as i32 + colx) as usize] as u32;
                    if cov == 0 {
                        continue;
                    }
                    let sa = cov * col[3] as u32 / 255;
                    let i = ((yy * pw + xx) * 4) as usize;
                    let inv = 255 - sa;
                    for c in 0..3 {
                        data[i + c] = ((col[c] as u32 * sa + data[i + c] as u32 * inv) / 255) as u8;
                    }
                    data[i + 3] = (sa + data[i + 3] as u32 * inv / 255) as u8;
                }
            }
            pen += m.advance_width;
        }
    }

    fn wrap(&mut self, s: &str, px: f32, max_w: f32, max_lines: usize) -> Vec<String> {
        let mut lines: Vec<String> = Vec::new();
        let mut cur = String::new();
        for word in s.split_whitespace() {
            let trial = if cur.is_empty() { word.to_string() } else { format!("{cur} {word}") };
            if self.width(&trial, px) <= max_w || cur.is_empty() {
                cur = trial;
            } else {
                lines.push(std::mem::take(&mut cur));
                cur = word.to_string();
            }
        }
        if !cur.is_empty() {
            lines.push(cur);
        }
        if lines.len() > max_lines {
            lines.truncate(max_lines);
            let last = lines.last_mut().unwrap();
            while !last.is_empty() && self.width(&format!("{last}…"), px) > max_w {
                last.pop();
            }
            last.push('…');
        }
        // Hard-clip single words that are wider than the bubble.
        for l in lines.iter_mut() {
            while l.chars().count() > 1 && self.width(l, px) > max_w {
                l.pop();
            }
        }
        lines
    }
}

// ---------------------------------------------------------------- bubble

#[derive(Clone, Copy, PartialEq, Eq)]
enum BubbleKind {
    Say,
    Claude,
    Track,
    Lyric,
}

struct Bubble {
    kind: BubbleKind,
    text: String,
    start: f32,
    until: f32,
}

// ---------------------------------------------------------------- input from the window proc

#[derive(Clone, Copy)]
enum Input {
    LDown,
    LUp,
    LDouble,
    RUp,
}

thread_local! {
    static INPUTS: std::cell::RefCell<Vec<Input>> = const { std::cell::RefCell::new(Vec::new()) };
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let push = |i: Input| INPUTS.with(|q| q.borrow_mut().push(i));
    match msg {
        WM_LBUTTONDOWN => {
            push(Input::LDown);
            0
        }
        WM_LBUTTONUP => {
            push(Input::LUp);
            0
        }
        WM_LBUTTONDBLCLK => {
            push(Input::LDouble);
            0
        }
        WM_RBUTTONUP => {
            push(Input::RUp);
            0
        }
        WM_MOUSEACTIVATE => MA_NOACTIVATE as LRESULT,
        WM_DESTROY => {
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn cursor() -> (i32, i32) {
    let mut p = POINT { x: 0, y: 0 };
    unsafe { GetCursorPos(&mut p) };
    (p.x, p.y)
}

fn work_area_at(x: i32, y: i32) -> RECT {
    unsafe {
        let mon = MonitorFromPoint(POINT { x, y }, MONITOR_DEFAULTTONEAREST);
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        GetMonitorInfoW(mon, &mut mi);
        mi.rcWork
    }
}

/// True when the foreground window covers its whole monitor (games, videos, slides).
fn fullscreen_app_active(me: HWND) -> bool {
    unsafe {
        let fg = GetForegroundWindow();
        if fg.is_null() || fg == me {
            return false;
        }
        let mut cls = [0u16; 64];
        let n = GetClassNameW(fg, cls.as_mut_ptr(), cls.len() as i32);
        let cls = String::from_utf16_lossy(&cls[..n.max(0) as usize]);
        if matches!(cls.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd") {
            return false;
        }
        // Maximised windows overhang the monitor too; real fullscreen has no caption.
        if IsZoomed(fg) != 0 || (GetWindowLongW(fg, GWL_STYLE) as u32 & WS_CAPTION) == WS_CAPTION {
            return false;
        }
        let mut r: RECT = std::mem::zeroed();
        if GetWindowRect(fg, &mut r) == 0 {
            return false;
        }
        let mon = MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST);
        let mut mi: MONITORINFO = std::mem::zeroed();
        mi.cbSize = std::mem::size_of::<MONITORINFO>() as u32;
        GetMonitorInfoW(mon, &mut mi);
        let m = mi.rcMonitor;
        r.left <= m.left && r.top <= m.top && r.right >= m.right && r.bottom >= m.bottom
    }
}

fn open_app() {
    let _ = std::process::Command::new("explorer").arg(APP_URL).spawn();
}

/// Rough syllable count for lip-sync.
fn syllables(s: &str) -> u32 {
    let mut n = 0;
    let mut prev = false;
    for c in s.chars() {
        let v = matches!(c.to_ascii_lowercase(), 'a' | 'e' | 'i' | 'o' | 'u' | 'y');
        if v && !prev {
            n += 1;
        }
        prev = v;
    }
    n.max(if s.trim().is_empty() { 0 } else { 1 })
}

fn ease_out_back(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let c1 = 1.70158;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

fn approach(cur: f32, target: f32, dt: f32, tau: f32) -> f32 {
    cur + (target - cur) * (1.0 - (-dt / tau).exp())
}

// ---------------------------------------------------------------- surface (DIB for UpdateLayeredWindow)

struct Surface {
    w: i32,
    h: i32,
    screen: HDC,
    mem: HDC,
    bmp: HBITMAP,
    old: *mut core::ffi::c_void,
    bits: *mut u8,
}

impl Surface {
    fn new(w: i32, h: i32) -> Option<Surface> {
        unsafe {
            let screen = GetDC(std::ptr::null_mut());
            let mem = CreateCompatibleDC(screen);
            let mut bi: BITMAPINFO = std::mem::zeroed();
            bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
            bi.bmiHeader.biWidth = w;
            bi.bmiHeader.biHeight = -h; // top-down
            bi.bmiHeader.biPlanes = 1;
            bi.bmiHeader.biBitCount = 32;
            let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
            let bmp = CreateDIBSection(mem, &bi, DIB_RGB_COLORS, &mut bits, std::ptr::null_mut(), 0);
            if bmp.is_null() || bits.is_null() {
                DeleteDC(mem);
                ReleaseDC(std::ptr::null_mut(), screen);
                return None;
            }
            let old = SelectObject(mem, bmp);
            Some(Surface { w, h, screen, mem, bmp, old, bits: bits as *mut u8 })
        }
    }

    fn present(&mut self, hwnd: HWND, pm: &Pixmap, x: i32, y: i32, alpha: u8) {
        let n = (self.w * self.h * 4) as usize;
        let src = pm.data();
        let dst = unsafe { std::slice::from_raw_parts_mut(self.bits, n) };
        for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
            d[0] = s[2];
            d[1] = s[1];
            d[2] = s[0];
            d[3] = s[3];
        }
        let pos = POINT { x, y };
        let size = SIZE { cx: self.w, cy: self.h };
        let zero = POINT { x: 0, y: 0 };
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: alpha,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        unsafe {
            UpdateLayeredWindow(hwnd, self.screen, &pos, &size, self.mem, &zero, 0, &blend, ULW_ALPHA);
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.mem, self.old);
            DeleteObject(self.bmp);
            DeleteDC(self.mem);
            ReleaseDC(std::ptr::null_mut(), self.screen);
        }
    }
}

// ---------------------------------------------------------------- the pet

struct Pet {
    hwnd: HWND,
    prefs: Prefs,
    face: Face,
    text: Option<Text>,
    t0: Instant,

    scale: f32, // dpi * size preset
    dpi: u32,
    w: i32,
    h: i32,
    x: i32,
    y: i32,
    canvas: Pixmap,
    layer: Pixmap,
    fx: Pixmap,
    surface: Surface,

    // interaction
    pressed: Option<((i32, i32), (i32, i32), f32)>, // cursor at press, window pos at press, time
    dragging: bool,
    hover: f32,
    squash: f32,
    last_cursor: (i32, i32),
    cursor_moved_at: f32,
    temp_mood: Option<(Mood, f32)>,

    // feed tracking
    say_seq: u64,
    last_track: String,
    last_lyric: String,
    lyric_at: f32,
    claude: String,
    claude_since: f32,

    bubble: Option<Bubble>,
    bubble_a: f32,
    hidden_until: Option<Instant>,
    fs_hidden: bool,
    alpha: f32,
    last_checks: f32,
    appear: f32,
}

fn new_pixmap(w: i32, h: i32) -> Result<Pixmap, String> {
    Pixmap::new(w as u32, h as u32).ok_or_else(|| "pixmap alloc failed".to_string())
}

impl Pet {
    fn ms(&self) -> f32 {
        self.t0.elapsed().as_secs_f32() * 1000.0
    }

    fn resize(&mut self) -> Result<(), String> {
        let dpi = unsafe { GetDpiForWindow(self.hwnd) }.max(96);
        let scale = dpi as f32 / 96.0 * SIZES[self.prefs.size.min(SIZES.len() - 1)].1;
        let (w, h) = ((WIN_W * scale).round() as i32, (WIN_H * scale).round() as i32);
        if w == self.w && h == self.h && dpi == self.dpi {
            return Ok(());
        }
        // Keep the body's bottom-centre anchored when the size changes.
        if self.w > 0 {
            self.x += (self.w - w) / 2;
            self.y += self.h - h;
        }
        self.dpi = dpi;
        self.scale = scale;
        self.w = w;
        self.h = h;
        self.canvas = new_pixmap(w, h)?;
        self.layer = new_pixmap(w, h)?;
        self.fx = new_pixmap(w, h)?;
        self.surface = Surface::new(w, h).ok_or("DIB alloc failed")?;
        Ok(())
    }

    fn body_rect(&self) -> (f32, f32, f32, f32) {
        let s = self.scale;
        let pop = 1.0 + 0.035 * self.hover - 0.06 * self.squash;
        let (bw, bh) = (BODY_W * s * (1.0 + 0.03 * self.squash) * pop, BODY_H * s * pop * (1.0 - 0.04 * self.squash));
        let cx = self.w as f32 / 2.0;
        let bottom = self.h as f32 - BODY_BOTTOM_PAD * s;
        let appear = ease_out_back(self.appear);
        let (bw, bh) = (bw * (0.6 + 0.4 * appear), bh * (0.6 + 0.4 * appear));
        (cx - bw / 2.0, bottom - bh, bw, bh)
    }

    fn body_contains(&self, sx: i32, sy: i32) -> bool {
        let (bx, by, bw, bh) = self.body_rect();
        let (lx, ly) = ((sx - self.x) as f32, (sy - self.y) as f32);
        lx >= bx && lx <= bx + bw && ly >= by && ly <= by + bh
    }

    fn bubble_rect(&mut self) -> Option<(f32, f32, f32, f32, Vec<String>, f32)> {
        let b = self.bubble.as_ref()?;
        let s = self.scale;
        let px = match b.kind {
            BubbleKind::Lyric | BubbleKind::Track => 13.0 * s,
            _ => 14.5 * s,
        };
        let text = b.text.clone();
        let t = self.text.as_mut()?;
        let lines = t.wrap(&text, px, (BUBBLE_MAX_W - 28.0) * s, 3);
        let tw = lines.iter().map(|l| t.width(l, px)).fold(0.0, f32::max);
        let lh = px * 1.32;
        let (pad_x, pad_y) = (14.0 * s, 9.0 * s);
        let bw = (tw + 2.0 * pad_x).max(54.0 * s);
        let bh = lines.len() as f32 * lh + 2.0 * pad_y;
        let (bx0, by0, _, _) = self.body_rect();
        let _ = bx0;
        let x = ((self.w as f32 - bw) / 2.0).max(2.0);
        let y = by0 - 12.0 * s - bh;
        Some((x, y.max(2.0), bw, bh, lines, px))
    }

    // ------------------------------------------------------------ per-frame logic

    fn handle_input(&mut self, now: f32) {
        let inputs: Vec<Input> = INPUTS.with(|q| std::mem::take(&mut *q.borrow_mut()));
        for i in inputs {
            match i {
                Input::LDown => {
                    unsafe { SetCapture(self.hwnd) };
                    self.pressed = Some((cursor(), (self.x, self.y), now));
                    self.dragging = false;
                    self.squash = 1.0;
                }
                Input::LUp => {
                    unsafe { ReleaseCapture() };
                    let was_drag = self.dragging;
                    self.pressed = None;
                    self.dragging = false;
                    if was_drag {
                        self.prefs.x = Some(self.x);
                        self.prefs.y = Some(self.y);
                        self.prefs.save();
                        self.face.play(Anim::Wiggle);
                        self.temp_mood = Some((Mood::Happy, now + 1200.0));
                    } else {
                        let (cx, cy) = cursor();
                        let in_bubble = self.bubble_rect().is_some_and(|(x, y, w, h, _, _)| {
                            let (lx, ly) = ((cx - self.x) as f32, (cy - self.y) as f32);
                            lx >= x && lx <= x + w && ly >= y && ly <= y + h
                        });
                        if in_bubble {
                            if let Some(b) = self.bubble.as_mut() {
                                b.until = now;
                            }
                        } else {
                            self.pet(now);
                        }
                    }
                }
                Input::LDouble => open_app(),
                Input::RUp => self.menu(),
            }
        }

        if let Some((c0, w0, _)) = self.pressed {
            let c = cursor();
            let (dx, dy) = (c.0 - c0.0, c.1 - c0.1);
            if !self.dragging && dx * dx + dy * dy > 16 {
                self.dragging = true;
            }
            if self.dragging {
                self.x = w0.0 + dx;
                self.y = w0.1 + dy;
            }
        }
    }

    fn pet(&mut self, now: f32) {
        let pick = (now as u64 / 7) % 3;
        match pick {
            0 => {
                self.temp_mood = Some((Mood::Love, now + 1600.0));
            }
            1 => {
                self.temp_mood = Some((Mood::Happy, now + 1400.0));
                self.face.play(Anim::Peek);
            }
            _ => {
                self.temp_mood = Some((Mood::Excited, now + 1200.0));
            }
        }
    }

    fn menu(&mut self) {
        const OPEN: usize = 1;
        const LYRICS: usize = 2;
        const FOLLOW: usize = 3;
        const HIDE: usize = 4;
        const QUIT: usize = 5;
        const SIZE0: usize = 10;
        const RESET: usize = 20;
        unsafe {
            let m = CreatePopupMenu();
            let sub = CreatePopupMenu();
            for (i, (name, _)) in SIZES.iter().enumerate() {
                let flags = MF_STRING | if i == self.prefs.size { MF_CHECKED } else { 0 };
                AppendMenuW(sub, flags, SIZE0 + i, wide(name).as_ptr());
            }
            AppendMenuW(m, MF_STRING, OPEN, wide("Open Bell app").as_ptr());
            AppendMenuW(m, MF_SEPARATOR, 0, std::ptr::null());
            AppendMenuW(m, MF_STRING | MF_POPUP, sub as usize, wide("Size").as_ptr());
            AppendMenuW(m, MF_STRING | if self.prefs.lyrics { MF_CHECKED } else { 0 }, LYRICS, wide("Show lyrics").as_ptr());
            AppendMenuW(m, MF_STRING | if self.prefs.follow_mouse { MF_CHECKED } else { 0 }, FOLLOW, wide("Eyes follow mouse").as_ptr());
            AppendMenuW(m, MF_STRING, RESET, wide("Move to corner").as_ptr());
            AppendMenuW(m, MF_STRING, HIDE, wide("Hide for 30 minutes").as_ptr());
            AppendMenuW(m, MF_SEPARATOR, 0, std::ptr::null());
            AppendMenuW(m, MF_STRING, QUIT, wide("Quit Bell").as_ptr());
            let (cx, cy) = cursor();
            SetForegroundWindow(self.hwnd);
            let cmd = TrackPopupMenu(m, TPM_RETURNCMD | TPM_RIGHTBUTTON | TPM_BOTTOMALIGN, cx, cy, 0, self.hwnd, std::ptr::null()) as usize;
            DestroyMenu(m);
            match cmd {
                OPEN => open_app(),
                LYRICS => self.prefs.lyrics = !self.prefs.lyrics,
                FOLLOW => self.prefs.follow_mouse = !self.prefs.follow_mouse,
                HIDE => self.hidden_until = Some(Instant::now() + Duration::from_secs(30 * 60)),
                RESET => {
                    self.place_default();
                    self.prefs.x = None;
                    self.prefs.y = None;
                }
                QUIT => {
                    crate::log("quit from overlay menu");
                    std::process::exit(0);
                }
                c if (SIZE0..SIZE0 + SIZES.len()).contains(&c) => self.prefs.size = c - SIZE0,
                _ => {}
            }
            if cmd != 0 {
                self.prefs.save();
            }
        }
    }

    fn place_default(&mut self) {
        let (cx, cy) = cursor();
        let wa = work_area_at(cx, cy);
        self.x = wa.right - self.w - (16.0 * self.scale) as i32;
        self.y = wa.bottom - self.h + (6.0 * self.scale) as i32;
    }

    fn sync_feed(&mut self, feed: &Mutex<Feed>, now: f32) -> Mood {
        let f = feed.lock().unwrap();

        if f.say_seq != self.say_seq {
            self.say_seq = f.say_seq;
            if !f.say_text.is_empty() {
                let ms = f.say_ms.clamp(2500, 15000) as f32;
                self.bubble = Some(Bubble { kind: BubbleKind::Say, text: f.say_text.clone(), start: now, until: now + ms });
                self.temp_mood = Some((Mood::parse(&f.say_mood), now + ms.min(5000.0)));
            }
        }

        if f.claude != self.claude {
            self.claude = f.claude.clone();
            self.claude_since = now;
            match self.claude.as_str() {
                "waiting" => {
                    let what = if f.claude_project.is_empty() { String::new() } else { format!(" ({})", f.claude_project) };
                    let b = Bubble { kind: BubbleKind::Claude, text: format!("Claude needs you{what}"), start: now, until: f32::MAX };
                    if self.bubble.as_ref().is_none_or(|x| x.kind != BubbleKind::Say) {
                        self.bubble = Some(b);
                    }
                }
                "done" => {
                    self.temp_mood = Some((Mood::Happy, now + 2500.0));
                    if self.bubble.as_ref().is_some_and(|x| x.kind == BubbleKind::Claude) {
                        self.bubble = None;
                    }
                }
                _ => {
                    if self.bubble.as_ref().is_some_and(|x| x.kind == BubbleKind::Claude) {
                        self.bubble = None;
                    }
                }
            }
        }

        if f.playing && !f.track.is_empty() && f.track != self.last_track {
            let text = if f.artist.is_empty() { f.track.clone() } else { format!("{} — {}", f.track, f.artist) };
            if self.bubble.as_ref().is_none_or(|b| matches!(b.kind, BubbleKind::Lyric | BubbleKind::Track)) {
                self.bubble = Some(Bubble { kind: BubbleKind::Track, text, start: now, until: now + 5000.0 });
            }
        }
        if !f.track.is_empty() {
            self.last_track = f.track.clone();
        }

        // Sing-along.
        if f.lyric != self.last_lyric {
            self.last_lyric = f.lyric.clone();
            self.lyric_at = now;
            if self.prefs.lyrics && f.playing && !f.lyric.is_empty()
                && self.bubble.as_ref().is_none_or(|b| b.kind == BubbleKind::Lyric || (b.kind == BubbleKind::Track && now - b.start > 2500.0))
            {
                let ms = if f.line_ms > 0 { f.line_ms as f32 } else { 4000.0 };
                self.bubble = Some(Bubble { kind: BubbleKind::Lyric, text: f.lyric.clone(), start: now, until: now + ms.clamp(1500.0, 9000.0) });
            }
        }
        if f.playing && !f.lyric.is_empty() {
            self.face.set_sing(2, self.lyric_at, syllables(&f.lyric), f.line_ms as f32);
        } else if f.playing {
            self.face.set_sing(1, 0.0, 0, 0.0);
        } else {
            self.face.set_sing(0, 0.0, 0, 0.0);
        }

        // Mood priority: temporary > dragging > Claude > music > activity.
        if self.dragging {
            return Mood::Surprised;
        }
        if let Some((m, until)) = self.temp_mood {
            if now < until {
                return m;
            }
            self.temp_mood = None;
        }
        match self.claude.as_str() {
            "waiting" => return if now - self.claude_since < 1500.0 { Mood::Surprised } else { Mood::Happy },
            "working" => return Mood::Focus,
            _ => {}
        }
        if f.playing {
            return Mood::Music;
        }
        Mood::parse(&f.mood)
    }

    fn frame(&mut self, feed: &Mutex<Feed>, dt: f32) -> Result<(), String> {
        let now = self.ms();
        self.resize()?;
        self.handle_input(now);

        let mood = self.sync_feed(feed, now);
        self.face.set_mood(mood);

        // Hover + gaze.
        let c = cursor();
        if c != self.last_cursor {
            self.last_cursor = c;
            self.cursor_moved_at = now;
        }
        let hovering = self.body_contains(c.0, c.1) || self.pressed.is_some();
        self.hover = approach(self.hover, if hovering { 1.0 } else { 0.0 }, dt, 90.0);
        self.squash = approach(self.squash, if self.pressed.is_some() && !self.dragging { 0.6 } else { 0.0 }, dt, 70.0);
        self.appear = (self.appear + dt / 450.0).min(1.0);

        let (bx, by, bw, bh) = self.body_rect();
        let fcx = self.x as f32 + bx + bw / 2.0;
        let fcy = self.y as f32 + by + bh / 2.0;
        let gaze = if self.prefs.follow_mouse && (now - self.cursor_moved_at < 2500.0 || hovering) {
            let dx = (c.0 as f32 - fcx) / (420.0 * self.scale);
            let dy = (c.1 as f32 - fcy) / (320.0 * self.scale);
            Some((dx, dy))
        } else {
            None
        };
        self.face.set_gaze(gaze);
        self.face.update(now);

        // Bubble lifetime.
        if self.bubble.as_ref().is_some_and(|b| now > b.until) && self.bubble_a < 0.02 {
            self.bubble = None;
        }
        let bubble_on = self.bubble.as_ref().is_some_and(|b| now <= b.until);
        self.bubble_a = approach(self.bubble_a, if bubble_on { 1.0 } else { 0.0 }, dt, 70.0);

        // Visibility: manual hide, fullscreen apps.
        if now - self.last_checks > 1000.0 {
            self.last_checks = now;
            self.fs_hidden = fullscreen_app_active(self.hwnd);
            unsafe {
                SetWindowPos(self.hwnd, HWND_TOPMOST, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
            }
        }
        if self.hidden_until.is_some_and(|t| Instant::now() >= t) {
            self.hidden_until = None;
        }
        let visible = !self.fs_hidden && self.hidden_until.is_none();
        self.alpha = approach(self.alpha, if visible { 1.0 } else { 0.0 }, dt, 110.0);

        self.render(now);
        let a = (self.alpha * 255.0).round() as u8;
        self.surface.present(self.hwnd, &self.canvas, self.x, self.y, a);
        Ok(())
    }

    // ------------------------------------------------------------ drawing

    fn render(&mut self, now: f32) {
        let s = self.scale;
        self.canvas.fill(Color::TRANSPARENT);
        self.layer.fill(Color::TRANSPARENT);
        self.fx.fill(Color::TRANSPARENT);

        let (bx, by, bw, bh) = self.body_rect();
        let r = BODY_R * s * (bw / (BODY_W * s));

        // Soft shadow.
        for i in 0..6 {
            let g = i as f32 * 2.2 * s;
            let a = (16 - i * 2) as u8;
            face::rrect(&mut self.canvas, bx - g + 2.0 * s, by - g + 9.0 * s, bw + 2.0 * g - 4.0 * s, bh + 2.0 * g - 6.0 * s, r + g, Color::from_rgba8(0, 0, 0, a), BlendMode::SourceOver);
        }

        // Shell (bezel) with a vertical gradient + rim light.
        if let Some(path) = face::rrect_path(bx, by, bw, bh, r) {
            let mut p = Paint::default();
            p.anti_alias = true;
            p.shader = LinearGradient::new(
                Point::from_xy(0.0, by),
                Point::from_xy(0.0, by + bh),
                vec![
                    GradientStop::new(0.0, Color::from_rgba8(58, 62, 74, 255)),
                    GradientStop::new(1.0, Color::from_rgba8(22, 24, 30, 255)),
                ],
                SpreadMode::Pad,
                Transform::identity(),
            )
            .unwrap_or(tiny_skia::Shader::SolidColor(Color::from_rgba8(30, 32, 40, 255)));
            self.canvas.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
            let mut st = Paint::default();
            st.anti_alias = true;
            st.set_color(Color::from_rgba8(255, 255, 255, 34));
            self.canvas.stroke_path(&path, &st, &Stroke { width: 1.2 * s, ..Default::default() }, Transform::identity(), None);
        }

        // Screen.
        let in_ = BEZEL * s * (bw / (BODY_W * s));
        let (sx, sy, sw, shh, sr) = (bx + in_, by + in_, bw - 2.0 * in_, bh - 2.0 * in_, (r - in_).max(4.0));
        face::rrect(&mut self.canvas, sx, sy, sw, shh, sr, Color::from_rgba8(5, 6, 9, 255), BlendMode::SourceOver);

        // Faint glow of the eye colour behind the eyes.
        let col = self.face.color();
        if let Some(path) = face::rrect_path(sx, sy, sw, shh, sr) {
            let mut p = Paint::default();
            p.anti_alias = true;
            if let Some(sh) = RadialGradient::new(
                Point::from_xy(sx + sw / 2.0, sy + shh / 2.0),
                Point::from_xy(sx + sw / 2.0, sy + shh / 2.0),
                sw * 0.55,
                vec![GradientStop::new(0.0, face::rgba(col, 40)), GradientStop::new(1.0, face::rgba(col, 0))],
                SpreadMode::Pad,
                Transform::identity(),
            ) {
                p.shader = sh;
                self.canvas.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
            }
        }

        // Face, clipped to the screen.
        let k = sw / 300.0;
        self.face.draw(&mut self.layer, &mut self.fx, sx + sw / 2.0, sy + shh / 2.0 + 2.0 * s, k);

        if let (Some(mut mask), Some(path)) = (Mask::new(self.w as u32, self.h as u32), face::rrect_path(sx, sy, sw, shh, sr)) {
            mask.fill_path(&path, FillRule::Winding, true, Transform::identity());
            self.canvas.draw_pixmap(0, 0, self.layer.as_ref(), &PixmapPaint::default(), Transform::identity(), Some(&mask));
        }

        // Glass glare.
        if let Some(path) = face::rrect_path(sx, sy, sw, shh * 0.45, sr) {
            let mut p = Paint::default();
            p.anti_alias = true;
            if let Some(sh) = LinearGradient::new(
                Point::from_xy(0.0, sy),
                Point::from_xy(0.0, sy + shh * 0.45),
                vec![GradientStop::new(0.0, Color::from_rgba8(255, 255, 255, 14)), GradientStop::new(1.0, Color::from_rgba8(255, 255, 255, 0))],
                SpreadMode::Pad,
                Transform::identity(),
            ) {
                p.shader = sh;
                self.canvas.fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
            }
        }

        // Floating z's while sleepy.
        let zzz = self.face.zzz();
        if zzz > 0.3 {
            if let Some(tx) = self.text.as_mut() {
                let x = sx + sw * 0.83;
                let y = sy + shh * 0.25;
                for i in 0..3 {
                    let ph = ((now / 1000.0 + i as f32 * 0.7) % 2.1) / 2.1;
                    if ph > 0.85 {
                        continue;
                    }
                    let a = ((1.0 - ph / 0.85) * 230.0 * zzz) as u8;
                    let px = (11.0 + 4.0 * i as f32) * s;
                    tx.draw(&mut self.fx, "z", x + i as f32 * 10.0 * s + (ph * 6.0).sin() * 3.0 * s, y - ph * 46.0 * s, px, [140, 190, 230, a]);
                }
            }
        }

        self.canvas.draw_pixmap(0, 0, self.fx.as_ref(), &PixmapPaint::default(), Transform::identity(), None);

        self.draw_claude_badge(now, bx + bw - 16.0 * s, by + 14.0 * s);
        self.draw_bubble(now);
    }

    fn draw_claude_badge(&mut self, now: f32, cx: f32, cy: f32) {
        let st = self.claude.as_str();
        if st != "working" && st != "waiting" {
            return;
        }
        let s = self.scale;
        let waiting = st == "waiting";
        let pulse = if waiting { 0.5 + 0.5 * (now / 200.0).sin() } else { 0.0 };
        let r = (if waiting { 7.0 + 3.0 * pulse } else { 6.0 }) * s;
        let spin = now / if waiting { 700.0 } else { 260.0 };
        let orange = Color::from_rgba8(217, 119, 87, 255);
        face::oval(&mut self.canvas, cx, cy, r + 4.0 * s, r + 4.0 * s, Color::from_rgba8(12, 13, 17, 230), BlendMode::SourceOver);
        for i in 0..4 {
            let a = i as f32 * std::f32::consts::PI / 4.0 + spin;
            let (dx, dy) = (a.cos() * r, a.sin() * r);
            face::line(&mut self.canvas, cx - dx, cy - dy, cx + dx, cy + dy, 2.2 * s, orange);
        }
    }

    fn draw_bubble(&mut self, now: f32) {
        if self.bubble_a < 0.01 {
            return;
        }
        let Some((x, y, w, h, lines, px)) = self.bubble_rect() else { return };
        let b = self.bubble.as_ref().unwrap();
        let (kind, start) = (b.kind, b.start);
        let s = self.scale;
        let a = self.bubble_a;
        let pop = ease_out_back(((now - start) / 260.0).min(1.0));
        let grow = 0.85 + 0.15 * pop;
        // Scale around the tail point.
        let (ax, ay) = (self.w as f32 / 2.0, y + h);
        let (x, y, w, h) = (ax + (x - ax) * grow, ay + (y - ay) * grow, w * grow, h * grow);

        let (bg, fg): (Color, [u8; 3]) = match kind {
            BubbleKind::Say => (Color::from_rgba8(250, 249, 245, 255), [24, 26, 32]),
            BubbleKind::Claude => (Color::from_rgba8(250, 249, 245, 255), [200, 98, 66]),
            BubbleKind::Track => (Color::from_rgba8(18, 20, 26, 238), [235, 238, 245]),
            BubbleKind::Lyric => {
                let c = self.face.color();
                (Color::from_rgba8(14, 16, 22, 225), [c[0] as u8, c[1] as u8, c[2] as u8])
            }
        };
        let mut bg = bg;
        bg.apply_opacity(a);
        let r = (12.0 * s).min(h / 2.0);

        // Shadow, body, tail.
        face::rrect(&mut self.canvas, x + 1.0 * s, y + 3.0 * s, w, h, r, Color::from_rgba8(0, 0, 0, (40.0 * a) as u8), BlendMode::SourceOver);
        face::rrect(&mut self.canvas, x, y, w, h, r, bg, BlendMode::SourceOver);
        let tx = self.w as f32 / 2.0 + 18.0 * s;
        let mut pb = tiny_skia::PathBuilder::new();
        pb.move_to(tx - 8.0 * s, y + h - 1.0);
        pb.line_to(tx + 6.0 * s, y + h - 1.0);
        pb.line_to(tx - 6.0 * s, y + h + 9.0 * s * grow);
        pb.close();
        if let Some(p) = pb.finish() {
            let mut paint = Paint::default();
            paint.anti_alias = true;
            paint.set_color(bg);
            self.canvas.fill_path(&p, &paint, FillRule::Winding, Transform::identity(), None);
        }

        // Typewriter for spoken lines; lyrics/track appear whole.
        let total: usize = lines.iter().map(|l| l.chars().count()).sum();
        let shown = match kind {
            BubbleKind::Say | BubbleKind::Claude => (((now - start) / 24.0) as usize).min(total),
            _ => total,
        };
        let px = px * grow;
        let lh = px * 1.32;
        let pad_y = 9.0 * s * grow;
        let Some(t) = self.text.as_mut() else { return };
        let mut left = shown;
        for (i, l) in lines.iter().enumerate() {
            if left == 0 {
                break;
            }
            let n = l.chars().count().min(left);
            left -= n;
            let part: String = l.chars().take(n).collect();
            let lw = t.width(l, px);
            let lx = x + (w - lw) / 2.0;
            let ly = y + pad_y + lh * i as f32 + px * 1.0;
            t.draw(&mut self.canvas, &part, lx, ly, px, [fg[0], fg[1], fg[2], (255.0 * a) as u8]);
        }
    }
}

fn run(feed: Arc<Mutex<Feed>>) -> Result<(), String> {
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let hinst = GetModuleHandleW(std::ptr::null());
        let class = wide("BellDeskPet");
        let wc = WNDCLASSW {
            style: CS_DBLCLKS,
            lpfnWndProc: Some(wndproc),
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: hinst,
            hIcon: std::ptr::null_mut(),
            hCursor: LoadCursorW(std::ptr::null_mut(), IDC_HAND),
            hbrBackground: std::ptr::null_mut(),
            lpszMenuName: std::ptr::null(),
            lpszClassName: class.as_ptr(),
        };
        RegisterClassW(&wc);
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            class.as_ptr(),
            wide("Bell").as_ptr(),
            WS_POPUP,
            0,
            0,
            10,
            10,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            hinst,
            std::ptr::null(),
        );
        if hwnd.is_null() {
            return Err("CreateWindowEx failed".into());
        }

        let prefs = Prefs::load();
        let mut pet = Pet {
            hwnd,
            prefs: prefs.clone(),
            face: Face::new(),
            text: Text::load(),
            t0: Instant::now(),
            scale: 1.0,
            dpi: 0,
            w: 0,
            h: 0,
            x: 0,
            y: 0,
            canvas: new_pixmap(1, 1)?,
            layer: new_pixmap(1, 1)?,
            fx: new_pixmap(1, 1)?,
            surface: Surface::new(1, 1).ok_or("DIB alloc failed")?,
            pressed: None,
            dragging: false,
            hover: 0.0,
            squash: 0.0,
            last_cursor: (0, 0),
            cursor_moved_at: -10_000.0,
            temp_mood: None,
            say_seq: 0,
            last_track: String::new(),
            last_lyric: String::new(),
            lyric_at: 0.0,
            claude: String::new(),
            claude_since: 0.0,
            bubble: None,
            bubble_a: 0.0,
            hidden_until: None,
            fs_hidden: false,
            alpha: 0.0,
            last_checks: 0.0,
            appear: 0.0,
        };
        if pet.text.is_none() {
            crate::log("overlay: no system font found, bubbles disabled");
        }
        pet.resize()?;
        match (prefs.x, prefs.y) {
            (Some(x), Some(y)) => {
                // Make sure it's still on a screen (monitors change).
                let wa = work_area_at(x + pet.w / 2, y + pet.h / 2);
                pet.x = x.clamp(wa.left - pet.w / 3, wa.right - pet.w * 2 / 3);
                pet.y = y.clamp(wa.top - pet.h / 3, wa.bottom - pet.h / 2);
            }
            _ => pet.place_default(),
        }
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        crate::log("overlay up");

        let mut last = Instant::now();
        let mut msg: MSG = std::mem::zeroed();
        loop {
            while PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                if msg.message == WM_QUIT {
                    return Ok(());
                }
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            let dt = last.elapsed().as_secs_f32() * 1000.0;
            if dt >= 14.0 {
                last = Instant::now();
                pet.frame(&feed, dt.min(100.0))?;
            }
            if pet.alpha < 0.005 && (pet.fs_hidden || pet.hidden_until.is_some()) {
                std::thread::sleep(Duration::from_millis(150));
            } else if DwmFlush() != 0 {
                std::thread::sleep(Duration::from_millis(8));
            }
        }
    }
}
