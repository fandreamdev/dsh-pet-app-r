//! window.rs —— 窗口操作（创建 / 位置跟随 / 点击穿透 / 重建）。
//!
//! Tauri 的 webview 窗口操作要求在（或经派发到）主线程执行，这里统一经
//! app.run_on_main_thread 派发，从任意工作线程调用都安全。

use tauri::{Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

#[cfg(windows)]
use crate::passthrough;
use crate::shared::{Shared, StaticBox};

pub const WIN_MARGIN_RATIO: f64 = 0.22; // 窗口四周外扩 = 宠物宽 × 比例（与 frontend/constants.js 保持一致）
pub const SCREEN_H: f64 = 360.0;
pub const FEET_Y: f64 = 330.0;
// 与前端 sprite HIT_BOX 一致：命中框 = 人物身体（640×360 画布坐标）
const HIT_X0: f64 = 200.0;
const HIT_Y0: f64 = 50.0;
const HIT_X1: f64 = 440.0;
const HIT_Y1: f64 = 335.0;

/// 命中框（窗口客户区坐标）—— 几何与前端 sprite.js 完全一致。
pub fn pet_hit_rect(size: f64, bottom_pad: f64) -> (f64, f64, f64, f64) {
    let m = size * WIN_MARGIN_RATIO;
    let height = size * 9.0 / 16.0;
    let x = m + (HIT_X0 / 640.0) * size;
    let y = m + bottom_pad + (HIT_Y0 / 360.0) * height;
    let w = ((HIT_X1 - HIT_X0) / 640.0) * size;
    let h = ((HIT_Y1 - HIT_Y0) / 360.0) * height;
    (x, y, w, h)
}

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
    })
    .as_str()
}

/// 由宠物尺寸推算窗口内容区尺寸。
pub fn window_size_for(size: f64) -> (f64, f64) {
    let h = size * 9.0 / 16.0;
    let bottom_pad = size * (9.0 / 16.0) * (SCREEN_H - FEET_Y) / SCREEN_H;
    let m = size * WIN_MARGIN_RATIO;
    (size + 2.0 * m, h + bottom_pad + 2.0 * m)
}

/// 创建单只宠物窗（index.html?petIndex&api&workArea…）。窗口 label = pet-<序号>。
pub fn create_pet_window(
    shared: &Shared,
    _pet_id: &str,
    size: f64,
    pet_index: usize,
    api_base: &str,
    work_area: (f64, f64),
) -> Result<(), String> {
    let app = &shared.0.app;
    let (w, h) = window_size_for(size);
    let label = format!("pet-{pet_index}");
    let page = format!(
        "index.html?petIndex={}&api={}&workAreaW={}&workAreaH={}",
        pet_index, api_base, work_area.0 as u32, work_area.1 as u32
    );
    let lab = label.clone();
    let _win = WebviewWindowBuilder::new(app, &label, WebviewUrl::App(page.into()))
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
            eprintln!(
                "[window] {lab} page_load {:?} url={}",
                payload.event(),
                payload.url()
            );
        })
        .build()
        .map_err(|e| format!("建窗失败 {label}: {e}"))?;
    // 原生区域级穿透（Windows）：窗口只在宠物命中框内可点，其余真实点击穿透。
    // 见 src/passthrough.rs；命中框随后由 set_bounds_by_index 持续更新。
    #[cfg(windows)]
    {
        if let Ok(hwnd) = _win.hwnd() {
            // tauri 的 HWND 是 windows crate 的 tuple 类型，转成裸指针供 passthrough 使用
            let ptr: *mut core::ffi::c_void = unsafe { std::mem::transmute(hwnd) };
            passthrough::attach(&label, ptr);
        }
    }
    Ok(())
}

/// 位置跟随（按窗口序号）。box_* 为包围盒工作区坐标（碰撞站场登记用）。
// 10 个参数与上游 Electron 版 setBounds 一一对应；后续可收敛为结构体，暂显式放行该 lint。
#[allow(clippy::too_many_arguments)]
pub fn set_bounds_by_index(
    shared: &Shared,
    index: usize,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    box_x: f64,
    box_y: f64,
    size: f64,
    bottom_pad: f64,
) {
    let pet_id = shared.id_by_index(index);
    if let Some(pet_id) = pet_id {
        shared.0.static_boxes.lock().unwrap().insert(
            pet_id.clone(),
            StaticBox {
                x: box_x,
                y: box_y,
                size,
                bottom_pad,
            },
        );
        shared.0.flight.lock().unwrap().remove(&pet_id);
    }
    let app = shared.0.app.clone();
    let app2 = app.clone();
    let label = format!("pet-{index}");
    // 命中框（客户区坐标，与前端几何一致）——窗口移动时同步给原生穿透层
    let rect = pet_hit_rect(size, bottom_pad);
    let _ = app.run_on_main_thread(move || {
        if let Some(win) = app2.get_webview_window(&label) {
            let _ = win.set_position(PhysicalPosition::new(x as i32, y as i32));
            let _ = win.set_size(PhysicalSize::new(w.max(1.0) as u32, h.max(1.0) as u32));
        }
        #[cfg(windows)]
        passthrough::set_hit_rect(&label, rect);
    });
}

/// 点击穿透翻转（按窗口序号）。
///
/// WM_NCHITTEST 模型下：interactive=true（菜单/弹窗打开、拖拽中前端要求）→ 整窗 HTCLIENT；
/// false → 回到“只在命中框内可点”的区域命中。
pub fn set_interactive_by_index(shared: &Shared, index: usize, interactive: bool) {
    let label = format!("pet-{index}");
    #[cfg(windows)]
    passthrough::set_full(&label, interactive);
    let _ = shared;
    let _ = index;
}

/// 打开设置窗（单例；已存在则显示并聚焦）。
pub fn open_settings_window(shared: &Shared) -> Result<(), String> {
    let app = &shared.0.app;
    if let Some(win) = app.get_webview_window("settings") {
        let _ = win.show();
        let _ = win.set_focus();
        return Ok(());
    }
    build_settings_window(shared, true)
}

/// 预建（隐藏）设置窗：在首个宠物窗之前创建，规避 WebView2 多窗口初始化问题。
pub fn prebuild_settings_window(shared: &Shared) -> Result<(), String> {
    build_settings_window(shared, false)
}

fn build_settings_window(shared: &Shared, visible: bool) -> Result<(), String> {
    let app = &shared.0.app;
    let api_base = shared.api_base();
    let page = format!("settings.html?api={api_base}");
    let label = "settings".to_string();
    let _win = WebviewWindowBuilder::new(app, &label, WebviewUrl::App(page.into()))
        .title("dsh-pet-rust 设置")
        .inner_size(780.0, 640.0)
        .resizable(true)
        .visible(visible)
        .additional_browser_args(browser_args())
        .on_page_load(move |_win, payload| {
            eprintln!(
                "[settings] page_load {:?} url={}",
                payload.event(),
                payload.url()
            );
        })
        .build()
        .map_err(|e| format!("打开设置窗失败: {e}"))?;
    eprintln!("[settings] built label={label} visible={visible}");
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
        #[cfg(windows)]
        passthrough::detach(&label);
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
