const PORT = 9229;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function main() {
  let ts = [];
  for (let i = 0; i < 15; i++) { try { ts = await (await fetch(`http://127.0.0.1:${PORT}/json`)).json(); if (ts.length) break; } catch {} await sleep(400); }
  const p = ts.find((t) => t.type === 'page' && t.url.includes('index.html'));
  if (!p) return console.log('no page');
  const ws = new WebSocket(p.webSocketDebuggerUrl);
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = () => rej(new Error('ws')); });
  const r = await new Promise((res) => { ws.onmessage = (e) => { const m = JSON.parse(e.data); if (m.id === 1) res(m); }; ws.send(JSON.stringify({ id: 1, method: 'Runtime.evaluate', params: { expression: 'window.outerWidth+"x"+window.outerHeight', returnByValue: true } })); });
  ws.close();
  console.log('outer=' + r.result.result.value);
}
main().catch((e) => { console.error(e); process.exit(1); });
