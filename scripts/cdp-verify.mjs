/**
 * CDP 渲染验证 —— 连已启动的 Chrome（--remote-debugging-port=9222），
 * 打开目标 URL，抓取渲染后的可见文本 + console 报错 + 截图。
 *
 * 目的：确认 UI 真的挂载了。编译通过 ≠ 渲染正确（白屏也能 tsc exit 0）。
 * 依赖：Node 18+ 原生 WebSocket / fetch，无需 ws 包。
 *
 * 用法：node scripts/cdp-verify.mjs <url> [outPng]
 */
import { writeFileSync } from 'node:fs';

const url = process.argv[2] ?? 'http://127.0.0.1:5176/#/dashboard';
const outPng = process.argv[3] ?? null;

async function main() {
  const list = await (await fetch('http://127.0.0.1:9222/json/list')).json();
  const page = list.find((t) => t.type === 'page');
  if (!page) {
    throw new Error('no page target — Chrome 未以 --remote-debugging-port=9222 启动？');
  }

  const ws = new WebSocket(page.webSocketDebuggerUrl);
  let id = 0;
  const pending = new Map();
  const errors = [];

  const send = (method, params = {}) =>
    new Promise((resolve, reject) => {
      const msgId = ++id;
      pending.set(msgId, { resolve, reject });
      ws.send(JSON.stringify({ id: msgId, method, params }));
    });

  await new Promise((resolve, reject) => {
    ws.addEventListener('open', resolve, { once: true });
    ws.addEventListener('error', () => reject(new Error('websocket 连接失败')), { once: true });
  });

  ws.addEventListener('message', (ev) => {
    const msg = JSON.parse(ev.data);
    if (msg.id && pending.has(msg.id)) {
      const { resolve, reject } = pending.get(msg.id);
      pending.delete(msg.id);
      if (msg.error) reject(new Error(JSON.stringify(msg.error)));
      else resolve(msg.result);
      return;
    }
    if (msg.method === 'Runtime.exceptionThrown') {
      const d = msg.params?.exceptionDetails;
      errors.push(d?.exception?.description ?? d?.text ?? 'unknown exception');
    }
    if (msg.method === 'Runtime.consoleAPICalled' && msg.params?.type === 'error') {
      errors.push(
        (msg.params.args ?? []).map((a) => a.value ?? a.description ?? '').join(' '),
      );
    }
  });

  await send('Runtime.enable');
  await send('Page.enable');
  await send('Page.navigate', { url });
  await new Promise((r) => setTimeout(r, 3500));

  const read = async (expr) => {
    const r = await send('Runtime.evaluate', { expression: expr, returnByValue: true });
    return r.result?.value;
  };

  const out = {
    theme: await read('document.documentElement.dataset.theme'),
    bodyBg: await read('getComputedStyle(document.body).backgroundColor'),
    surfaceVar: await read(
      'getComputedStyle(document.documentElement).getPropertyValue("--surface").trim()',
    ),
    inkAlias: await read(
      'getComputedStyle(document.documentElement).getPropertyValue("--ink").trim()',
    ),
    rootLen: await read('document.getElementById("root")?.innerHTML.length ?? 0'),
    crashed: await read('document.body.innerText.includes("Jev-Switch UI crashed")'),
    text: await read('document.body.innerText'),
  };

  console.log('=== CDP VERIFY ===');
  console.log('url        :', url);
  console.log('theme      :', out.theme);
  console.log('body bg    :', out.bodyBg);
  console.log('--surface  :', out.surfaceVar);
  console.log('--ink alias:', out.inkAlias);
  console.log('#root len  :', out.rootLen);
  console.log('crashed    :', out.crashed);
  console.log('js errors  :', errors.length ? errors : '(none)');
  console.log('--- visible text ---');
  console.log(out.text);

  if (outPng) {
    const shot = await send('Page.captureScreenshot', { format: 'png' });
    writeFileSync(outPng, Buffer.from(shot.data, 'base64'));
    console.log('screenshot →', outPng);
  }

  ws.close();
  process.exit(out.crashed || errors.length > 0 ? 1 : 0);
}

main().catch((e) => {
  console.error('VERIFY FAILED:', e.message);
  process.exit(1);
});
