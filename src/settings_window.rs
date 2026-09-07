use crate::shared::Shared;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

pub fn prepare(shared: &Shared) -> Result<(), String> {
    if shared.0.app.get_webview_window("settings").is_some() {
        return Ok(());
    }
    build(shared, false)
}

pub fn open(shared: &Shared) -> Result<(), String> {
    let app = &shared.0.app;
    if let Some(win) = app.get_webview_window("settings") {
        win.show().map_err(|e| e.to_string())?;
        let _ = win.unminimize();
        let _ = win.set_focus();
        return Ok(());
    }
    build(shared, true)
}

fn build(shared: &Shared, visible: bool) -> Result<(), String> {
    let app = &shared.0.app;
    let page = format!("settings.html?api={}", shared.api_base());
    eprintln!("[settings] creating native window visible={visible}");
    let win = WebviewWindowBuilder::new(app, "settings", WebviewUrl::App(page.into()))
        .title("dsh-pet-rust 设置")
        .inner_size(780.0, 640.0)
        .min_inner_size(520.0, 420.0)
        .decorations(true)
        .transparent(false)
        // 与宠物窗同为置顶：保证设置窗打开时位于宠物窗之上，X 不被透明宠物窗挡住
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(true)
        .visible(visible)
        .additional_browser_args(crate::pet_windows::browser_args())
        .on_page_load(|_, p| eprintln!("[settings] {:?} {}", p.event(), p.url()))
        .build()
        .map_err(|e| format!("创建设置窗口失败: {e}"))?;
    // X 走默认“销毁”，下次打开会重建（两窗 browser_args 已一致，重建没问题）。
    if visible {
        win.show().map_err(|e| format!("显示设置窗口失败: {e}"))?;
        win.set_focus()
            .map_err(|e| format!("聚焦设置窗口失败: {e}"))?;
    }
    Ok(())
}
