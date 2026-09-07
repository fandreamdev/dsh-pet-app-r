//! passthrough.rs —— Windows 原生点击穿透（WS_EX_TRANSPARENT + 光标轮询，仅 Windows）。
//!
//! 与 dsh-pet-indesktop 同方案：宠物窗默认对鼠标「整窗穿透」（WS_EX_TRANSPARENT），
//! 光标进入宠物命中框时由轮询线程去掉该样式、让窗口可交互（点击/拖拽/右键菜单），
//! 光标离开命中框即恢复穿透；菜单/拖拽/弹窗需要整窗可交互时由前端 setInteractive(true)
//! 强制（对应 full 标记）。
//!
//! 相比旧的 WM_NCHITTEST→HTTRANSPARENT 区域穿透，WS_EX_TRANSPARENT 对「下层窗口的非客户区
//! （如设置窗标题栏 X）」的点击穿透更可靠，不会吞掉下层应用的点击。

#![cfg(windows)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use windows_sys::Win32::Foundation::{HWND, POINT};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetWindowLongPtrW, SetWindowLongPtrW, SetWindowPos, GWL_EXSTYLE, SWP_FRAMECHANGED,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, WS_EX_TRANSPARENT,
};

struct Entry {
    /// 原生窗口句柄（存数值，绕过 raw pointer 的 !Send/!Sync）。
    hwnd: usize,
    /// 前端强制整窗可交互（拖拽/菜单/弹窗）。
    full: AtomicBool,
    /// 命中框（屏幕物理坐标）。
    rect: Mutex<(f64, f64, f64, f64)>,
    /// 当前已应用的「可交互」状态（避免重复 SetWindowLongPtrW 造成闪烁）。
    interactive: AtomicBool,
}

static REGISTRY: OnceLock<Mutex<HashMap<String, Arc<Entry>>>> = OnceLock::new();

fn registry() -> &'static Mutex<HashMap<String, Arc<Entry>>> {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

/// 应用/移除整窗穿透（on=true 穿透，on=false 可交互）。幂等。
fn apply_transparent(hwnd: usize, on: bool) {
    unsafe {
        let ex = GetWindowLongPtrW(hwnd as HWND, GWL_EXSTYLE) as u32;
        let next = if on {
            ex | WS_EX_TRANSPARENT
        } else {
            ex & !WS_EX_TRANSPARENT
        };
        if next != ex {
            SetWindowLongPtrW(hwnd as HWND, GWL_EXSTYLE, next as isize);
            // 刷新框架让样式立即生效（不改位置/尺寸/z序/激活态）
            SetWindowPos(
                hwnd as HWND,
                std::ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED,
            );
        }
    }
}

/// 绑定窗口（创建后主线程调用一次）。初始为整窗穿透。
pub fn attach(label: &str, hwnd: *mut core::ffi::c_void) {
    let entry = Arc::new(Entry {
        hwnd: hwnd as usize,
        full: AtomicBool::new(false),
        rect: Mutex::new((0.0, 0.0, 0.0, 0.0)),
        interactive: AtomicBool::new(false),
    });
    apply_transparent(entry.hwnd, true); // 初始穿透
    registry().lock().unwrap().insert(label.to_string(), entry);
}

/// 解除绑定（窗口关闭前）。幂等。
pub fn detach(label: &str) {
    if let Some(entry) = registry().lock().unwrap().remove(label) {
        apply_transparent(entry.hwnd, false); // 还原，避免残留穿透样式
    }
}

/// 更新命中框（屏幕物理坐标）。
pub fn set_hit_rect(label: &str, rect: (f64, f64, f64, f64)) {
    let reg = registry().lock().unwrap();
    if let Some(e) = reg.get(label) {
        *e.rect.lock().unwrap() = rect;
    }
}

/// 强制整窗可交互（菜单/拖拽/弹窗 true；false 回到命中框判定）。
pub fn set_full(label: &str, full: bool) {
    let reg = registry().lock().unwrap();
    if let Some(e) = reg.get(label) {
        e.full.store(full, Ordering::Relaxed);
    }
}

/// 启动光标轮询线程（进程级一次）。每 ~15ms 采样光标：命中框内或强制可交互 → 窗口可交互；
/// 否则整窗穿透。
pub fn start_poller(app: tauri::AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(15));
        let mut cursor = POINT { x: 0, y: 0 };
        if unsafe { GetCursorPos(&mut cursor) } == 0 {
            continue;
        }
        let cx = cursor.x as f64;
        let cy = cursor.y as f64;
        let mut changes: Vec<(usize, bool)> = Vec::new();
        {
            let reg = registry().lock().unwrap();
            for e in reg.values() {
                let full = e.full.load(Ordering::Relaxed);
                let (rx, ry, rw, rh) = *e.rect.lock().unwrap();
                let inside = cx >= rx && cx <= rx + rw && cy >= ry && cy <= ry + rh;
                let desired = full || inside;
                if desired != e.interactive.load(Ordering::Relaxed) {
                    e.interactive.store(desired, Ordering::Relaxed);
                    changes.push((e.hwnd, desired));
                }
            }
        }
        if !changes.is_empty() {
            let _ = app.run_on_main_thread(move || {
                for (hwnd, interactive) in changes {
                    apply_transparent(hwnd, !interactive);
                }
            });
        }
    });
}
