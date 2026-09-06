/**
 * dsh-pet-rust bridge —— 在 window.petBridge 上暴露与 Electron 版同名 API
 * （sprite.js 原样可跑），实现全部改为调 Rust 本地服务（HTTP + SSE）。
 */
'use strict';

(function () {
  if (window.petBridge) return;

  function post(path, body) {
    fetch(API + path, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(body || {}),
    }).catch(() => {});
  }

  const flightCbs = [];
  const hitCbs = [];
  let sse = null;

  window.__reportTele = (data) => post('/pet/status', { index: CONFIG.petIndex, data });

  window.petBridge = {
    // 位置跟随（本窗口序号寻址；box 为包围盒坐标，供碰撞站场）
    setBounds(x, y, width, height, boxX, boxY, size, bottomPad, vx, vy) {
      post('/pet/bounds', { index: CONFIG.petIndex, x, y, width, height, boxX, boxY, size, bottomPad, vx, vy });
    },
    setInteractive(interactive) {
      post('/pet/interactive', { index: CONFIG.petIndex, interactive: !!interactive });
    },
    // 打开设置（Rust 主进程开设置窗）
    openSettings() {
      post('/pet/open-settings', {});
    },
    // 跨窗碰撞
    reportFlight(state) {
      post('/pet/flight', {
        pet: window.__petId || state.id,
        x: state.x,
        y: state.y,
        size: state.size,
        bottomPad: state.bottomPad,
        vx: state.vx || 0,
        vy: state.vy || 0,
      });
    },
    onFlightStates(cb) {
      flightCbs.push(cb);
    },
    reportCollide(targetId, vx, vy) {
      post('/pet/collide', { targetId, vx, vy });
    },
    onPetHit(cb) {
      hitCbs.push(cb);
    },
    // 配置/素材就绪后由 renderer 调用（需要真实 pet id 过滤 hit）
    startEvents(petId) {
      if (sse) sse.close();
      sse = new EventSource(API + '/events?pet=' + encodeURIComponent(petId));
      sse.addEventListener('flight', (e) => {
        try {
          const states = JSON.parse(e.data);
          for (const cb of flightCbs) cb(states || []);
        } catch {
          /* ignore */
        }
      });
      sse.addEventListener('hit', (e) => {
        try {
          const d = JSON.parse(e.data);
          for (const cb of hitCbs) cb({ vx: d.vx, vy: d.vy });
        } catch {
          /* ignore */
        }
      });
      sse.addEventListener('config', () => {
        window.location.reload();
      });
      // 遥测：每 2s 上报 DOM/视频运行状态（Rust /debug/status 读取，验证/排障用）
      if (window.__teleTimer) clearInterval(window.__teleTimer);
      window.__teleTimer = setInterval(() => {
        const v = document.querySelector('video.is-front');
        const debug = window.__dshPetDebug || {};
        const data = {
          configOk: debug.configOk === true,
          spriteCount: debug.spriteCount || 0,
          errors: debug.errors || [],
          videoTime: v ? Math.round(v.currentTime * 10) / 10 : null,
          videos: document.querySelectorAll('video').length,
          pet: window.__petId || null,
        };
        post('/pet/status', { index: CONFIG.petIndex, data });
      }, 2000);
    },
  };
})();
