/* 抓两张截图验证画面与动画推进 */
const PORT = 9229;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function main() {
  let targets = [];
  for (let i = 0; i < 20; i++) {
    try {
      targets = await (await fetch(`http://127.0.0.1:${PORT}/json`)).json();
      if (targets.some((t) => t.type === 'page')) break;
    } catch {}
    await sleep(400);
  }
  const page = targets.find((t) => t.type === 'page' && t.url.includes('index.html'));
  if (!page) return console.log('no page');
  const shot = async (name) => {
    const ws = new WebSocket(page.webSocketDebuggerUrl);
    await new Promise((res, rej) => { ws.onopen = res; ws.onerror = () => rej(new Error('ws')); });
    const data = await new Promise((res, rej) => {
      ws.onmessage = (e) => { const m = JSON.parse(e.data); if (m.id === 1) res(m); };
      ws.send(JSON.stringify({ id: 1, method: 'Page.captureScreenshot', params: { format: 'png' } }));
      setTimeout(() => rej(new Error('shot timeout')), 8000);
    });
    ws.close();
    const fs = await import('node:fs');
    fs.writeFileSync(name, Buffer.from(data.result.data, 'base64'));
    console.log('saved ' + name);
  };
  await shot('shot-a.png');
  await sleep(1200);
  await shot('shot-b.png');
}
main().catch((e) => { console.error('FAIL', e); process.exit(1); });
