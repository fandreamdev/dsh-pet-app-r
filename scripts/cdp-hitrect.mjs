const PORT = 9229;
async function main() {
  const ts = await (await fetch(`http://127.0.0.1:${PORT}/json`)).json();
  const p = ts.find((t) => t.type === 'page' && t.url.includes('index.html'));
  if (!p) return console.log('no page');
  const ws = new WebSocket(p.webSocketDebuggerUrl);
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = () => rej(new Error('ws')); });
  const r = await new Promise((res) => { ws.onmessage = (e) => { const m = JSON.parse(e.data); if (m.id === 1) res(m); }; ws.send(JSON.stringify({ id: 1, method: 'Runtime.evaluate', params: { expression: `JSON.stringify((()=>{const h=document.querySelector('.pet-hit');const b=h.getBoundingClientRect();const s=document.querySelector('.pet-sprite');const sb=s.getBoundingClientRect();return {winInner:[window.innerWidth,window.innerHeight], spriteLeft:sb.left, hit:{x:b.left,y:b.top,w:b.width,h:b.height}}})())`, returnByValue: true } })); });
  ws.close();
  console.log(r.result.result.value);
}
main().catch((e) => { console.error(e); process.exit(1); });
