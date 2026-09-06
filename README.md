# dsh-pet-rust

dsh-pet-app 的 **Rust(Tauri 2) 重构版** —— **独立、自包含的 git 仓库**（不再依赖任何外部文件夹）。

- 路线：Rust 负责全部**宿主层**（透明置顶窗口、点击穿透、托盘、配置合并、素材 HTTP 服务、跨窗碰撞 broker、设置 API）；渲染复用前端（`sprite.js` + `shared-core.js` + VP9-alpha webm），行为与 dsh-pet-app（Electron）共用同一份 JS 引擎。
- **自包含**：`assets/`（106 个 webm + fonts + pic + config.jsonc）与 `frontend/`（含构建产物 `shared-core.js`）都随仓库维护；运行时只需本仓库，素材根按 `DSH_PET_ASSET_ROOT` → `<cwd>/assets` → `<exe>/assets` 自动发现。
- 素材/纯逻辑来源（仅作追溯，非运行时依赖）：见 [SOURCES.md](./SOURCES.md)；如需从上游刷新，用 `node scripts/sync-from-siblings.mjs`（开发期工具，可离线）。
- 数据通道：Rust 内起本地 HTTP+SSE（127.0.0.1 随机端口），前端 `bridge.js` 把 `window.petBridge` 从 Electron IPC 换成 HTTP fetch / EventSource（sprite.js 原样可跑）。

## 运行

```sh
cargo run        # 依赖已配 crates 镜像；首次编译较慢
```

## 布局

```
src/main.rs      装配：素材根/userData → 起本地服务 → 托盘 → 宠物窗
src/config.rs    默认 config.jsonc + 用户覆盖 → merged（与上游形状一致）
src/assets.rs    素材根发现 + 安全解析
src/server.rs    axum：/config /meta /thumb|font|pic /pet/* /events(SSE)
src/window.rs    每宠一透明置顶窗（pet-<i>），位置跟随/穿透
src/shared.rs    共享状态（宠物映射/静止框/飞行/事件广播）
frontend/        index.html+constants.js+bridge.js+sprite.js+renderer.js(+shared-core.js 构建产物)
                 settings.html/css/js（走 API）
```

## 验证状态（本机实测证据）

- [x] 依赖链路/链接器、tauri-build（icons/icon.ico）通过
- [x] Rust 模块（config/assets/server/window/shared/main）编译通过
- [x] **透明窗口 + VP9-alpha spike 通过**：WebView2 中 sprite 运行、双缓冲播放、动画链切换、截图帧差异
- [x] 本地 API：/config、/meta、/thumb Range(206)、/font、/pic、/events(SSE)、/debug/status
- [x] **多开**：临时双宠物配置 → 两窗同屏各自 sprite 播放（pet-0 main / pet-1 second，videoTime 各自推进）
- [x] **点击回应**：合成 pointerdown/up + click → 切到「点击回应-开心跃动」(once)
- [x] **拖拽/释放**：pointer 位移 >5px → `dragState.dragging=true`、动画切「被鼠标拖拽悬空反馈」；释放 → 回待机链、`customPos` 记录
- [x] **右键菜单**：contextmenu → `.dsh-pet-menu-*` 列挂载、`menuOpen=true`，Esc/close 关闭
- [x] **设置窗往返**：打开 → 列表(2 chips) → 改 size=400 保存 → user config 落盘(400+physics) → Rust 重建宠物窗(page_load 日志 + telemetry 恢复) → 恢复默认回落单宠 462/924px
- [x] **本地自定义素材**：userData/custom/main-animation/webm 覆盖生效（仅用户目录存在的文件可 200）
- [x] 冒烟收尾：默认单宠启动无错（errors:[]，statuses 单条且重建清空）

## 已知限制 / 待办

- 打包（`tauri build` NSIS/portable）与发布文档 —— 下阶段
- 跨窗碰撞真实触发未实测（默认 petCollision:false；碰撞 broker 逻辑已就位）
- **交互模型 v1**：窗口始终可交互（余量已收窄 0.22×size，实测 665×484）；透明外扩区会挡住少量下层点击。
  后续改进 = WM_NCHITTEST 区域级穿透（只在宠物命中框内可点、其余穿透），见 `src/window.rs set_interactive_by_index` 注释。
- CDP 目标枚举在窗口重建后会失效（页面本身正常，见 page_load 日志）；遥测 `/debug/status` 是可靠观测通道
- 多显示器仍按主屏工作区；位置记忆/开机自启未做

## 调试与排障

- 开 WebView2 远程调试：`DSH_PET_DEBUG=1 cargo run`（端口 9229），
  `scripts/cdp-diag.mjs / cdp-interact*.mjs / cdp-shot.mjs / cdp-settings3.mjs` 为 CDP 验证脚本。
- 渲染端每 2s 上报运行状态到 `/pet/status`；Rust 侧 `GET /debug/status` 读取。

> 关键坑记录：① axum 方法不匹配回 405 且无 CORS 头 → POST 预检被浏览器拦，
> 需全局 OPTIONS 中间件（`server.rs cors_preflight`）；② WebView2 远程调试必须经
> `.additional_browser_args(...)`（环境变量不生效）；③ 重建后 CDP 枚举不可靠，以页面加载日志+遥测为准；
> ④ **交互修复**：WebView2/Tauri 没有 Electron 的鼠标事件转发（forward:true），整窗穿透+悬停翻转会变成
> “宠物永远点不中/拖不动/右键无反应”——已改为窗口始终可交互并收窄外扩余量（Rust 与 frontend 两端
> `WINDOW_MARGIN_RATIO=0.22` 同步）。
