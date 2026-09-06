# SOURCES.md —— 素材与共享代码来源（仅追溯用途）

本仓库**自包含**：以下内容已随仓库维护，运行时不依赖任何外部目录；本文件仅记录它们最初从哪个项目同步而来，以及如何按需刷新。

| 目录/文件 | 来源 | 说明 |
|---|---|---|
| `assets/webm/`（106 个 VP9-alpha webm） | dsh-pet 插件包 `assets/webm/`（本地副本位于 `../dsh-pet-app/assets` / `../dsh-pet/dsh-pet/assets`） | 播放动画，唯一格式；**允许开源使用、禁止商用** |
| `assets/fonts/`、`assets/pic/` | 同上 | 气泡字体、光标/图标 |
| `assets/config.jsonc` | 同上 | 默认配置（配置模型单一来源） |
| `frontend/shared-core.js` | `src/shared/index.ts` 的 esbuild IIFE 构建产物（`window.PetShared`） | 纯逻辑（物理/菜单/拍平…），已入库 |
| `frontend/sprite.js`、`settings.css` | dsh-pet desktop helper（`runtime/electron-helper/`）的改编版 | 本仓库自行维护（sprite 为上游 PetSprite 小改） |
| `icons/icon.ico` | 由 dsh-pet-app `assets/pic/notify-done.png` 封装生成 | tauri-build Windows 资源图标 |

## 刷新（可选，开发期）

```sh
node scripts/sync-from-siblings.mjs   # 从 ../dsh-pet-app/assets（或 UPSTREAM=… 指定）同步素材
node scripts/build-shared-core.mjs    # 若同时刷新了 src/shared，重建 frontend/shared-core.js
```

> 刷新脚本只是开发便利工具：仓库内已自带的素材与构建产物不需要它们即可运行。
> 许可提醒：素材沿用上游约束（允许开源使用、禁止商用），分发本仓库时请保留本说明。
