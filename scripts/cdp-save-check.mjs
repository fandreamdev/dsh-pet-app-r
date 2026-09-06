/* 保存重建复测：默认单宠 → size 400 保存 → 等重建 → 检查宽度/配置 */
const PORT = 9229;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function targets() { return (await (await fetch(`http://127.0.0.1:${PORT}/json`)).json()).filter((t) => t.type === 'page'); }
async function waitTargets(pred, tries = 24) {
  for (let i = 0; i < tries; i++) { const ts = await targets().catch(() => []); const h = ts.filter(pred); if (h.length) return h; await sleep(500); }
  return [];
}
async function ev(url, expression, id) {
  const ws = new WebSocket(url);
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = () => rej(new Error('ws')); });
  const r = await new Promise((res, rej) => {
    ws.onmessage = (e) => { const m = JSON.parse(e.data); if (m.id === id) res(m.result && m.result.result ? m.result.result.value : undefined); };
    ws.send(JSON.stringify({ id, method: 'Runtime.evaluate', params: { expression, returnByValue: true, awaitPromise: true } }));
    setTimeout(() => rej(new Error('timeout')), 15000);
  });
  ws.close();
  return r;
}
async function main() {
  await waitTargets((t) => t.url.includes('settings.html'));
  let s = (await targets()).find((t) => t.url.includes('settings.html'));
  await ev(s.webSocketDebuggerUrl, `(async()=>{const fs=document.getElementById('f-size'); fs.value='400'; fs.dispatchEvent(new Event('change',{bubbles:true})); document.getElementById('btn-save').click(); await new Promise(r=>setTimeout(r,1500)); return 1;})()`, 60);
  const pets = await waitTargets((t) => t.url.includes('index.html'));
  console.log('petsAfterSave=' + pets.length);
  for (const p of pets) {
    const w = await ev(p.webSocketDebuggerUrl, 'window.outerWidth', 61).catch(() => null);
    const st = await ev(p.webSocketDebuggerUrl, `JSON.stringify((window.__dshPetDebug||{}).errors)`, 62).catch(() => null);
    console.log('  width=' + w + ' errors=' + st);
  }
  const any = await targets();
  const p0 = any.find((t) => t.url.includes('index.html'));
  if (p0) {
    const api = new URL(p0.url).searchParams.get('api');
    const cfg = await (await fetch(api + '/config')).json();
    console.log('config size=' + cfg.main.pets[0].size);
  }
}
main().catch((e) => { console.error('FAIL', e); process.exit(1); });
