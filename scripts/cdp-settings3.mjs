/* 设置窗往返 v3：confirm stub；保存→验证重建→恢复默认→验证回落 */
const PORT = 9229;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function targets() {
  return (await (await fetch(`http://127.0.0.1:${PORT}/json`)).json()).filter((t) => t.type === 'page');
}
async function waitTargets(pred, tries = 20) {
  for (let i = 0; i < tries; i++) {
    const ts = await targets().catch(() => []);
    const hit = ts.filter(pred);
    if (hit.length) return hit;
    await sleep(500);
  }
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
  const settings = await waitTargets((t) => t.url.includes('settings.html'));
  if (!settings.length) return console.log('no settings');
  const su = settings[0].webSocketDebuggerUrl;

  // 保存 size=400
  await ev(su, `(async()=>{const fs=document.getElementById('f-size'); fs.value='400'; fs.dispatchEvent(new Event('change',{bubbles:true})); document.getElementById('btn-save').click(); await new Promise(r=>setTimeout(r,2000)); return document.getElementById('msg').textContent;})()`, 50);
  const pets = await waitTargets((t) => t.url.includes('index.html'));
  const widths = [];
  for (const p of pets) widths.push(await ev(p.webSocketDebuggerUrl, 'window.outerWidth', 51).catch(() => null));
  console.log('afterSave pets=' + pets.length + ' widths=' + JSON.stringify(widths));

  // 恢复默认（stub confirm）
  await ev(su, `(async()=>{ window.confirm=()=>true; document.getElementById('btn-reset').click(); await new Promise(r=>setTimeout(r,2500)); return document.getElementById('msg').textContent; })()`, 52);
  await sleep(2500);
  const pets2 = await waitTargets((t) => t.url.includes('index.html'));
  console.log('afterReset pets=' + pets2.length);
  const any = await targets();
  const p0 = any.find((t) => t.url.includes('index.html'));
  if (p0) {
    const api = new URL(p0.url).searchParams.get('api');
    const cfg = await (await fetch(api + '/config')).json();
    console.log('config pets=' + cfg.main.pets.length + ' first=' + cfg.main.pets[0].id + ' size=' + cfg.main.pets[0].size);
    const w = await ev(p0.webSocketDebuggerUrl, 'window.outerWidth', 53).catch(() => null);
    console.log('pet0Width=' + w);
  }
}
main().catch((e) => { console.error('FAIL', e); process.exit(1); });
