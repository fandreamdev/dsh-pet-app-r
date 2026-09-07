/**
 * dsh-pet-rust 设置页 —— 纯 DOM，无框架；数据走 Rust 本地服务 API。
 *
 * 与 dsh-pet-app 的设置页同一份 UI，仅把 settingsBridge IPC 换成 HTTP fetch。
 */
'use strict';

const params = new URLSearchParams(location.search);
const API = params.get('api') || '';
const $ = (id: string): any => document.getElementById(id);

let pets: any[] = [];
let template: any = null;
let selId: string | null = null;

async function req(path: string, opts?: any) {
  const res = await fetch(API + path, opts || {});
  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new Error((body && body.error) || ('http ' + res.status));
  }
  return res.json();
}

function msg(text: string, isErr?: boolean) {
  const el = $('msg');
  el.textContent = text;
  el.className = 'msg' + (isErr ? ' err' : '');
}

function renderChips() {
  const box = $('pet-chips');
  box.innerHTML = '';
  for (const p of pets) {
    const chip = document.createElement('span');
    chip.className = 'chip' + (p.id === selId ? ' active' : '');
    chip.textContent = (p.name || p.id) + ' · ' + p.size;
    chip.onclick = () => select(p.id);
    box.appendChild(chip);
  }
}

function fillForm(p) {
  $('f-name').value = p.name || p.id;
  $('f-size').value = p.size;
  const pos = p.position || {};
  $('f-corner').value = pos.corner || 'bottom-right';
  $('f-marginX').value = Number.isFinite(Number(pos.marginX)) ? pos.marginX : 24;
  $('f-marginY').value = Number.isFinite(Number(pos.marginY)) ? pos.marginY : 100;
}

function readForm() {
  const p = pets.find((x) => x.id === selId);
  if (!p) return;
  p.name = $('f-name').value.trim() || p.id;
  p.size = Math.max(120, Number($('f-size').value) || p.size);
  p.position = p.position || {};
  p.position.corner = $('f-corner').value;
  p.position.marginX = Number($('f-marginX').value) || 0;
  p.position.marginY = Number($('f-marginY').value) || 0;
}

function select(id) {
  selId = id;
  renderChips();
  const p = pets.find((x) => x.id === id);
  if (p) fillForm(p);
  $('btn-del').disabled = pets.length <= 1;
}

async function load() {
  const merged = await req('/config');
  const main = merged && merged.main;
  const list = (main && Array.isArray(main.pets) ? main.pets : []).slice();
  pets = list;
  template = list.length ? JSON.parse(JSON.stringify(list[0])) : null;
  if (pets.length) select(pets[0].id);
  else {
    $('pet-chips').innerHTML = '';
    msg('没有可编辑的宠物', true);
  }
  fillPhysics(main && main.physics);
}

function fillPhysics(ph) {
  if (!ph) return;
  $('p-gravity').value = ph.gravity;
  $('p-restitution').value = ph.restitution;
  $('p-groundFriction').value = ph.groundFriction;
  $('p-ceilingBounce').checked = !!ph.ceilingBounce;
  $('p-throwPower').value = ph.throwPower;
  $('p-petCollision').checked = !!ph.petCollision;
}

function readPhysics() {
  const num = (id, fallback) => {
    const v = Number($(id).value);
    return Number.isFinite(v) ? v : fallback;
  };
  return {
    gravity: num('p-gravity', 1400),
    restitution: num('p-restitution', 0.78),
    groundFriction: num('p-groundFriction', 2.5),
    ceilingBounce: $('p-ceilingBounce').checked,
    throwPower: num('p-throwPower', 1.0),
    petCollision: $('p-petCollision').checked,
  };
}

async function showMeta() {
  try {
    const m = await req('/meta');
    $('meta').textContent =
      `默认配置：${m.defaultPath}\n` +
      `用户配置：${m.userPath}\n` +
      `自定义素材：${m.customDir}\n` +
      `素材根：${m.assetRoot}\n` +
      `userData：${m.userData}`;
  } catch {
    $('meta').textContent = '(meta 获取失败)';
  }
}

$('btn-add').onclick = () => {
  readForm();
  if (!template) return;
  const fresh = JSON.parse(JSON.stringify(template));
  const base = 'pet-' + (pets.length + 1);
  fresh.id = base;
  fresh.name = base;
  pets.push(fresh);
  select(fresh.id);
};

$('btn-del').onclick = () => {
  if (pets.length <= 1) return;
  if (!window.confirm('删除宠物「' + selId + '」？（保存后生效）')) return;
  readForm();
  pets = pets.filter((p) => p.id !== selId);
  if (pets.length) select(pets[0].id);
  else renderChips();
};

$('btn-save').onclick = async () => {
  readForm();
  try {
    await req('/config', {
      method: 'PUT',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ pets: pets.map((p) => ({ ...p })), physics: readPhysics() }),
    });
    msg('已保存，宠物即时更新');
    await load();
  } catch (e) {
    msg('保存失败：' + e.message, true);
  }
};

$('btn-reset').onclick = async () => {
  if (!window.confirm('恢复默认配置？将删除 userData 中整个 config.json（含手工编辑字段），不可恢复。')) return;
  try {
    await req('/config', { method: 'DELETE' });
    msg('已恢复默认');
    await load();
  } catch (e) {
    msg('恢复失败：' + e.message, true);
  }
};

for (const id of ['f-name', 'f-size', 'f-corner', 'f-marginX', 'f-marginY']) {
  $(id).addEventListener('change', () => {
    readForm();
    renderChips();
  });
}

void load().catch((e) => msg('加载配置失败：' + e.message, true));
void showMeta();
