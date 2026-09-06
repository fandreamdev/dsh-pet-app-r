/* 第二轮：点击(含 click 事件) + 右键菜单(直接调用) */
const PORT = 9229;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
async function pages() {
  return (await (await fetch(`http://127.0.0.1:${PORT}/json`)).json()).filter((t) => t.type === 'page');
}
async function ev(url, expression, id) {
  const ws = new WebSocket(url);
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = () => rej(new Error('ws')); });
  const r = await new Promise((res, rej) => {
    ws.onmessage = (e) => { const m = JSON.parse(e.data); if (m.id === id) res(m.result && m.result.result ? m.result.result.value : undefined); };
    ws.send(JSON.stringify({ id, method: 'Runtime.evaluate', params: { expression, returnByValue: true, awaitPromise: true } }));
    setTimeout(() => rej(new Error('timeout')), 10000);
  });
  ws.close();
  return r;
}
async function main() {
  const ps = await pages();
  const page = ps.find((p) => p.url.includes('petIndex=0'));
  if (!page) return console.log('no page');
  const u = page.webSocketDebuggerUrl;

  const click = await ev(
    u,
    `(async()=>{
      const hit=document.querySelector('.pet-hit'); const r=hit.getBoundingClientRect();
      const x=r.left+r.width/2, y=r.top+r.height/2;
      hit.dispatchEvent(new PointerEvent('pointerdown',{bubbles:true,cancelable:true,pointerId:7,pointerType:'mouse',button:0,clientX:x,clientY:y}));
      window.dispatchEvent(new PointerEvent('pointerup',{bubbles:true,pointerId:7,pointerType:'mouse',button:0,clientX:x,clientY:y}));
      hit.dispatchEvent(new MouseEvent('click',{bubbles:true,cancelable:true,clientX:x,clientY:y}));
      await new Promise(r=>setTimeout(r,700));
      const s=sprites[0];
      return JSON.stringify({anim:s.anim, once:s.once});
    })()`,
    21,
  );
  console.log('click=' + click);

  const menu = await ev(
    u,
    `(async()=>{
      const s=sprites[0];
      s.onContextMenu({preventDefault(){},clientX:120,clientY:90,button:2});
      await new Promise(r=>setTimeout(r,350));
      const cols=[...document.querySelectorAll('div')].filter(d=>d.className&&typeof d.className==='string'&&d.className.includes('dsh-pet-menu')).map(d=>d.className);
      const open=s.menuOpen;
      let closed=false;
      if(open){ s.menuClose && s.menuClose(); s.menuOpen=false; window.__dshPetDebug.menuOpen=false; closed=true; }
      return JSON.stringify({open, cols:[...new Set(cols)].slice(0,6), closed});
    })()`,
    22,
  );
  console.log('menu=' + menu);
}
main().catch((e) => { console.error('FAIL', e); process.exit(1); });
