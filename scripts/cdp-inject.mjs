/**
 * CDP 注入调试代码 —— 在目标页面里执行 JS，验证组件逻辑是否运行
 */
import { setTimeout as delay } from 'node:timers/promises';

async function main() {
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

  await send('Runtime.enable');

  // 注入调试代码：拦截 fetch，记录所有 :11435 请求
  const injectCode = `
    (function() {
      const orig = window.fetch;
      window.__jev_fetches__ = [];
      window.fetch = function(...args) {
        const url = typeof args[0] === 'string' ? args[0] : args[0]?.url;
        if (url?.includes('11435')) {
          console.log('[FETCH]', url);
          window.__jev_fetches__.push({ url, time: Date.now() });
        }
        return orig.apply(this, args);
      };
      console.log('[INJECT] fetch interceptor installed');
    })();
  `;

  await send('Runtime.evaluate', { expression: injectCode });
  console.log('injected fetch interceptor, waiting 5s...');
  await delay(5000);

  const fetchLog = await send('Runtime.evaluate', {
    expression: 'window.__jev_fetches__ ?? []',
    returnByValue: true,
  });

  console.log('=== captured fetches to :11435 ===');
  if (fetchLog.result?.value?.length) {
    fetchLog.result.value.forEach((f) => console.log(f.url));
  } else {
    console.log('(none — useEffect may not have fired)');
  }

  ws.close();
}

main().catch((e) => {
  console.error('FAILED:', e.message);
  process.exit(1);
});
