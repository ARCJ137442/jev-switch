/**
 * CDP 硬重载 —— 强制刷新页面（忽略缓存），用于 HMR 未生效时。
 * 注意：Node 原生 WebSocket 用 addEventListener，没有 ws 包的 .on()。
 */
const list = await (await fetch('http://127.0.0.1:9222/json/list')).json();
const page = list.find((t) => t.type === 'page');
if (!page) throw new Error('no page target');

const ws = new WebSocket(page.webSocketDebuggerUrl);
let id = 0;
const pending = new Map();

const send = (method, params = {}) =>
  new Promise((resolve, reject) => {
    const msgId = ++id;
    pending.set(msgId, { resolve, reject });
    ws.send(JSON.stringify({ id: msgId, method, params }));
  });

await new Promise((r) => ws.addEventListener('open', r, { once: true }));
ws.addEventListener('message', (ev) => {
  const msg = JSON.parse(ev.data);
  if (msg.id && pending.has(msg.id)) {
    const { resolve, reject } = pending.get(msg.id);
    pending.delete(msg.id);
    msg.error ? reject(new Error(JSON.stringify(msg.error))) : resolve(msg.result);
  }
});

await send('Page.enable');
await send('Page.reload', { ignoreCache: true });
console.log('reloaded (ignoreCache)');
await new Promise((r) => setTimeout(r, 800));
ws.close();
