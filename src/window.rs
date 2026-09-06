//! window.rs —— 窗口操作（创建 / 位置跟随 / 点击穿透 / 重建）。
//!
//! Tauri 的 webview 窗口操作要求在（或经派发到）主线程执行，这里统一经
//! app.run_on_main_thread 派发，从任意工作线程调用都安全。

use tauri::{Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

use crate::shared::{Shared, StaticBox};

pub const WIN_MARGIN_RATIO: f64 = 0.5; // 窗口四周外扩 = 宠物宽 × 比例
pub const SCREEN_H: f64 = 360.0;
pub const FEET_Y: f64 = 330.0;

/// WebView2 附加参数；DSH_PET_DEBUG=1 时开远程调试（本机验证/排障用）。
fn browser_args() -> &'static str {
    static ARGS: std::sync::OnceLock<String> = std::sync::OnceLock::new();
    ARGS.get_or_init(|| {
        let base = "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection";
        if std::env::var_os("DSH_PET_DEBUG").is_some() {
            format!("--remote-debugging-port=9229 {base}")
        } else {
            base.to_string()
        }
    }).as_str()
}

/// 由宠物尺寸推算窗口内容区尺寸。
pub fn window_size_for(size: f64) -> (f64, f64) {
    let h = size * 9.0 / 16.0;
    let bottom_pad = size * (9.0 / 16.0) * (SCREEN_H - FEET_Y) / SCREEN_H;
    let m = size * WIN_MARGIN_RATIO;
    (size + 2.0 * m, h + bottom_pad + 2.0 * m)
}

/// 创建单只宠物窗（index.html?petIndex&api&workArea…）。窗口 label = pet-<序号>。
pub fn create_pet_window(shared: &Shared, _pet_id: &str, size: f64, pet_index: usize, api_base: &str, work_area: (f64, f64)) -> Result<(), String> {
    let app = &shared.0.app;
    let (w, h) = window_size_for(size);
    let label = format!("pet-{pet_index}");
    let page = format!(
        "index.html?petIndex={}&api={}&workAreaW={}&workAreaH={}",
        pet_index, api_base, work_area.0 as u32, work_area.1 as u32
    );
    let lab = label.clone();
    let win = WebviewWindowBuilder::new(app, &label, WebviewUrl::App(page.into()))
        .title("dsh-pet-rust")
        .inner_size(w, h)
        .transparent(true)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .shadow(false)
        .resizable(false)
        .additional_browser_args(browser_args())
        .on_page_load(move |_win, payload| {
            eprintln!("[window] {lab} page_load {:?} url={}", payload.event(), payload.url());
        })
        .build()
        .map_err(|e| format!("建窗失败 {label}: {e}"))?;
    // 默认整窗穿透
    let _ = win.set_ignore_cursor_events(true);
    Ok(())
}

/// 位置跟随（按窗口序号）。box_* 为包围盒工作区坐标（碰撞站场登记用）。
pub fn set_bounds_by_index(shared: &Shared, index: usize, x: f64, y: f64, w: f64, h: f64, box_x: f64, box_y: f64, size: f64, bottom_pad: f64) {
    let pet_id = shared.id_by_index(index);
    if let Some(pet_id) = pet_id {
        shared.0.static_boxes.lock().unwrap().insert(
            pet_id.clone(),
            StaticBox { x: box_x, y: box_y, size, bottom_pad },
        );
        shared.0.flight.lock().unwrap().remove(&pet_id);
    }
    let app = shared.0.app.clone();
    let app2 = app.clone();
    let label = format!("pet-{index}");
    let _ = app.run_on_main_thread(move || {
        if let Some(win) = app2.get_webview_window(&label) {
            let _ = win.set_position(PhysicalPosition::new(x as i32, y as i32));
            let _ = win.set_size(PhysicalSize::new(w.max(1.0) as u32, h.max(1.0) as u32));
        }
    });
}

/// 点击穿透翻转（按窗口序号）。
pub fn set_interactive_by_index(shared: &Shared, index: usize, interactive: bool) {
    let app = shared.0.app.clone();
    let app2 = app.clone();
    let label = format!("pet-{index}");
    let _ = app.run_on_main_thread(move || {
        if let Some(win) = app2.get_webview_window(&label) {
            let _ = win.set_ignore_cursor_events(!interactive);
        }
    });
}

/// 打开设置窗（单例）。
pub fn open_settings_window(shared: &Shared) -> Result<(), String> {
    let app = &shared.0.app;
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.set_focus();
        return Ok(());
    }
    let api_base = shared.api_base();
    let page = format!("settings.html?api={api_base}");
    let _win = WebviewWindowBuilder::new(app, "settings", WebviewUrl::App(page.into()))
        .title("dsh-pet-rust 设置")
        .inner_size(780.0, 640.0)
        .resizable(true)
        .additional_browser_args(browser_args())
        .build()
        .map_err(|e| format!("打开设置窗失败: {e}"))?;
    Ok(())
}

/// 重建全部宠物窗（启动/配置保存后共用）。返回桌面宠物数量。
pub fn rebuild_pet_windows(shared: &Shared, merged: &serde_json::Value) -> Result<usize, String> {
    close_all(shared);
    let pets = crate::config::desktop_pets(merged);
    let ids: Vec<String> = pets.iter().map(|(id, _)| id.clone()).collect();
    shared.reset_pets(&ids);
    let app = &shared.0.app;
    let (mw, mh) = app
        .primary_monitor()
        .ok()
        .flatten()
        .map(|m| {
            let s = m.size();
            (s.width, s.height)
        })
        .unwrap_or((1920, 1080));
    let wa = (mw as f64, mh as f64);
    let api_base = shared.api_base();
    let n = pets.len();
    for (i, (id, size)) in pets.iter().enumerate() {
        create_pet_window(shared, id, *size, i, &api_base, wa)?;
    }
    Ok(n)
}

pub fn close_all(shared: &Shared) {
    let app = shared.0.app.clone();
    let count = shared.0.id_of.lock().unwrap().len();
    for i in 0..count {
        let label = format!("pet-{i}");
        if let Some(win) = app.get_webview_window(&label) {
            let _ = win.close();
        }
    }
    shared.reset_pets(&[]);
}

/// 显示/隐藏全部宠物窗。
pub fn set_visible_all(shared: &Shared, visible: bool) {
    let app = shared.0.app.clone();
    let count = shared.0.id_of.lock().unwrap().len();
    for i in 0..count {
        let label = format!("pet-{i}");
        let app2 = app.clone();
        let _ = app.run_on_main_thread(move || {
            if let Some(win) = app2.get_webview_window(&label) {
                if visible {
                    let _ = win.show();
                } else {
                    let _ = win.hide();
                }
            }
        });
    }
}
