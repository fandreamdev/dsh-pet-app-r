use crate::config;
#[cfg(windows)]
use crate::passthrough;
use crate::shared::{Shared, StaticBox};
use tauri::{Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

const MARGIN: f64 = 0.22;
const SCREEN_H: f64 = 360.0;
const FEET_Y: f64 = 330.0;

pub(crate) fn browser_args() -> &'static str {
    static ARGS: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    ARGS.get_or_init(|| {
        let base = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection";
        if std::env::var_os("DSH_PET_DEBUG").is_some() {
            format!("--remote-debugging-port=9229 {base}")
        } else {
            base.to_string()
        }
    })
    .as_str()
}

fn window_size(size: f64) -> (f64, f64) {
    let h = size * 9.0 / 16.0;
    let bottom = size * (9.0 / 16.0) * (SCREEN_H - FEET_Y) / SCREEN_H;
    let m = size * MARGIN;
    (size + 2.0 * m, h + bottom + 2.0 * m)
}

fn hit_rect(size: f64, bottom: f64) -> (f64, f64, f64, f64) {
    let m = size * MARGIN;
    let h = size * 9.0 / 16.0;
    (
        m + size * 200.0 / 640.0,
        m + bottom + h * 50.0 / 360.0,
        size * 240.0 / 640.0,
        h * 285.0 / 360.0,
    )
}

pub fn create(
    shared: &Shared,
    size: f64,
    index: usize,
    api: &str,
    work: (f64, f64),
) -> Result<(), String> {
    let (w, h) = window_size(size);
    let label = format!("pet-{index}");
    let page = format!(
        "index.html?petIndex={index}&api={api}&workAreaW={}&workAreaH={}",
        work.0 as u32, work.1 as u32
    );
    let lab = label.clone();
    let win = WebviewWindowBuilder::new(&shared.0.app, &label, WebviewUrl::App(page.into()))
        .title("dsh-pet-rust")
        .inner_size(w, h)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(false)
        .resizable(false)
        .visible(true)
        .additional_browser_args(browser_args())
        .on_page_load(move |_, p| eprintln!("[pet:{lab}] {:?} {}", p.event(), p.url()))
        .build()
        .map_err(|e| format!("创建宠物窗口失败: {e}"))?;
    #[cfg(windows)]
    if let Ok(hwnd) = win.hwnd() {
        let ptr: *mut core::ffi::c_void = unsafe { std::mem::transmute(hwnd) };
        passthrough::attach(&label, ptr);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn set_bounds(
    shared: &Shared,
    index: usize,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    box_x: f64,
    box_y: f64,
    size: f64,
    bottom: f64,
) {
    if let Some(id) = shared.id_by_index(index) {
        shared.0.static_boxes.lock().unwrap().insert(
            id.clone(),
            StaticBox {
                x: box_x,
                y: box_y,
                size,
                bottom_pad: bottom,
            },
        );
        shared.0.flight.lock().unwrap().remove(&id);
    }
    let app = shared.0.app.clone();
    let label = format!("pet-{index}");
    let rect = hit_rect(size, bottom);
    let _ = app.clone().run_on_main_thread(move || {
        if let Some(win) = app.get_webview_window(&label) {
            let _ = win.set_position(PhysicalPosition::new(x as i32, y as i32));
            let _ = win.set_size(PhysicalSize::new(w.max(1.0) as u32, h.max(1.0) as u32));
        }
        #[cfg(windows)]
        passthrough::set_hit_rect(&label, rect);
    });
}

pub fn set_interactive(shared: &Shared, index: usize, value: bool) {
    #[cfg(windows)]
    passthrough::set_full(&format!("pet-{index}"), value);
    let _ = shared;
}

pub fn rebuild(shared: &Shared, merged: &serde_json::Value) -> Result<usize, String> {
    close_all(shared);
    let pets = config::desktop_pets(merged);
    shared.reset_pets(&pets.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>());
    let (mw, mh) = shared
        .0
        .app
        .primary_monitor()
        .ok()
        .flatten()
        .map(|m| {
            let s = m.size();
            (s.width as f64, s.height as f64)
        })
        .unwrap_or((1920.0, 1080.0));
    let api = shared.api_base();
    for (i, (_, size)) in pets.iter().enumerate() {
        create(shared, *size, i, &api, (mw, mh))?;
    }
    Ok(pets.len())
}

pub fn show_all(shared: &Shared) {
    let app = shared.0.app.clone();
    let count = shared.0.id_of.lock().unwrap().len();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        let _ = app.clone().run_on_main_thread(move || {
            for i in 0..count {
                if let Some(w) = app.get_webview_window(&format!("pet-{i}")) {
                    let _ = w.show();
                    let _ = w.set_focus();
                }
            }
        });
    });
}

pub fn close_all(shared: &Shared) {
    let app = shared.0.app.clone();
    let count = shared.0.id_of.lock().unwrap().len();
    for i in 0..count {
        let label = format!("pet-{i}");
        #[cfg(windows)]
        passthrough::detach(&label);
        if let Some(win) = app.get_webview_window(&label) {
            let _ = win.close();
        }
    }
    shared.reset_pets(&[]);
}

pub fn close_one(shared: &Shared, index: usize) {
    let label = format!("pet-{index}");
    #[cfg(windows)]
    passthrough::detach(&label);
    if let Some(win) = shared.0.app.get_webview_window(&label) {
        let _ = win.close();
    }
}

pub fn set_visible(shared: &Shared, visible: bool) {
    let app = shared.0.app.clone();
    let count = shared.0.id_of.lock().unwrap().len();
    for i in 0..count {
        let label = format!("pet-{i}");
        let app2 = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Some(w) = app2.get_webview_window(&label) {
                if visible {
                    let _ = w.show();
                } else {
                    let _ = w.hide();
                }
            }
        });
    }
}
