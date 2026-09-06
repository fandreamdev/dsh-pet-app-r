//! passthrough.rs —— WM_NCHITTEST 原生区域级穿透（仅 Windows）。
//!
//! 透明置顶宠物窗里：**只有宠物命中框区域可交互（HTCLIENT）**，其余（透明外扩区）真实
//! 点击穿透（HTTRANSPARENT）；菜单/弹窗等需要整窗可交互时由 `set_full(label,true)` 切换
//! （对应前端 sprite 的 setInteractive(true)）。
//!
//! 实现：SetWindowLongPtrW(GWLP_WNDPROC) 子类化并拦截 WM_NCHITTEST。屏幕坐标→客户区坐标
//! 用 GetWindowRect/GetClientRect 换算；命中框几何与前端 sprite（HIT_BOX 200,50,440,335）
//! 一致，由 window.rs 每次 set_bounds 同步。

#![cfg(windows)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use windows_sys::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM, RECT};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CallWindowProcW, GetClientRect, GetWindowLongPtrW, GetWindowRect, SetWindowLongPtrW, GWLP_USERDATA,
    GWLP_WNDPROC, HTCLIENT, HTTRANSPARENT, WM_NCDESTROY, WM_NCHITTEST,
};

type WndProc = Option<unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT>;

const HTCLIENT_I: isize = HTCLIENT as isize;
const HTTRANSPARENT_I: isize = HTTRANSPARENT as isize;

struct Control {
    original_proc: isize,
    full: AtomicBool,
    rect: Mutex<(f64, f64, f64, f64)>,
}

/// label -> (hwnd, 控制块指针)
static REGISTRY: OnceLock<Mutex<HashMap<String, (usize, usize)>>> = OnceLock::new();

fn registry() -> &'static Mutex<HashMap<String, (usize, usize)>> {
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

unsafe fn control_of(hwnd: HWND) -> *mut Control {
    GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Control
}

fn remove_registry_by(ptr: usize, hwnd: usize) {
    let mut reg = registry().lock().unwrap();
    let mut key = None;
    for (k, (h, p)) in reg.iter() {
        if *p == ptr && *h == hwnd {
            key = Some(k.clone());
            break;
        }
    }
    if let Some(k) = key {
        reg.remove(&k);
    }
}

/// 屏幕坐标 → 窗口客户区坐标（frameless，直接用窗口矩形与客户区矩形换算）。
unsafe fn screen_to_client(hwnd: HWND, sx: i32, sy: i32) -> Option<(i32, i32)> {
    let mut wr = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    let mut cr = RECT { left: 0, top: 0, right: 0, bottom: 0 };
    if GetWindowRect(hwnd, &mut wr) == 0 || GetClientRect(hwnd, &mut cr) == 0 {
        return None;
    }
    let dx = wr.left - cr.left;
    let dy = wr.top - cr.top;
    Some((sx - dx, sy - dy))
}

unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let ctl = control_of(hwnd);
    let orig = if ctl.is_null() { 0 } else { (*ctl).original_proc };
    match msg {
        WM_NCHITTEST if !ctl.is_null() => {
            let c = &*ctl;
            if c.full.load(Ordering::Relaxed) {
                return HTCLIENT_I;
            }
            let sx = (lparam & 0xffff) as i16 as i32;
            let sy = ((lparam >> 16) & 0xffff) as i16 as i32;
            if let Some((cx, cy)) = screen_to_client(hwnd, sx, sy) {
                let (rx, ry, rw, rh) = *c.rect.lock().unwrap();
                let inside = (cx as f64) >= rx
                    && (cx as f64) <= rx + rw
                    && (cy as f64) >= ry
                    && (cy as f64) <= ry + rh;
                return if inside { HTCLIENT_I } else { HTTRANSPARENT_I };
            }
            return HTCLIENT_I;
        }
        WM_NCDESTROY if !ctl.is_null() => {
            let orig_saved = (*ctl).original_proc;
            let ptr = ctl as usize;
            let hwnd_usize = hwnd as usize;
            if orig_saved != 0 {
                SetWindowLongPtrW(hwnd, GWLP_WNDPROC, orig_saved);
            }
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            remove_registry_by(ptr, hwnd_usize);
            let _ = Box::from_raw(ctl);
            return CallWindowProcW(std::mem::transmute::<isize, WndProc>(orig_saved), hwnd, msg, wparam, lparam);
        }
        _ => {}
    }
    CallWindowProcW(std::mem::transmute::<isize, WndProc>(orig), hwnd, msg, wparam, lparam)
}

/// 绑定窗口（创建后、主线程上调用一次）。hwnd 为原生窗口句柄（raw pointer）。
pub fn attach(label: &str, hwnd: *mut core::ffi::c_void) {
    if registry().lock().unwrap().contains_key(label) {
        return;
    }
    let hwnd_typed = hwnd as HWND; // windows-sys HWND 即 *mut c_void
    let boxed = Box::new(Control { original_proc: 0, full: AtomicBool::new(false), rect: Mutex::new((0.0, 0.0, 0.0, 0.0)) });
    let raw = Box::into_raw(boxed);
    let original = unsafe { SetWindowLongPtrW(hwnd_typed, GWLP_WNDPROC, wnd_proc as *const () as usize as isize) };
    unsafe {
        (*raw).original_proc = original;
        SetWindowLongPtrW(hwnd_typed, GWLP_USERDATA, raw as isize);
    }
    registry().lock().unwrap().insert(label.to_string(), (hwnd as usize, raw as usize));
}

/// 更新命中框（窗口客户区坐标）。
pub fn set_hit_rect(label: &str, rect: (f64, f64, f64, f64)) {
    let reg = registry().lock().unwrap();
    if let Some((_, ptr)) = reg.get(label).copied() {
        let ctl = ptr as *mut Control;
        unsafe { *(*ctl).rect.lock().unwrap() = rect };
    }
}

/// 切换整窗可交互（菜单/弹窗 true；false 回到区域命中）。
pub fn set_full(label: &str, full: bool) {
    let reg = registry().lock().unwrap();
    if let Some((_, ptr)) = reg.get(label).copied() {
        let ctl = ptr as *mut Control;
        unsafe { (*ctl).full.store(full, Ordering::Relaxed) };
    }
}

/// 解除绑定（窗口关闭前）：恢复原 proc、释放控制块。幂等。
pub fn detach(label: &str) {
    let mut reg = registry().lock().unwrap();
    if let Some((hwnd, ptr)) = reg.remove(label) {
        let hwnd = hwnd as HWND;
        let ctl = ptr as *mut Control;
        let orig = unsafe { (*ctl).original_proc };
        unsafe {
            if orig != 0 {
                SetWindowLongPtrW(hwnd, GWLP_WNDPROC, orig);
            }
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            let _ = Box::from_raw(ctl);
        }
    }
}
