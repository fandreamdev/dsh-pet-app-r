//! assets.rs —— 素材根定位与安全解析（语义同 dsh-pet-app main/asset-server.js）。
//!
//! 本仓库完全自包含：assets/（webm/fonts/pic/config.jsonc）随仓库维护，运行时不再依赖
//! 任何外部目录。资源根候选（按序取第一个存在且结构完整的）：
//!   1. 环境变量 DSH_PET_ASSET_ROOT（显式指定）
//!   2. <cwd>/assets                        （开发：cargo run / 直接运行）
//!   3. <exe 目录>/assets                   （打包后随程序分发）
use std::path::{Path, PathBuf};

pub const ALLOWED_EXT: [&str; 3] = ["webm", "ttf", "png"];

fn looks_like_root(p: &Path) -> bool {
    p.join("config.jsonc").exists() && p.join("webm").is_dir()
}

pub fn discover_asset_root() -> Option<PathBuf> {
    if let Ok(env) = std::env::var("DSH_PET_ASSET_ROOT") {
        let p = PathBuf::from(env);
        if looks_like_root(&p) {
            return Some(p);
        }
    }
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if looks_like_root(&cwd.join("assets")) {
        return Some(cwd.join("assets"));
    }
    // 打包后：<exe>/assets
    if let Ok(exe) = std::env::current_exe() {
        if let Some(p) = exe.parent().map(|d| d.join("assets")) {
            if looks_like_root(&p) {
                return Some(p);
            }
        }
    }
    None
}

fn ext_ok(name: &str) -> bool {
    let Some((_, ext)) = name.rsplit_once('.') else {
        return false;
    };
    ALLOWED_EXT.contains(&ext.to_ascii_lowercase().as_str())
}

/// 在根目录内做安全解析（防穿越）。
fn safe_resolve(root: &Path, rel: &str) -> Option<PathBuf> {
    if rel.is_empty() || rel.contains('\0') || rel.contains("..") {
        return None;
    }
    let full = root.join(rel);
    let full = full.canonicalize().ok()?;
    let rootc = root.canonicalize().ok()?;
    if !full.starts_with(&rootc) {
        return None;
    }
    Some(full)
}

/// 解析素材 URL 段：host = thumb | font | pic。
/// thumb 的 path 形如 <root>/<file>：main 先查 userData/custom/main-animation/webm，再查包内 webm；
/// 其它 root 只查 userData/custom/<root>-animation/webm。
pub fn resolve_asset(
    pkg_root: &Path,
    user_dir: &Path,
    host: &str,
    path_seg: &str,
) -> Option<PathBuf> {
    let file_name = path_seg.rsplit('/').next().unwrap_or("");
    if !ext_ok(file_name) {
        return None;
    }
    let rel = path_seg.trim_start_matches('/');
    match host {
        "thumb" => {
            let (root, name) = rel.split_once('/')?;
            if root.is_empty() || name.is_empty() {
                return None;
            }
            let custom_dir = user_dir.join("custom");
            // 品种目录存在 → 只查自己的（隔离语义）
            let species_dir = custom_dir.join(format!("{root}-animation")).join("webm");
            if species_dir.is_dir() {
                return safe_resolve(&species_dir, name);
            }
            if root == "main" {
                let overlay = custom_dir.join("main-animation").join("webm");
                if overlay.is_dir() {
                    if let Some(p) = safe_resolve(&overlay, name) {
                        return Some(p);
                    }
                }
                return safe_resolve(&pkg_root.join("webm"), name);
            }
            None
        }
        "font" => safe_resolve(&pkg_root.join("fonts"), rel),
        "pic" => safe_resolve(&pkg_root.join("pic"), rel),
        _ => None,
    }
}

/// 推断 content-type。
pub fn mime_of(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "webm" => "video/webm",
        "ttf" => "font/ttf",
        "png" => "image/png",
        _ => "application/octet-stream",
    }
}
