//! dsh-pet-rust —— Tauri 2 宿主：Rust 实现 dsh-pet-app v0.1.0 的核心宿主职责。
//!
//! 启动序列：发现素材根 → userData → 启动本地 API 服务(127.0.0.1 随机端口) →
//! 读取 merged 配置 → 每只桌面宠物一个透明置顶窗 → 托盘。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod assets;
mod config;
mod passthrough;
mod pet_windows;
mod server;
mod settings_window;
mod shared;

use shared::Shared;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, RunEvent};

static VISIBLE: AtomicBool = AtomicBool::new(true);
static QUITTING: AtomicBool = AtomicBool::new(false);

/// 给用户的可见提示：release 是 GUI 子系统（无控制台），找不到素材时用系统消息框。
fn warn_user(message: &str) {
    eprintln!("[dsh-pet-rust] {message}");
    #[cfg(windows)]
    unsafe {
        use windows_sys::Win32::UI::WindowsAndMessaging::MessageBoxW;
        let wide: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
        let cap: Vec<u16> = "dsh-pet-rust"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let _ = MessageBoxW(std::ptr::null_mut(), wide.as_ptr(), cap.as_ptr(), 0);
        // MB_OK
    }
}

fn main() {
    tauri::Builder::default()
        .setup(|app| {
            // 素材根：找不到时给用户可见提示（release 无控制台），并继续运行托盘等待修复
            let mut extra_roots = Vec::new();
            if let Ok(resource_dir) = app.path().resource_dir() {
                extra_roots.push(resource_dir.join("assets"));
            }
            let Some(asset_root) = assets::discover_asset_root(&extra_roots) else {
                warn_user(
                    "dsh-pet-rust 未找到素材目录 assets/\n\n请把本程序放到仓库根目录运行，\n或用环境变量 DSH_PET_ASSET_ROOT 指向包含 config.jsonc 与 webm/ 的目录。",
                );
                return Ok(());
            };
            let user_dir = app.path().app_data_dir().unwrap_or_else(|_| {
                std::env::current_dir()
                    .unwrap_or_default()
                    .join("user-data")
            });
            std::fs::create_dir_all(&user_dir).ok();

            // 共享状态 + 本地服务（先拿端口，窗口 query 需要）
            let shared = Shared::new(app.handle().clone(), asset_root.clone(), user_dir.clone());
            let base = tauri::async_runtime::block_on(server::start(shared.clone()))?;
            eprintln!(
                "[dsh-pet-rust] asset_root={} api={}",
                asset_root.display(),
                base
            );

            // 托盘
            let toggle_item = MenuItem::with_id(app, "toggle", "隐藏宠物", true, None::<&str>)?;
            let settings_item = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
            let menu = Menu::with_items(
                app,
                &[
                    &toggle_item,
                    &settings_item,
                    &PredefinedMenuItem::separator(app)?,
                    &quit_item,
                ],
            )?;
            let mut tray_builder = TrayIconBuilder::with_id("tray")
                .tooltip("dsh-pet-rust")
                .menu(&menu)
                .show_menu_on_left_click(false);
            if let Some(icon) = app.default_window_icon().cloned() {
                tray_builder = tray_builder.icon(icon);
            }
            let _tray = tray_builder.build(app)?;

            // 管理状态 + 初始宠物窗
            app.manage(shared.clone());
            #[cfg(windows)]
            passthrough::start_poller(app.handle().clone());
            let _ = settings_window::prepare(&shared);
            let merged = config::read_merged(&asset_root, &user_dir)?;
            let n = pet_windows::rebuild(&shared, &merged)?;
            pet_windows::show_all(&shared);
            eprintln!("[dsh-pet-rust] 桌面宠物 {n} 只");
            Ok(())
        })
        .on_menu_event(|app, event| match event.id().as_ref() {
            "toggle" => {
                let next = !VISIBLE.load(Ordering::Relaxed);
                VISIBLE.store(next, Ordering::Relaxed);
                let shared = app.state::<Shared>();
                pet_windows::set_visible(&shared, next);
                // 重建托盘菜单以切换文案（Windows 下 app.menu() 拿不到托盘菜单）
                if let Some(tray) = app.tray_by_id("tray") {
                    let new_toggle = MenuItem::with_id(
                        app,
                        "toggle",
                        if next { "隐藏宠物" } else { "显示宠物" },
                        true,
                        None::<&str>,
                    );
                    let new_settings = MenuItem::with_id(app, "settings", "设置", true, None::<&str>);
                    let new_quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>);
                    let sep = PredefinedMenuItem::separator(app);
                    let rebuilt = match (new_toggle, new_settings, sep, new_quit) {
                        (Ok(t), Ok(s), Ok(sep), Ok(q)) => {
                            Menu::with_items(app, &[&t, &s, &sep, &q]).ok()
                        }
                        _ => None,
                    };
                    if let Some(menu) = rebuilt {
                        let _ = tray.set_menu(Some(menu));
                    }
                }
            }
            "settings" => {
                let shared = app.state::<Shared>();
                if let Err(e) = settings_window::open(&shared) {
                    warn_user(&format!("打开设置失败：{e}"));
                }
            }
            "quit" => {
                QUITTING.store(true, Ordering::Relaxed);
                let shared = app.state::<Shared>();
                pet_windows::close_all(&shared);
                app.exit(0);
                // 兜底：若窗口关闭流程卡住，1s 后强制退出进程
                std::thread::spawn(|| {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                    std::process::exit(0);
                });
            }
            _ => {}
        })
        .build(tauri::generate_context!())
        .expect("error while building dsh-pet-rust")
        .run(|app, event| {
            // 宠物窗因配置重建会被 close：仅当“真的退出”时才允许进程退出（托盘驻留）
            if let RunEvent::ExitRequested { api, .. } = event {
                if !QUITTING.load(Ordering::Relaxed) {
                    api.prevent_exit();
                }
            }
            let _ = app;
        });
}
