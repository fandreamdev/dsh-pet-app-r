/* CDP 交互验证：点击回应、右键菜单、拖拽释放 */
const PORT = 9229;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function pages() {
  return (await (await fetch(`http://127.0.0.1:${PORT}/json`)).json()).filter((t) => t.type === 'page');
}
async function withWs(url, fn) {
  const ws = new WebSocket(url);
  await new Promise((res, rej) => { ws.onopen = res; ws.onerror = () => rej(new Error('ws')); });
  const result = await fn(ws);
  ws.close();
  return result;
}
async function ev(url, expression, id) {
  return withWs(url, (ws) =>
    new Promise((res, rej) => {
      ws.onmessage = (e) => { const m = JSON.parse(e.data); if (m.id === id) res(m.result && m.result.result ? m.result.result.value : undefined); };
      ws.send(JSON.stringify({ id, method: 'Runtime.evaluate', params: { expression, returnByValue: true, awaitPromise: true } }));
      setTimeout(() => rej(new Error('ev timeout')), 10000);
    }),
  );
}

async function main() {
  let ps = [];
  for (let i = 0; i < 20; i++) {
    ps = await pages().catch(() => []);
    if (ps.length >= 1) break;
    await sleep(400);
  }
  const page = ps.find((p) => p.url.includes('petIndex=0'));
  if (!page) return console.log('no pet-0 page: ' + JSON.stringify(ps.map((p) => p.url)));
  const u = page.webSocketDebuggerUrl;

  const s0 = await ev(u, `JSON.stringify({spriteCount: sprites.length, anim: sprites[0] && sprites[0].anim, facing: sprites[0] && sprites[0].facing})`, 1);
  console.log('baseline=' + s0);

  // ---- 点击（命中区中央 pointerdown/up）----
  const clickRes = await ev(
    u,
    `(async()=>{
      const hit=document.querySelector('.pet-hit'); if(!hit) return 'no-hit';
      const r=hit.getBoundingClientRect(); const x=r.left+r.width/2, y=r.top+r.height/2;
      const fire=(type,opts)=>hit.dispatchEvent(new PointerEvent(type,{bubbles:true,cancelable:true,pointerId:1,pointerType:'mouse',button:opts&&opts.button,clientX:x,clientY:y,screenX:x,screenY:y}));
      fire('pointerdown',{button:0}); window.dispatchEvent(new PointerEvent('pointerup',{bubbles:true,pointerId:1,pointerType:'mouse',button:0,clientX:x,clientY:y}));
      await new Promise(r=>setTimeout(r,600));
      return JSON.stringify({anim:sprites[0].anim, once:sprites[0].once, pendingSquash:sprites[0].pendingSquash});
    })()`,
    2,
  );
  console.log('afterClick=' + clickRes);

  // ---- 右键菜单 ----
  const menuRes = await ev(
    u,
    `(async()=>{
      const hit=document.querySelector('.pet-hit'); const r=hit.getBoundingClientRect();
      hit.dispatchEvent(new MouseEvent('contextmenu',{bubbles:true,cancelable:true,clientX:r.left+r.width/2,clientY:r.top+r.height/2,button:2}));
      await new Promise(r=>setTimeout(r,300));
      const cols=document.querySelectorAll('.dsh-pet-menu-column').length;
      const open=sprites[0].menuOpen;
      // Esc 关闭
      window.dispatchEvent(new KeyboardEvent('keydown',{key:'Escape',bubbles:true}));
      await new Promise(r=>setTimeout(r,300));
      return JSON.stringify({cols, openBefore: open, openAfter: sprites[0].menuOpen});
    })()`,
    3,
  );
  console.log('contextMenu=' + menuRes);

  // ---- 拖拽（位移 >5px 再松开）----
  const dragRes = await ev(
    u,
    `(async()=>{
      const hit=document.querySelector('.pet-hit'); const r=hit.getBoundingClientRect();
      const x0=r.left+r.width/2, y0=r.top+r.height/2;
      const fire=(t,el,x,y)=>el.dispatchEvent(new PointerEvent(t,{bubbles:true,cancelable:true,pointerId:2,pointerType:'mouse',button:0,clientX:x,clientY:y}));
      const before=sprites[0].pos.x;
      fire('pointerdown',hit,x0,y0);
      await new Promise(r=>setTimeout(r,120));
      fire('pointermove',hit,x0+120,y0+40);  // 超过 5px 阈值 → 进入拖拽
      await new Promise(r=>setTimeout(r,200));
      const midDrag=sprites[0].dragState.dragging;
      const midAnim=sprites[0].anim;
      window.dispatchEvent(new PointerEvent('pointerup',{bubbles:true,pointerId:2,pointerType:'mouse',button:0,clientX:x0+120,clientY:y0+40}));
      await new Promise(r=>setTimeout(r,900));
      return JSON.stringify({before, after:sprites[0].pos.x, midDrag, midAnim, draggingNow:sprites[0].dragState.dragging, customPos:sprites[0].customPos});
    })()`,
    4,
  );
  console.log('drag=' + dragRes);

  // 最终状态
  const fin = await ev(u, `JSON.stringify({errors: window.__dshPetDebug.errors, anim: sprites[0] && sprites[0].anim})`, 5);
  console.log('final=' + fin);
}

main().catch((e) => { console.error('FAIL', e); process.exit(1); });
