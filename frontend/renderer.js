/**
 * dsh-pet-rust renderer —— 启动入口。
 *
 * 依赖链：shared-core.js → constants.js → bridge.js → sprite.js → renderer.js。
 * 配置经 Rust 本地服务 GET {API}/config（merged 成品，字段填满、与上游形状一致），
 * 前端复用 S.flattenConfigPets 零改动拍平；失败大声报错 + 5s 重试。
 */
'use strict';

// 早期失败也要能上报（Rust /debug/status 读取，排障用）
function reportTele(extra) {
  try {
    if (window.__reportTele) window.__reportTele(Object.assign({ t: Date.now() }, extra));
  } catch {
    /* ignore */
  }
}
window.addEventListener('error', (e) => reportTele({ runtimeError: String(e.message || e.error) }));
reportTele({ boot: 'start' });

function showError(message) {
  console.error('[dsh-pet-rust] ' + message);
  window.__dshPetDebug.configOk = false;
  errorEl.textContent = 'dsh-pet-rust 配置错误：' + message;
  errorEl.classList.add('visible');
}
function hideError() {
  errorEl.classList.remove('visible');
  errorEl.textContent = '';
}
function scheduleReboot() {
  if (bootTimer) return;
  bootTimer = setTimeout(() => {
    bootTimer = null;
    void boot();
  }, 5000);
}

async function loadConfig() {
  if (!API) throw new Error('缺少 api 参数');
  const res = await fetch(API + '/config', { cache: 'no-store' });
  if (!res.ok) throw new Error('config http ' + res.status);
  const merged = await res.json();
  return {
    pets: S.flattenConfigPets(merged),
    physics: (merged && merged.main && merged.main.physics) || S.DEFAULT_PHYSICS,
  };
}

async function boot() {
  try {
    const cfg = await loadConfig();
    config = cfg;
    hideError();
    const pets = cfg.pets.filter((p) => S.isDesktopVisible(p.display));
    if (pets.length === 0) {
      showError('配置中没有 display 为 desktop/both 的宠物');
      scheduleReboot();
      return;
    }
    const pet = pets[CONFIG.petIndex];
    if (!pet) {
      showError('petIndex=' + CONFIG.petIndex + ' 超出桌面宠物列表（共 ' + pets.length + ' 只）');
      scheduleReboot();
      return;
    }
    for (const s of sprites) s.dispose();
    sprites = [new PetSprite(pet)];
    window.__petId = pet.id;
    window.__dshPetDebug.configOk = true;
    window.__dshPetDebug.spriteCount = sprites.length;
    if (window.petBridge && window.petBridge.startEvents) window.petBridge.startEvents(pet.id);
    for (const s of sprites) s.playIdle();
    reportTele({ boot: 'ok', configOk: true, spriteCount: sprites.length, pet: pet.id });
  } catch (e) {
    reportTele({ boot: 'error', fatal: String(e && e.message ? e.message : e) });
    showError('配置加载失败：' + (e && e.message ? String(e.message) : String(e)));
    scheduleReboot();
  }
}

// 注入字体 + 光标（Rust 服务提供）
function injectAssets() {
  const style = document.createElement('style');
  style.textContent =
    '@font-face{font-family:"ShangshouSoftCandy";src:url("' +
    FONT_URL +
    encodeURIComponent('上首软糖体') +
    '.ttf") format("truetype");font-display:swap;font-weight:400}' +
    '.pet-hit{cursor:url("' + PIC_URL + 'cursor-grab.png") 16 16, grab}' +
    '.pet-hit.dragging{cursor:url("' + PIC_URL + 'cursor-grabbing.png") 16 16, grabbing}';
  document.head.appendChild(style);
  const menuStyle = document.createElement('style');
  menuStyle.textContent = S.MENU_CSS;
  document.head.appendChild(menuStyle);
}

window.addEventListener('resize', () => {
  for (const s of sprites) s.position();
});

injectAssets();
void boot();
