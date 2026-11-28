//! Native Bell app window: frameless WebView2 window around the local app
//! (http://localhost:7777). Started as `bell.exe --app` by the running companion.

use tao::dpi::LogicalSize;
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder};
use tao::platform::windows::WindowBuilderExtWindows;
use tao::window::{Icon, WindowBuilder};
use wry::WebViewBuilder;

const TITLE: &str = "Bell";
const URL: &str = "http://localhost:7777/?native=1";

enum Ui {
    Drag,
    Min,
    Max,
    Close,
}

/// Brings an already open app window forward. Returns true if one existed.
fn focus_existing() -> bool {
    use windows_sys::Win32::UI::WindowsAndMessaging::{FindWindowW, SetForegroundWindow, ShowWindow, SW_RESTORE};
    let title: Vec<u16> = "Bell".encode_utf16().chain(Some(0)).collect();
    let class: Vec<u16> = "BellAppWindow".encode_utf16().chain(Some(0)).collect();
    unsafe {
        let h = FindWindowW(class.as_ptr(), title.as_ptr());
        if h.is_null() {
            return false;
        }
        ShowWindow(h, SW_RESTORE);
        SetForegroundWindow(h);
    }
    true
}

/// Black rounded tile with two white eyes, same look as the notch.
fn icon() -> Option<Icon> {
    const N: u32 = 64;
    let mut px = vec![0u8; (N * N * 4) as usize];
    let r = 14.0f32;
    let eyes = [(18.0f32, 22.0f32), (38.0, 22.0)];
    for y in 0..N {
        for x in 0..N {
            let (fx, fy) = (x as f32 + 0.5, y as f32 + 0.5);
            // Rounded-square mask.
            let qx = (fx - N as f32 / 2.0).abs() - (N as f32 / 2.0 - r);
            let qy = (fy - N as f32 / 2.0).abs() - (N as f32 / 2.0 - r);
            let d = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt();
            if d > r {
                continue;
            }
            let eye = eyes.iter().any(|&(ex, ey)| fx >= ex && fx < ex + 8.0 && fy >= ey && fy < ey + 20.0);
            let c = if eye { 248 } else { 8 };
            let i = ((y * N + x) * 4) as usize;
            px[i..i + 4].copy_from_slice(&[c, c, c, 255]);
        }
    }
    Icon::from_rgba(px, N, N).ok()
}

pub fn run() {
    if focus_existing() {
        return;
    }
    let event_loop = EventLoopBuilder::<Ui>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let window = match WindowBuilder::new()
        .with_title(TITLE)
        .with_window_classname("BellAppWindow")
        .with_decorations(false)
        .with_undecorated_shadow(true)
        .with_inner_size(LogicalSize::new(1240.0, 820.0))
        .with_min_inner_size(LogicalSize::new(760.0, 520.0))
        .with_window_icon(icon())
        .build(&event_loop)
    {
        Ok(w) => w,
        Err(e) => {
            crate::log(&format!("app window failed: {e}"));
            return;
        }
    };

    let webview = WebViewBuilder::new()
        .with_url(URL)
        .with_background_color((0, 0, 0, 255))
        .with_devtools(cfg!(debug_assertions))
        .with_new_window_req_handler(|url, _| {
            if url.starts_with("http://") || url.starts_with("https://") {
                let _ = std::process::Command::new("explorer").arg(&url).spawn();
            }
            wry::NewWindowResponse::Deny
        })
        .with_ipc_handler(move |req| {
            let ev = match req.body().as_str() {
                "drag" => Ui::Drag,
                "min" => Ui::Min,
                "max" => Ui::Max,
                "close" => Ui::Close,
                _ => return,
            };
            let _ = proxy.send_event(ev);
        })
        .build(&window);
    let _webview = match webview {
        Ok(w) => w,
        Err(e) => {
            crate::log(&format!("webview failed (WebView2 missing?): {e}"));
            return;
        }
    };

    event_loop.run(move |event, _, flow| {
        *flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(Ui::Drag) => {
                let _ = window.drag_window();
            }
            Event::UserEvent(Ui::Min) => window.set_minimized(true),
            Event::UserEvent(Ui::Max) => window.set_maximized(!window.is_maximized()),
            Event::UserEvent(Ui::Close) | Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                *flow = ControlFlow::Exit
            }
            _ => {}
        }
    });
}
