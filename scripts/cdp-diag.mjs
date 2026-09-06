/* 快速诊断：页面脚本/全局/API 可达性 */
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
  if (!page) {
    console.log('no page');
    return;
  }
  const api = new URL(page.url).searchParams.get('api');
  const openWs = async () => {
    const ws = new WebSocket(page.webSocketDebuggerUrl);
    await new Promise((res, rej) => {
      ws.onopen = res;
      ws.onerror = () => rej(new Error('ws err'));
    });
    return ws;
  };
  const ev = async (expression, id) => {
    const ws = await openWs();
    const r = await new Promise((res, rej) => {
      ws.onmessage = (e) => {
        const m = JSON.parse(e.data);
        if (m.id === id) res(m);
      };
      ws.send(JSON.stringify({ id, method: 'Runtime.evaluate', params: { expression, returnByValue: true, awaitPromise: true } }));
      setTimeout(() => rej(new Error('timeout ' + expression.slice(0, 60))), 8000);
    });
    ws.close();
    return r.result && r.result.result ? r.result.result.value : (r.result && r.result.exceptionDetails ? 'EXC:' + JSON.stringify(r.result.exceptionDetails) : undefined);
  };
  console.log('API=' + api);
  console.log('globals=' + (await ev(`JSON.stringify({S: !!window.PetShared, bridge: !!window.petBridge, report: typeof window.__reportTele, scripts: document.scripts.length, title: document.title})`, 10)));
  console.log('fetchTest=' + (await ev(`(async()=>{try{const r=await fetch('${api}/config');return 'ok:'+r.status+':'+(await r.text()).slice(0,60)}catch(e){return 'ERR:'+e.message}})()`, 11)));
  console.log('dbg=' + (await ev(`JSON.stringify(window.__dshPetDebug||null)`, 12)));
}

main().catch((e) => {
  console.error('FAIL', e);
  process.exit(1);
});
