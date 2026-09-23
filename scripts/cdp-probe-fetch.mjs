/**
 * 在页面上下文里直接 fetch daemon，看跨源是否被拦（CORS）。
 * 页面在 :5176，daemon 在 :11435 —— 跨源，需 daemon 回 CORS 头。
 */
const list = await (await fetch('http://127.0.0.1:9222/json/list')).json();
const page = list.find((t) => t.type === 'page');
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
await send('Runtime.enable');

const probe = async (path) => {
  const r = await send('Runtime.evaluate', {
    expression: `
      fetch('http://127.0.0.1:11435${path}')
        .then(r => r.text().then(t => 'OK ' + r.status + ' :: ' + t.slice(0, 200)))
        .catch(e => 'ERR ' + e.message)
    `,
    awaitPromise: true,
    returnByValue: true,
  });
  return r.result?.value;
};

console.log('page origin:', (await send('Runtime.evaluate', { expression: 'location.origin', returnByValue: true })).result.value);
console.log('/health           →', await probe('/health'));
console.log('/v1/admin/status  →', await probe('/v1/admin/status'));
console.log('/v1/admin/providers →', await probe('/v1/admin/providers'));
console.log('/v1/admin/routes  →', await probe('/v1/admin/routes'));

ws.close();
