/**
 * CDP 网络日志捕获 —— 监听浏览器发往 :11435 的所有请求 + console 输出
 */
import { setTimeout as delay } from 'node:timers/promises';

async function main() {
  const list = await (await fetch('http://127.0.0.1:9222/json/list')).json();
  const page = list.find((t) => t.type === 'page');
  if (!page) throw new Error('no page target');

  const ws = new WebSocket(page.webSocketDebuggerUrl);
  let id = 0;
  const pending = new Map();
  const netLog = [];
  const consoleLog = [];

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
    if (msg.method === 'Network.responseReceived') {
      const r = msg.params.response;
      if (r.url.includes('11435')) {
        netLog.push(`${r.status} ${msg.params.type} ${r.url}`);
      }
    }
    if (msg.method === 'Network.loadingFailed') {
      const p = msg.params;
      if (p.request?.url.includes('11435')) {
        netLog.push(`FAILED ${p.type} ${p.request.url} — ${p.errorText}`);
      }
    }
    if (msg.method === 'Runtime.consoleAPICalled') {
      const args = msg.params.args.map((a) => a.value ?? a.description ?? '').join(' ');
      consoleLog.push(`[${msg.params.type}] ${args}`);
    }
  });

  await send('Network.enable');
  await send('Runtime.enable');
  console.log('monitoring for 6 seconds...');
  await delay(6000);

  console.log('=== network to :11435 ===');
  console.log(netLog.length ? netLog.join('\n') : '(none captured)');
  console.log('\n=== console (last 15) ===');
  console.log(consoleLog.slice(-15).join('\n') || '(empty)');

  ws.close();
}

main().catch((e) => {
  console.error('FAILED:', e.message);
  process.exit(1);
});
