/**
 * CDP 全程网络追踪 —— 先开 Network 域再导航，捕获首屏所有请求。
 * 修正 cdp-inject.mjs 的测量缺陷（拦截器装在挂载后，错过初始 fetch）。
 *
 * 用法：node scripts/cdp-trace.mjs <url> [outPng]
 */
import { writeFileSync } from 'node:fs';

const url = process.argv[2] ?? 'http://127.0.0.1:5176/#/dashboard';
const outPng = process.argv[3] ?? null;

const list = await (await fetch('http://127.0.0.1:9222/json/list')).json();
const page = list.find((t) => t.type === 'page');
if (!page) throw new Error('no page target');

const ws = new WebSocket(page.webSocketDebuggerUrl);
let id = 0;
const pending = new Map();
const reqs = new Map();
const net = [];
const errors = [];

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
    return;
  }
  const p = msg.params;
  if (msg.method === 'Network.requestWillBeSent' && p.request.url.includes('11435')) {
    reqs.set(p.requestId, p.request.url);
  }
  if (msg.method === 'Network.responseReceived' && reqs.has(p.requestId)) {
    net.push(`${p.response.status} ${reqs.get(p.requestId)}`);
  }
  if (msg.method === 'Network.loadingFailed' && reqs.has(p.requestId)) {
    net.push(`FAILED ${reqs.get(p.requestId)} — ${p.errorText}`);
  }
  if (msg.method === 'Runtime.exceptionThrown') {
    const d = p.exceptionDetails;
    errors.push(d?.exception?.description ?? d?.text ?? 'exception');
  }
  if (msg.method === 'Runtime.consoleAPICalled' && p.type === 'error') {
    errors.push((p.args ?? []).map((a) => a.value ?? a.description ?? '').join(' '));
  }
});

await send('Network.enable');
await send('Runtime.enable');
await send('Page.enable');
await send('Page.navigate', { url });
await new Promise((r) => setTimeout(r, 4000));

const read = async (expr) => {
  const r = await send('Runtime.evaluate', { expression: expr, returnByValue: true });
  return r.result?.value;
};

console.log('=== requests to :11435 (from navigation) ===');
console.log(net.length ? net.join('\n') : '(none)');
console.log('\n=== js errors ===');
console.log(errors.length ? errors.join('\n') : '(none)');
console.log('\n=== rendered text ===');
console.log(await read('document.body.innerText'));

if (outPng) {
  const shot = await send('Page.captureScreenshot', { format: 'png' });
  writeFileSync(outPng, Buffer.from(shot.data, 'base64'));
  console.log('\nscreenshot →', outPng);
}

ws.close();
