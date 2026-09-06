/**
 * dsh-pet-rust renderer constants —— 基础设施。
 *
 * 经典 script 顺序加载：shared-core.js → constants.js → bridge.js → sprite.js → renderer.js。
 * 与 dsh-pet-app（Electron）constants.js 的差异：
 *   - 数据源 = 本地 HTTP API（127.0.0.1:<port>，Rust axum），api base 经 query 注入；
 *   - ASSET_BASE / FONT_URL / PIC_URL 指向该 API（Rust 端做素材解析 + Range）。
 */
'use strict';

const S = window.PetShared;

const params = new URLSearchParams(location.search);
const API = params.get('api') || '';
const CONFIG = {
  petIndex: Number(params.get('petIndex') || '0'),
  scale: Number(params.get('scale') || '1'),
  workAreaW: Number(params.get('workAreaW') || (window.screen && window.screen.availWidth) || 1920),
  workAreaH: Number(params.get('workAreaH') || (window.screen && window.screen.availHeight) || 1080),
};

// 视口 = 主屏工作区（漫游边界/角落定位/位置换算用它）
const VIEW = { w: CONFIG.workAreaW, h: CONFIG.workAreaH };

/** Rust 本地服务素材端点 */
const ASSET_BASE = API + '/thumb/';
const FONT_URL = API + '/font/';
const PIC_URL = API + '/pic/';

const BUBBLE_DURATION_MS = 10 * 1000;
const WINDOW_MARGIN_RATIO = 0.5;

// ---------- 全局状态 ----------
const rootEl = document.getElementById('root');
const errorEl = document.getElementById('pet-error');
let config = null; // { pets: 拍平实例, physics }
let sprites = [];
let bootTimer = null;

window.__dshPetDebug = { errors: [], configOk: false, spriteCount: 0, bootAt: Date.now() };
window.addEventListener('error', (event) => {
  window.__dshPetDebug.errors.push(String(event.message || event.error));
});
