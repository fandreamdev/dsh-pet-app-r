//! shared.rs —— 跨模块共享状态（配置映射 / 窗口 / 飞行 / 碰撞 / 事件通道）。

use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::AppHandle;
use tokio::sync::broadcast;

/// 静止/拖拽宠物的包围盒登记（来自前端 set-bounds）
#[derive(Clone, Serialize)]
pub struct StaticBox {
    pub x: f64,
    pub y: f64,
    pub size: f64,
    pub bottom_pad: f64,
}

/// 飞行宠物实时状态（来自前端 report-flight）
#[derive(Clone, Serialize)]
pub struct FlightState {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub size: f64,
    pub bottom_pad: f64,
    pub vx: f64,
    pub vy: f64,
}

#[derive(Clone)]
pub enum ServerEvent {
    /// 全量飞行/静止状态快照（前端碰撞检测用）
    Flight(Vec<FlightState>),
    /// 你被撞了（定向给 pet id）
    Hit { pet: String, vx: f64, vy: f64 },
    /// 配置变更（前端刷新）
    ConfigChanged,
}

pub struct Inner {
    pub app: AppHandle,
    pub asset_root: PathBuf,
    pub user_dir: PathBuf,
    /// 本地 API base（http://127.0.0.1:PORT），服务启动后写入
    pub api_base: Mutex<String>,
    /// 真实宠物 id → 窗口序号（窗口 label = pet-<序号>）
    pub idx: Mutex<HashMap<String, usize>>,
    /// 序号 → 宠物 id（广播数据携带真实 id）
    pub id_of: Mutex<Vec<String>>,
    pub static_boxes: Mutex<HashMap<String, StaticBox>>, // key = 真实 pet id
    pub flight: Mutex<HashMap<String, FlightState>>,     // key = 真实 pet id
    /// 渲染端遥测（index -> 状态 JSON；验证 DOM/视频运行用）
    pub statuses: Mutex<HashMap<usize, serde_json::Value>>,
    pub ev: broadcast::Sender<ServerEvent>,
}

#[derive(Clone)]
pub struct Shared(pub Arc<Inner>);

impl Shared {
    pub fn new(app: AppHandle, asset_root: PathBuf, user_dir: PathBuf) -> Shared {
        let (ev, _) = broadcast::channel(64);
        Shared(Arc::new(Inner {
            app,
            asset_root,
            user_dir,
            api_base: Mutex::new(String::new()),
            idx: Mutex::new(HashMap::new()),
            id_of: Mutex::new(Vec::new()),
            static_boxes: Mutex::new(HashMap::new()),
            flight: Mutex::new(HashMap::new()),
            statuses: Mutex::new(HashMap::new()),
            ev,
        }))
    }

    pub fn reset_pets(&self, ids: &[String]) {
        let mut idx = self.0.idx.lock().unwrap();
        let mut id_of = self.0.id_of.lock().unwrap();
        idx.clear();
        id_of.clear();
        for (i, id) in ids.iter().enumerate() {
            idx.insert(id.clone(), i);
            id_of.push(id.clone());
        }
        self.0.static_boxes.lock().unwrap().clear();
        self.0.flight.lock().unwrap().clear();
        self.0.statuses.lock().unwrap().clear();
    }

    /// 序号 → 宠物 id
    pub fn id_by_index(&self, index: usize) -> Option<String> {
        self.0.id_of.lock().unwrap().get(index).cloned()
    }

    pub fn api_base(&self) -> String {
        self.0.api_base.lock().unwrap().clone()
    }

    pub fn set_api_base(&self, base: String) {
        *self.0.api_base.lock().unwrap() = base;
    }

    /// 全部（静止 + 飞行）快照，广播用
    pub fn snapshot(&self) -> Vec<FlightState> {
        let mut all: Vec<FlightState> = Vec::new();
        {
            let f = self.0.flight.lock().unwrap();
            for s in f.values() {
                all.push(s.clone());
            }
        }
        {
            let b = self.0.static_boxes.lock().unwrap();
            for (id, sb) in b.iter() {
                if !all.iter().any(|f| &f.id == id) {
                    all.push(FlightState {
                        id: id.clone(),
                        x: sb.x,
                        y: sb.y,
                        size: sb.size,
                        bottom_pad: sb.bottom_pad,
                        vx: 0.0,
                        vy: 0.0,
                    });
                }
            }
        }
        all
    }
}
