//! config.rs —— 配置单一来源（与 dsh-pet-app main/config-store.js 语义对齐，形状完全兼容）。
//!
//! 数据源：
//!   - 默认配置：assets/config.jsonc（JSONC，允许注释）
//!   - 用户覆盖：userData/config.json（给出即替换对应字段，未写回落默认）
//!
//! 合并产物形状与上游一致：{ main: {...}, [custom 品种 key]: {...} }，
//! 前端用 shared-core 的 flattenConfigPets 拍平（零改动复用）。

use serde_json::{json, Map, Value};
use std::path::{Path, PathBuf};

const CORNERS: [&str; 4] = ["top-left", "top-right", "bottom-left", "bottom-right"];
const DISPLAYS: [&str; 4] = ["web", "desktop", "both", "none"];

/// 剥离 JSONC 注释（// 与块注释）——注意别误伤字符串里的 //。
fn strip_jsonc(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0usize;
    let mut in_str = false;
    while i < bytes.len() {
        let c = bytes[i];
        if in_str {
            out.push(c);
            if c == b'\\' {
                if i + 1 < bytes.len() {
                    out.push(bytes[i + 1]);
                    i += 2;
                    continue;
                }
            } else if c == b'"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        match c {
            b'"' => {
                in_str = true;
                out.push(c);
                i += 1;
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'/' => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if i + 1 < bytes.len() && bytes[i + 1] == b'*' => {
                i += 2;
                while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                    i += 1;
                }
                i += 2;
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn parse_json(value: &Value, path: &str) -> Result<Value, String> {
    match value {
        Value::Object(_) => Ok(value.clone()),
        _ => Err(format!("{path}: 不是对象")),
    }
}

pub fn read_json_file(file: &Path) -> Result<Value, String> {
    let raw = std::fs::read_to_string(file).map_err(|e| format!("读取失败 {}: {e}", file.display()))?;
    let raw = raw.strip_prefix('\u{feff}').unwrap_or(&raw);
    let text = if file.extension().is_some_and(|e| e == "jsonc") {
        strip_jsonc(raw)
    } else {
        raw.to_string()
    };
    serde_json::from_str(&text).map_err(|e| format!("解析失败 {}: {e}", file.display()))
}

fn num(v: Option<&Value>, default: f64) -> f64 {
    match v.and_then(|x| x.as_f64()) {
        Some(x) if x.is_finite() => x,
        _ => default,
    }
}

fn bool_of(v: Option<&Value>, default: bool) -> bool {
    match v.and_then(|x| x.as_bool()) {
        Some(b) => b,
        None => default,
    }
}

fn str_of(v: Option<&Value>, default: &str) -> String {
    match v.and_then(|x| x.as_str()) {
        Some(s) => s.to_string(),
        None => default.to_string(),
    }
}

/// 归一化单个宠物实例；Err = 应剔除（id 类非法）。
fn normalize_pet(raw: &Value, index: usize) -> Result<Value, String> {
    let obj = match raw.as_object() {
        Some(o) => o,
        None => return Err(format!("pets[{index}] 不是对象")),
    };
    let id = str_of(obj.get("id"), "").trim().to_string();
    let id_forbidden = |c: char| c == '\\' || c == '/' || c == ':' || (c as u32) < 0x20;
    if id.is_empty() || id.chars().count() > 64 || id.chars().any(id_forbidden) {
        return Err(format!("pets[{index}] id 缺失/非法"));
    }
    let size = num(obj.get("size"), 462.0).max(1.0);
    let pos = obj.get("position").and_then(|p| p.as_object()).cloned().unwrap_or_default();
    let corner = match pos.get("corner").and_then(|c| c.as_str()) {
        Some(c) if CORNERS.contains(&c) => c.to_string(),
        _ => "top-right".to_string(),
    };
    let margin_x = num(pos.get("marginX"), 24.0);
    let margin_y = num(pos.get("marginY"), 100.0);
    let display = match obj.get("display").and_then(|d| d.as_str()) {
        Some(d) if DISPLAYS.contains(&d) => d.to_string(),
        _ => "both".to_string(),
    };
    let name = str_of(obj.get("name"), &id);
    let mut out = Map::new();
    out.insert("name".into(), Value::String(if name.trim().is_empty() { id.clone() } else { name.trim().to_string() }));
    out.insert("id".into(), Value::String(id));
    out.insert("size".into(), json!(size));
    out.insert("balanceEnabled".into(), json!(bool_of(obj.get("balanceEnabled"), false)));
    out.insert("whisperEnabled".into(), json!(bool_of(obj.get("whisperEnabled"), false)));
    out.insert("workStatusEnabled".into(), json!(bool_of(obj.get("workStatusEnabled"), false)));
    out.insert("display".into(), Value::String(display));
    out.insert(
        "position".into(),
        json!({ "corner": corner, "marginX": margin_x, "marginY": margin_y }),
    );
    Ok(Value::Object(out))
}

fn dedupe_pets(list: &[Value]) -> Vec<Value> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for p in list {
        let id = p.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
        if seen.contains(&id) {
            continue;
        }
        seen.insert(id);
        out.push(p.clone());
    }
    out
}

/// 校验/归一化条目级字段（physics / animations / animationWeights / eventsRefreshSec）。
/// 规则与 Node 版一致：缺失回落默认；结构非法回落默认。
fn normalize_physics(raw: &Value) -> Value {
    let o = raw.as_object().cloned().unwrap_or_default();
    json!({
        "gravity": num(o.get("gravity"), 1400.0),
        "restitution": num(o.get("restitution"), 0.78),
        "groundFriction": num(o.get("groundFriction"), 2.5),
        "ceilingBounce": bool_of(o.get("ceilingBounce"), true),
        "throwPower": num(o.get("throwPower"), 1.0),
        "petCollision": bool_of(o.get("petCollision"), false),
    })
}

/// 合并单个条目（main 或 custom 品种）。
fn merge_entry(def: &Value, override_value: Option<&Value>) -> Value {
    let mut merged = def.clone();
    if let Some(ov) = override_value {
        if let (Some(m), Some(o)) = (merged.as_object_mut(), ov.as_object()) {
            for (k, v) in o {
                if k != "pets" && k != "physics" && k != "animations" && k != "animationWeights" && k != "eventsRefreshSec" {
                    m.insert(k.clone(), v.clone());
                }
            }
        }
    }
    // 物理
    let def_phys = def.get("physics").cloned().unwrap_or_else(|| normalize_physics(&Value::Null));
    let phys = override_value.and_then(|o| o.get("physics")).map(normalize_physics).unwrap_or(def_phys);
    if let Some(m) = merged.as_object_mut() {
        m.insert("physics".into(), phys);
    }
    // pets
    let raw_pets: Vec<Value> = override_value
        .and_then(|o| o.get("pets"))
        .and_then(|p| p.as_array())
        .cloned()
        .unwrap_or_else(|| def.get("pets").and_then(|p| p.as_array()).cloned().unwrap_or_default());
    let mut pets: Vec<Value> = Vec::new();
    for (i, p) in raw_pets.iter().enumerate() {
        match normalize_pet(p, i) {
            Ok(p) => pets.push(p),
            Err(e) => eprintln!("[config] {e}，已剔除"),
        }
    }
    let pets = dedupe_pets(&pets);
    let pets = if pets.is_empty() {
        dedupe_pets(
            &def.get("pets")
                .and_then(|p| p.as_array())
                .cloned()
                .unwrap_or_default()
                .iter()
                .enumerate()
                .filter_map(|(i, p)| normalize_pet(p, i).ok())
                .collect::<Vec<_>>(),
        )
    } else {
        pets
    };
    if let Some(m) = merged.as_object_mut() {
        m.insert("pets".into(), Value::Array(pets));
    }
    merged
}

/// 顶层配置读取（解析失败即安装损坏）。
fn read_default(default_dir: &Path) -> Result<Value, String> {
    let file = default_dir.join("config.jsonc");
    let raw = read_json_file(&file).map_err(|e| format!("内置默认配置损坏（{file:?}）：{e}"))?;
    parse_json(&raw, &file.display().to_string())?;
    if raw.get("pets").and_then(|p| p.as_array()).is_none() {
        return Err("内置默认配置缺少 pets".into());
    }
    Ok(raw)
}

/// 读取合并配置。default_dir 提供 assets/config.jsonc；user_dir 为 userData。
pub fn read_merged(default_dir: &Path, user_dir: &Path) -> Result<Value, String> {
    let def = read_default(default_dir)?;
    let user_file = user_dir.join("config.json");
    let user: Value = if user_file.exists() {
        read_json_file(&user_file).unwrap_or_else(|e| {
            eprintln!("[config] 用户配置解析失败，按无用户配置处理：{e}");
            Value::Null
        })
    } else {
        Value::Null
    };
    let user_obj = if user.is_object() { Some(&user) } else { None };
    let main = merge_entry(&def, user_obj);

    // custom 品种：userData/custom/<名>-config.json + <名>-animation/webm
    let mut merged = Map::new();
    merged.insert("main".into(), main);
    let custom_dir = user_dir.join("custom");
    if custom_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&custom_dir) {
            let mut files: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.is_file()
                        && p.file_name()
                            .and_then(|n| n.to_str())
                            .is_some_and(regex_like_config_name)
                })
                .collect();
            files.sort();
            for f in files {
                let stem = f.file_name().unwrap().to_string_lossy().to_string();
                let prefix = stem
                    .strip_suffix("-config.json")
                    .or_else(|| stem.strip_suffix("-config.jsonc"))
                    .unwrap_or("")
                    .to_string();
                if prefix.is_empty() {
                    continue;
                }
                let anim_dir = custom_dir.join(format!("{prefix}-animation")).join("webm");
                if !anim_dir.is_dir() {
                    eprintln!("[config] custom 品种 {prefix} 缺少 {prefix}-animation/ 目录，已跳过");
                    continue;
                }
                match read_json_file(&f) {
                    Ok(raw) => {
                        merged.insert(prefix.clone(), merge_entry(&def, Some(&raw)));
                    }
                    Err(e) => eprintln!("[config] custom 品种配置解析失败（{prefix}）：{e}"),
                }
            }
        }
    }
    Ok(Value::Object(merged))
}

fn regex_like_config_name(name: &str) -> bool {
    // 简化：以 -config.json 或 -config.jsonc 结尾即可
    name.ends_with("-config.json") || name.ends_with("-config.jsonc")
}

/// 保存用户配置（设置页）：白名单重建 pets/physics 等，透传保留其它既有顶层手改字段。
pub fn save_user_config(payload: &Value, _default_dir: &Path, user_dir: &Path) -> Result<(), String> {
    let payload = payload.as_object().ok_or("payload 不是对象")?;
    let user_file = user_dir.join("config.json");
    let existing: Value = if user_file.exists() {
        read_json_file(&user_file).unwrap_or_else(|_| Value::Object(Map::new()))
    } else {
        Value::Object(Map::new())
    };
    let mut out = existing.as_object().cloned().unwrap_or_default();

    // pets
    let raw_pets = payload.get("pets").ok_or("缺少 pets")?.as_array().ok_or("pets 不是数组")?;
    let mut pets: Vec<Value> = Vec::new();
    for (i, p) in raw_pets.iter().enumerate() {
        match normalize_pet(p, i) {
            Ok(p) => pets.push(p),
            Err(e) => return Err(e.to_string()),
        }
    }
    let pets = dedupe_pets(&pets);
    if pets.is_empty() {
        return Err("至少保留一只宠物".into());
    }
    out.insert("pets".into(), Value::Array(pets));

    // physics（可选）
    if let Some(ph) = payload.get("physics") {
        out.insert("physics".into(), normalize_physics(ph));
    }
    // 其它顶层精调字段原样透传（保留已有手改）
    if let Some(n) = payload.get("notificationsEnabled") {
        out.insert("notificationsEnabled".into(), n.clone());
    }
    let _ = parse_json(&Value::Object(out.clone()), "config");

    std::fs::create_dir_all(user_dir).map_err(|e| e.to_string())?;
    let json_text = serde_json::to_string_pretty(&Value::Object(out)).map_err(|e| e.to_string())?;
    std::fs::write(&user_file, json_text).map_err(|e| e.to_string())?;
    Ok(())
}

pub fn reset_user_config(user_dir: &Path) {
    let user_file = user_dir.join("config.json");
    let _ = std::fs::remove_file(user_file);
}

/// 桌面可见宠物列表（{id,size}）。
pub fn desktop_pets(merged: &Value) -> Vec<(String, f64)> {
    let mut out = Vec::new();
    if let Some(obj) = merged.as_object() {
        for entry in obj.values() {
            if let Some(pets) = entry.get("pets").and_then(|p| p.as_array()) {
                for p in pets {
                    let display = p.get("display").and_then(|d| d.as_str()).unwrap_or("");
                    if display == "desktop" || display == "both" {
                        let id = p.get("id").and_then(|x| x.as_str()).unwrap_or("").to_string();
                        let size = p.get("size").and_then(|x| x.as_f64()).unwrap_or(462.0);
                        out.push((id, size));
                    }
                }
            }
        }
    }
    out
}

/// 序列化用的 meta 信息。
pub fn meta(default_dir: &Path, user_dir: &Path) -> Value {
    json!({
        "defaultPath": default_dir.join("config.jsonc").display().to_string(),
        "userPath": user_dir.join("config.json").display().to_string(),
        "customDir": user_dir.join("custom").display().to_string(),
        "assetRoot": default_dir.display().to_string(),
        "userData": user_dir.display().to_string(),
    })
}
