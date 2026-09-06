//! server.rs —— 本地 HTTP + SSE 服务（127.0.0.1 随机端口）。
//!
//! 与 dsh-pet-app 的 Electron 主进程职责一一对应（Rust 实现）：
//!   /config /meta、/thumb /font /pic（Range）、/pet/*（窗口/碰撞）、/events（SSE）。
//! 前端 window.petBridge（frontend/bridge.js）访问这些端点，动画引擎/物理与 Electron 版共用 JS。

use crate::assets;
use crate::config;
use crate::shared::{ServerEvent, Shared};
use crate::window;
use axum::body::Body;
use axum::extract::{Path as AxPath, Query, State};
use axum::http::{header, HeaderMap, Method, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::net::SocketAddr;
use tokio::net::TcpListener;

/// CORS 预检中间件：浏览器跨源 POST/PUT/DELETE 会先发 OPTIONS，直接应答（axum 对
/// 方法不匹配会回 405 且不带 CORS 头，导致前端 fetch 被浏览器拦截）。
async fn cors_preflight(req: Request<Body>, next: Next) -> Response {
    if req.method() == Method::OPTIONS {
        return (StatusCode::OK, cors(), "").into_response();
    }
    next.run(req).await
}

fn cors() -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*".parse().unwrap());
    h.insert(header::ACCESS_CONTROL_ALLOW_METHODS, "GET, POST, PUT, DELETE, OPTIONS".parse().unwrap());
    h.insert(header::ACCESS_CONTROL_ALLOW_HEADERS, "content-type, range".parse().unwrap());
    h.insert(header::ACCESS_CONTROL_EXPOSE_HEADERS, "content-range, content-length".parse().unwrap());
    h
}

fn jerr(status: StatusCode, msg: &str) -> Response {
    (status, cors(), Json(json!({ "error": msg }))).into_response()
}

fn jobj(v: Value) -> Response {
    (StatusCode::OK, cors(), Json(v)).into_response()
}

// ---------- 配置 ----------

async fn get_config(State(shared): State<Shared>) -> Response {
    match config::read_merged(&shared.0.asset_root, &shared.0.user_dir) {
        Ok(v) => jobj(v),
        Err(e) => jerr(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

async fn put_config(State(shared): State<Shared>, Json(payload): Json<Value>) -> Response {
    match config::save_user_config(&payload, &shared.0.asset_root, &shared.0.user_dir) {
        Ok(()) => {
            let _ = shared.0.ev.send(ServerEvent::ConfigChanged);
            schedule_rebuild(shared.clone());
            jobj(json!({ "ok": true }))
        }
        Err(e) => jerr(StatusCode::BAD_REQUEST, &e),
    }
}

async fn del_config(State(shared): State<Shared>) -> Response {
    config::reset_user_config(&shared.0.user_dir);
    let _ = shared.0.ev.send(ServerEvent::ConfigChanged);
    schedule_rebuild(shared.clone());
    jobj(json!({ "ok": true }))
}

async fn get_meta(State(shared): State<Shared>) -> Response {
    jobj(config::meta(&shared.0.asset_root, &shared.0.user_dir))
}

/// 配置变更后重建宠物窗（派发到主线程）。
fn schedule_rebuild(shared: Shared) {
    let app = shared.0.app.clone();
    let _ = app.run_on_main_thread(move || {
        let merged = config::read_merged(&shared.0.asset_root, &shared.0.user_dir).unwrap_or_else(|_| Value::Object(Default::default()));
        let _ = window::rebuild_pet_windows(&shared, &merged);
    });
}

// ---------- 素材 ----------

fn parse_range(range: &str, len: usize) -> Option<(usize, usize)> {
    let rest = range.strip_prefix("bytes=")?;
    let (a, b) = rest.split_once('-')?;
    let start: usize = if a.is_empty() { 0 } else { a.parse().ok()? };
    if start >= len {
        return None;
    }
    let end: usize = if b.is_empty() { len - 1 } else { b.parse::<usize>().ok()?.min(len - 1) };
    if end < start {
        return None;
    }
    Some((start, end))
}

async fn serve_file(shared: &Shared, host: &str, path_seg: &str, headers: HeaderMap) -> Response {
    let Some(file) = assets::resolve_asset(&shared.0.asset_root, &shared.0.user_dir, host, path_seg) else {
        return (StatusCode::NOT_FOUND, cors(), "pet-asset: not found".to_string()).into_response();
    };
    let mime = assets::mime_of(&file);
    let data = match tokio::fs::read(&file).await {
        Ok(d) => d,
        Err(e) => return jerr(StatusCode::INTERNAL_SERVER_ERROR, &format!("read {:?}: {e}", file)),
    };
    let len = data.len();
    let mut resp_headers = cors();
    resp_headers.insert(header::CONTENT_TYPE, mime.parse().unwrap());
    resp_headers.insert(header::ACCEPT_RANGES, "bytes".parse().unwrap());
    resp_headers.insert(header::CACHE_CONTROL, "public, max-age=3600".parse().unwrap());

    if let Some(range) = headers.get(header::RANGE).and_then(|v| v.to_str().ok()) {
        if let Some((start, end)) = parse_range(range, len) {
            let slice = data[start..=end].to_vec();
            resp_headers.insert(header::CONTENT_RANGE, format!("bytes {start}-{end}/{len}").parse().unwrap());
            resp_headers.insert(header::CONTENT_LENGTH, slice.len().to_string().parse().unwrap());
            return (StatusCode::PARTIAL_CONTENT, resp_headers, Body::from(slice)).into_response();
        }
    }
    resp_headers.insert(header::CONTENT_LENGTH, len.to_string().parse().unwrap());
    (StatusCode::OK, resp_headers, Body::from(data)).into_response()
}

async fn get_thumb(State(shared): State<Shared>, AxPath((root, file)): AxPath<(String, String)>, headers: HeaderMap) -> Response {
    serve_file(&shared, "thumb", &format!("{root}/{file}"), headers).await
}
async fn get_font(State(shared): State<Shared>, AxPath(file): AxPath<String>, headers: HeaderMap) -> Response {
    serve_file(&shared, "font", &file, headers).await
}
async fn get_pic(State(shared): State<Shared>, AxPath(file): AxPath<String>, headers: HeaderMap) -> Response {
    serve_file(&shared, "pic", &file, headers).await
}

// ---------- 窗口 / 碰撞 ----------

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BoundsBody {
    index: usize,
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    box_x: f64,
    box_y: f64,
    size: f64,
    bottom_pad: f64,
}

#[derive(Deserialize)]
struct InteractiveBody {
    index: usize,
    interactive: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct FlightBody {
    pet: String,
    x: f64,
    y: f64,
    size: f64,
    bottom_pad: f64,
    vx: f64,
    vy: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct CollideBody {
    target_id: String,
    vx: f64,
    vy: f64,
}

async fn post_bounds(State(shared): State<Shared>, Json(b): Json<BoundsBody>) -> Response {
    window::set_bounds_by_index(&shared, b.index, b.x, b.y, b.width, b.height, b.box_x, b.box_y, b.size, b.bottom_pad);
    jobj(json!({ "ok": true }))
}

async fn post_interactive(State(shared): State<Shared>, Json(b): Json<InteractiveBody>) -> Response {
    window::set_interactive_by_index(&shared, b.index, b.interactive);
    jobj(json!({ "ok": true }))
}

async fn post_flight(State(shared): State<Shared>, Json(b): Json<FlightBody>) -> Response {
    shared.0.flight.lock().unwrap().insert(
        b.pet.clone(),
        crate::shared::FlightState { id: b.pet.clone(), x: b.x, y: b.y, size: b.size, bottom_pad: b.bottom_pad, vx: b.vx, vy: b.vy },
    );
    let _ = shared.0.ev.send(ServerEvent::Flight(shared.snapshot()));
    jobj(json!({ "ok": true }))
}

async fn post_collide(State(shared): State<Shared>, Json(b): Json<CollideBody>) -> Response {
    let _ = shared.0.ev.send(ServerEvent::Hit { pet: b.target_id.clone(), vx: b.vx, vy: b.vy });
    jobj(json!({ "ok": true }))
}

async fn post_open_settings(State(shared): State<Shared>) -> Response {
    match window::open_settings_window(&shared) {
        Ok(()) => jobj(json!({ "ok": true })),
        Err(e) => jerr(StatusCode::INTERNAL_SERVER_ERROR, &e),
    }
}

#[derive(Deserialize)]
struct StatusBody {
    index: usize,
    data: Value,
}

async fn post_status(State(shared): State<Shared>, Json(b): Json<StatusBody>) -> Response {
    shared.0.statuses.lock().unwrap().insert(b.index, b.data);
    jobj(json!({ "ok": true }))
}

async fn get_debug_status(State(shared): State<Shared>) -> Response {
    let m = shared.0.statuses.lock().unwrap().clone();
    let mut out = serde_json::Map::new();
    for (k, v) in m {
        out.insert(k.to_string(), v);
    }
    jobj(Value::Object(out))
}

// ---------- SSE ----------

#[derive(Deserialize)]
struct EventsQuery {
    pet: Option<String>,
}

async fn sse_events(State(shared): State<Shared>, Query(q): Query<EventsQuery>) -> Response {
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Result<SseEvent, std::convert::Infallible>>();
    let mut broadcast_rx = shared.0.ev.subscribe();
    let pet_filter = q.pet.clone();
    tokio::spawn(async move {
        loop {
            let evt = match broadcast_rx.recv().await {
                Ok(v) => v,
                Err(_) => break, // 发送端关闭
            };
            let (name, data): (&str, String) = match evt {
                ServerEvent::Flight(v) => ("flight", serde_json::to_string(&v).unwrap_or_else(|_| "[]".into())),
                ServerEvent::Hit { pet, vx, vy } => {
                    if let Some(f) = &pet_filter {
                        if f != &pet {
                            continue;
                        }
                    }
                    ("hit", serde_json::to_string(&json!({ "vx": vx, "vy": vy })).unwrap_or_default())
                }
                ServerEvent::ConfigChanged => ("config", "{}".into()),
            };
            let event = SseEvent::default().event(name).data(data);
            if tx.send(Ok(event)).is_err() {
                break;
            }
        }
    });
    let stream = tokio_stream::wrappers::UnboundedReceiverStream::new(rx);
    let sse = Sse::new(stream).keep_alive(KeepAlive::default());
    let mut headers = cors();
    headers.insert(header::CONTENT_TYPE, "text/event-stream".parse().unwrap());
    (headers, sse).into_response()
}

// ---------- 路由 ----------

fn router() -> Router<Shared> {
    Router::new()
        .route("/config", get(get_config).put(put_config).delete(del_config))
        .route("/meta", get(get_meta))
        .route("/thumb/{root}/{file}", get(get_thumb))
        .route("/font/{file}", get(get_font))
        .route("/pic/{file}", get(get_pic))
        .route("/pet/bounds", post(post_bounds))
        .route("/pet/interactive", post(post_interactive))
        .route("/pet/flight", post(post_flight))
        .route("/pet/collide", post(post_collide))
        .route("/pet/open-settings", post(post_open_settings))
        .route("/pet/status", post(post_status))
        .route("/debug/status", get(get_debug_status))
        .route("/events", get(sse_events))
        .fallback(handle_fallback)
        .layer(middleware::from_fn(cors_preflight))
}

async fn handle_fallback(method: Method) -> Response {
    if method == Method::OPTIONS {
        return (StatusCode::OK, cors(), "").into_response();
    }
    (StatusCode::NOT_FOUND, cors(), "not found").into_response()
}

/// 启动本地服务（127.0.0.1 随机端口），把 base 写回 shared；返回 base url。
pub async fn start(shared: Shared) -> Result<String, String> {
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).await.map_err(|e| format!("bind 失败: {e}"))?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    let base = format!("http://127.0.0.1:{port}");
    shared.set_api_base(base.clone());
    let app = router().with_state(shared.clone());
    tauri::async_runtime::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });
    Ok(base)
}
