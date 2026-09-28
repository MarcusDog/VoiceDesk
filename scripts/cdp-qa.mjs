import fs from 'node:fs/promises';
import path from 'node:path';

const endpoint = process.env.VOICEDESK_CDP || 'http://127.0.0.1:9223';
const outputDirectory = path.resolve('.gstack/qa-reports/screenshots');
await fs.mkdir(outputDirectory, { recursive: true });

const targets = await fetch(`${endpoint}/json/list`).then(response => response.json());
const target = targets.find(item => item.type === 'page' && item.url.includes('127.0.0.1:4173'));
if (!target) throw new Error('VoiceDesk browser target was not found');

const socket = new WebSocket(target.webSocketDebuggerUrl);
await new Promise((resolve, reject) => {
  socket.onopen = resolve;
  socket.onerror = reject;
});

let commandId = 0;
const pending = new Map();
const browserEvents = [];
socket.onmessage = event => {
  const message = JSON.parse(event.data);
  if (message.id) {
    const handler = pending.get(message.id);
    pending.delete(message.id);
    if (message.error) handler?.reject(new Error(message.error.message));
    else handler?.resolve(message.result);
    return;
  }
  if (['Runtime.exceptionThrown', 'Log.entryAdded'].includes(message.method)) {
    browserEvents.push(message);
  }
};

function command(method, params = {}) {
  const id = ++commandId;
  socket.send(JSON.stringify({ id, method, params }));
  return new Promise((resolve, reject) => pending.set(id, { resolve, reject }));
}

async function evaluate(expression) {
  const result = await command('Runtime.evaluate', {
    expression,
    awaitPromise: true,
    returnByValue: true,
  });
  if (result.exceptionDetails) throw new Error(result.exceptionDetails.text);
  return result.result.value;
}

async function screenshot(name) {
  const result = await command('Page.captureScreenshot', {
    format: 'png',
    captureBeyondViewport: false,
  });
  const file = path.join(outputDirectory, name);
  await fs.writeFile(file, Buffer.from(result.data, 'base64'));
  return file;
}

async function clickByText(text) {
  const clicked = await evaluate(`(() => {
    const target = [...document.querySelectorAll('button')]
      .find(element => (
        element.textContent.trim().includes(${JSON.stringify(text)})
        || element.getAttribute('aria-label') === ${JSON.stringify(text)}
      ) && !element.disabled);
    if (!target) return false;
    target.click();
    return true;
  })()`);
  if (!clicked) throw new Error(`Enabled button not found: ${text}`);
  await new Promise(resolve => setTimeout(resolve, 250));
}

async function waitFor(expression, timeout = 8000) {
  const startedAt = Date.now();
  while (Date.now() - startedAt < timeout) {
    if (await evaluate(expression)) return;
    await new Promise(resolve => setTimeout(resolve, 100));
  }
  throw new Error(`Timed out waiting for: ${expression}`);
}

await command('Page.enable');
await command('Runtime.enable');
await command('Log.enable');
await command('Page.reload', { ignoreCache: true });
await waitFor(`document.querySelector('.app-shell') !== null`);

const checks = [];
const record = async (name, expression) => {
  const value = await evaluate(expression);
  checks.push({ name, value });
};

await command('Emulation.setDeviceMetricsOverride', {
  width: 1440,
  height: 900,
  deviceScaleFactor: 1,
  mobile: false,
});
await record('home-title', `document.body.innerText.includes('今天想整理什么？')`);
await record('browser-mode-labelled', `document.body.innerText.includes('浏览器演示')`);
await screenshot('home-1440x900.png');

for (const [label, expected, file] of [
  ['全部记录', '全部记录', 'library-1440x900.png'],
  ['录音', '开始录音', 'recorder-1440x900.png'],
  ['AI 文档', 'AI 文档', 'documents-1440x900.png'],
  ['设置', 'AI 文档引擎', 'settings-1440x900.png'],
  ['首页', '今天想整理什么？', 'home-return-1440x900.png'],
]) {
  await clickByText(label);
  await record(`navigation-${label}`, `document.body.innerText.includes(${JSON.stringify(expected)})`);
  await screenshot(file);
}

await command('Input.dispatchKeyEvent', {
  type: 'keyDown',
  key: 'k',
  code: 'KeyK',
  modifiers: 2,
});
await command('Input.dispatchKeyEvent', {
  type: 'keyUp',
  key: 'k',
  code: 'KeyK',
  modifiers: 2,
});
await new Promise(resolve => setTimeout(resolve, 150));
await record('command-palette-opens', `document.querySelector('.command-palette') !== null`);
await screenshot('command-palette.png');
await command('Input.dispatchKeyEvent', { type: 'keyDown', key: 'Escape', code: 'Escape' });
await command('Input.dispatchKeyEvent', { type: 'keyUp', key: 'Escape', code: 'Escape' });

await command('Emulation.setDeviceMetricsOverride', {
  width: 900,
  height: 640,
  deviceScaleFactor: 1,
  mobile: false,
});
await new Promise(resolve => setTimeout(resolve, 150));
await record('minimum-width-no-horizontal-overflow', `document.documentElement.scrollWidth <= 900`);
await screenshot('home-900x640.png');

await command('Emulation.setDeviceMetricsOverride', {
  width: 768,
  height: 700,
  deviceScaleFactor: 1,
  mobile: false,
});
await new Promise(resolve => setTimeout(resolve, 150));
await record('compact-layout-message-or-fit', `document.body.innerText.includes('建议扩大窗口') || document.documentElement.scrollWidth <= 768`);
await screenshot('home-768x700.png');

await record(
  'all-buttons-have-labels',
  `[...document.querySelectorAll('button')].every(button => (button.getAttribute('aria-label') || button.title || button.textContent).trim().length > 0)`,
);
await record(
  'images-have-alt',
  `[...document.querySelectorAll('img')].every(image => image.hasAttribute('alt'))`,
);

console.log(JSON.stringify({
  checks,
  browserErrors: browserEvents.map(event => ({ method: event.method, params: event.params })),
  screenshots: await fs.readdir(outputDirectory),
}, null, 2));
socket.close();
