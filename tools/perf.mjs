#!/usr/bin/env bun
// A wall-clock performance probe for the running game, the way a player sees it:
//   bun tools/perf.mjs <url> [--headful] [--quality 0|1|2] [--dpr 2] [--seconds 6] [--size WxH] [--wait ms] [--shot out.png]
// Opens Chrome on the page, starts a race, then reports presented frames per
// second (requestAnimationFrame), the main thread's busy share, the GPU adapter
// and the hottest functions of a CPU profile.
import { spawn } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

const args = process.argv.slice(2);
const url = args.find(a => !a.startsWith('--') && !/^\d+$/.test(a)) ?? 'http://127.0.0.1:8833/';
const flag = (name, fallback) => { const i = args.indexOf(`--${name}`); return i >= 0 ? args[i + 1] : fallback; };
const seconds = Number(flag('seconds', 6));
const quality = flag('quality', null);
const dpr = Number(flag('dpr', 1));
const [width, height] = flag('size', '1280x800').split('x').map(Number);
const chrome = process.env.CHROME ?? '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const profile = mkdtempSync(join(tmpdir(), 'kr-perf-'));
const port = 9300 + Math.floor(Math.random() * 500);
const child = spawn(chrome, [
  ...(args.includes('--headful') ? [] : ['--headless=new']),
  `--remote-debugging-port=${port}`, `--user-data-dir=${profile}`, `--window-size=${width},${height}`,
  '--enable-unsafe-webgpu', '--no-first-run', '--no-default-browser-check', 'about:blank',
], { stdio: 'ignore' });
const sleep = ms => new Promise(r => setTimeout(r, ms));
let ws;
for (let i = 0; i < 50 && !ws; i++) {
  try {
    const pages = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
    const page = pages.find(p => p.type === 'page');
    if (page) ws = new WebSocket(page.webSocketDebuggerUrl);
  } catch { await sleep(200); }
}
await new Promise(r => ws.addEventListener('open', r, { once: true }));
let id = 0; const waiting = new Map();
ws.addEventListener('message', e => { const m = JSON.parse(e.data); if (m.id && waiting.has(m.id)) { waiting.get(m.id)(m); waiting.delete(m.id); } });
const send = (method, params = {}) => new Promise(r => { const n = ++id; waiting.set(n, r); ws.send(JSON.stringify({ id: n, method, params })); });
const evaluate = async (expression) => (await send('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true })).result?.result?.value;

if (dpr !== 1) await send('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: dpr, mobile: false });
await send('Page.enable');
await send('Page.navigate', { url });
const click = (testId) => evaluate(`(() => { const el = document.querySelector('[data-testid="${testId}"]'); if (!el) return false; el.click(); return true; })()`);
for (let i = 0; i < 100 && !(await click(quality != null ? `quality-${quality}` : 'play')); i++) await sleep(200);
if (quality != null) await click('play');
await sleep(Number(flag('wait', 4500))); // countdown, asset streaming, pipeline warm-up
const adapter = await evaluate(`navigator.gpu?.requestAdapter().then(a => a ? JSON.stringify({vendor: a.info?.vendor, arch: a.info?.architecture, desc: a.info?.description}) : 'none')`);
await send('Performance.enable');
const metric = async () => Object.fromEntries((await send('Performance.getMetrics')).result.metrics.map(m => [m.name, m.value]));
const before = await metric();
await send('Profiler.enable');
await send('Profiler.setSamplingInterval', { interval: 200 });
await send('Profiler.start');
const frames = await evaluate(`new Promise(done => { const t = []; const end = performance.now() + ${seconds * 1000};
  const f = now => { t.push(now); if (now < end) requestAnimationFrame(f); else done(t); }; requestAnimationFrame(f); })`);
const { result: { profile: cpu } } = await send('Profiler.stop');
const after = await metric();
const gaps = frames.slice(1).map((t, i) => t - frames[i]).sort((a, b) => a - b);
const q = p => gaps[Math.min(gaps.length - 1, Math.floor(p * gaps.length))].toFixed(1);
const wall = (frames.at(-1) - frames[0]) / 1000;
console.log(`url ${url}${quality != null ? ` quality ${quality}` : ''} ${width}x${height} dpr ${dpr} ${args.includes('--headful') ? 'headful' : 'headless'}`);
console.log(`gpu ${adapter}`);
console.log(`fps ${(gaps.length / wall).toFixed(1)} · frame p50 ${q(0.5)} ms p95 ${q(0.95)} ms max ${gaps.at(-1).toFixed(1)} ms`);
const d = k => (after[k] - before[k]);
console.log(`main thread busy ${(100 * d('TaskDuration') / wall).toFixed(0)}% (script ${(100 * d('ScriptDuration') / wall).toFixed(0)}%, layout ${(100 * d('LayoutDuration') / wall).toFixed(0)}%, style ${(100 * d('RecalcStyleDuration') / wall).toFixed(0)}%)`);
// Self time by function.
const self = new Map(); const byId = new Map(cpu.nodes.map(n => [n.id, n]));
const dt = cpu.timeDeltas; const total = dt.reduce((a, b) => a + b, 0);
cpu.samples.forEach((s, i) => { const n = byId.get(s); const f = n.callFrame; const key = `${f.functionName || '(anon)'} ${f.url.split('/').pop()}`; self.set(key, (self.get(key) ?? 0) + dt[i]); });
console.log('hottest (self time):');
for (const [k, v] of [...self].sort((a, b) => b[1] - a[1]).slice(0, 14)) console.log(`  ${(100 * v / total).toFixed(1).padStart(5)}%  ${k}`);
const shot = flag('shot', null);
if (shot) await Bun.write(shot, Buffer.from((await send('Page.captureScreenshot')).result.data, 'base64'));
ws.close(); child.kill(); await sleep(300); rmSync(profile, { recursive: true, force: true });
