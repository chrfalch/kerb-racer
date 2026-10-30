#!/usr/bin/env bun
// Where a frame's time goes, per graphics quality, from the engine's own
// counters (state.world.perf: CPU tick/feed/encode rings, GPU timestamps per
// pass, draws, triangles, culling) plus the page's requestAnimationFrame cadence.
//
//   bun tools/bench.mjs [q0 q1 q2 …] [--seconds 8] [--headless] [--vsync] [--dpr 2] [--size 1280x800]
//                       [--dist target/web-dist] [--json out.json] [--shot prefix] [--profile] [--raw]
//
// A scenario is `q<quality>` optionally followed by `,t<track>` (q2,t1). Each
// runs in a fresh Chrome: pick the quality, press Race, hold the gas (the
// player drives the racing line), wait out the countdown and pipeline warm-up,
// empty the rings, then sample for --seconds. Headless Chrome on macOS paces
// frames at 80-120 ms whatever the work; run headful (the default) for numbers. Build first: `bun exact.mjs web-build`.
import { spawn } from 'node:child_process';
import { mkdtempSync, rmSync, existsSync } from 'node:fs';
import { tmpdir, loadavg } from 'node:os';
import { join, resolve, extname } from 'node:path';

const args = process.argv.slice(2);
const flag = (name, fallback) => { const i = args.indexOf(`--${name}`); return i >= 0 ? args[i + 1] : fallback; };
const valued = new Set(['seconds', 'dpr', 'size', 'dist', 'json', 'warm', 'shot']);
const scenarios = args.filter((a, i) => !a.startsWith('--') && !valued.has(args[i - 1]?.slice(2)));
if (!scenarios.length) scenarios.push('q0', 'q1', 'q2');
const seconds = Number(flag('seconds', 8));
const warm = Number(flag('warm', 5000)); // countdown (3 s) and pipeline compiles
const dpr = Number(flag('dpr', 2));
const [width, height] = flag('size', '1280x800').split('x').map(Number);
const headless = args.includes('--headless');
// Uncapped by default: with vsync on, every quality on an M-series Mac sits at
// 60 fps and the headroom is invisible. --vsync measures what a player sees.
const vsync = args.includes('--vsync');
const dist = resolve(import.meta.dir, '..', flag('dist', 'target/web-dist'));
if (!existsSync(join(dist, 'index.html'))) { console.error(`no build at ${dist}: run bun exact.mjs web-build`); process.exit(2); }
// Chrome for Testing (Playwright's) first: the installed Chrome 154 dies at launch
// on the agent machine (FATAL path_service 1001). CHROME overrides both.
const CFT = join(process.env.HOME, 'Library/Caches/ms-playwright/chromium-1208/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing');
const CHROME = process.env.CHROME ?? (existsSync(CFT) ? CFT : '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome');
const sleep = ms => new Promise(r => setTimeout(r, ms));

const types = { '.html': 'text/html', '.js': 'text/javascript', '.json': 'application/json', '.wasm': 'application/wasm' };
const server = Bun.serve({ port: 0, async fetch(req) {
  const path = resolve(dist, '.' + decodeURIComponent(new URL(req.url).pathname.replace(/\/$/, '/index.html')));
  if (!path.startsWith(dist + '/')) return new Response('forbidden', { status: 403 });
  const file = Bun.file(path);
  if (!(await file.exists())) return new Response('not found', { status: 404 });
  return new Response(file, { headers: { 'content-type': types[extname(path)] ?? 'application/octet-stream',
    'Cross-Origin-Opener-Policy': 'same-origin', 'Cross-Origin-Embedder-Policy': 'require-corp' } });
} });

async function cdp(url) {
  const ws = new WebSocket(url);
  await new Promise((ok, fail) => { ws.addEventListener('open', ok, { once: true }); ws.addEventListener('error', fail, { once: true }); });
  let id = 0; const waiting = new Map();
  ws.addEventListener('message', e => { const m = JSON.parse(e.data); if (m.id && waiting.has(m.id)) { waiting.get(m.id)(m); waiting.delete(m.id); } });
  const call = (method, params = {}) => new Promise((ok, fail) => {
    const n = ++id; const t = setTimeout(() => { waiting.delete(n); fail(new Error(`${method} timed out`)); }, 30000);
    waiting.set(n, m => { clearTimeout(t); m.error ? fail(new Error(m.error.message)) : ok(m.result); });
    ws.send(JSON.stringify({ id: n, method, params }));
  });
  const evaluate = async expression => {
    const r = await call('Runtime.evaluate', { expression, awaitPromise: true, returnByValue: true });
    if (r.exceptionDetails) throw new Error(r.exceptionDetails.exception?.description ?? r.exceptionDetails.text);
    return r.result.value;
  };
  return { call, evaluate, close: () => ws.close() };
}

const STATE = `((reset)=>{const el=document.querySelector('[data-testid="world"]');const id=Number(el?.dataset.view);
  return globalThis.exact?.gpu?.agent(id,{op:'state',perf:true,...(reset?{perf_reset:true}:{})});})`;

async function run(spec) {
  const quality = Number(/q(\d)/.exec(spec)?.[1] ?? 1);
  const track = Number(/t(\d)/.exec(spec)?.[1] ?? 0);
  const profile = mkdtempSync(join(tmpdir(), 'kr-bench-'));
  const chrome = spawn(CHROME, [`--user-data-dir=${profile}`, '--no-first-run', '--no-default-browser-check',
    `--window-size=${width},${height}`, '--remote-debugging-port=0', ...(headless ? ['--headless=new'] : []),
    '--enable-unsafe-webgpu', '--disable-background-timer-throttling', '--disable-renderer-backgrounding',
    '--disable-backgrounding-occluded-windows',
    ...(vsync ? [] : ['--disable-gpu-vsync', '--disable-frame-rate-limit']), `http://localhost:${server.port}/`], { stdio: 'ignore' });
  let client;
  try {
    const deadline = Date.now() + 60000;
    while (!client && Date.now() < deadline) {
      try {
        const port = (await Bun.file(join(profile, 'DevToolsActivePort')).text()).split('\n')[0];
        const page = (await (await fetch(`http://localhost:${port}/json/list`)).json()).find(p => p.type === 'page');
        if (page) client = await cdp(page.webSocketDebuggerUrl);
      } catch {}
      if (!client) await sleep(100);
    }
    if (!client) throw new Error('no CDP page');
    await client.call('Emulation.setDeviceMetricsOverride', { width, height, deviceScaleFactor: dpr, mobile: false });
    const click = async id => {
      let box;
      for (let i = 0; i < 300 && !box; i++) {
        box = await client.evaluate(`(()=>{const e=document.querySelector('[data-testid="${id}"]');if(!e)return null;const r=e.getBoundingClientRect();return r.width?[r.x+r.width/2,r.y+r.height/2]:null})()`).catch(() => null);
        if (!box) await sleep(100);
      }
      if (!box) throw new Error(`no ${id}`);
      for (const type of ['mouseMoved', 'mousePressed', 'mouseReleased'])
        await client.call('Input.dispatchMouseEvent', { type, x: box[0], y: box[1], button: 'left', clickCount: 1 });
      await sleep(250);
    };
    await click(`track-${track}`);
    await click(`quality-${quality}`);
    await click('play');
    let s;
    while (Date.now() < deadline + 60000) {
      s = await client.evaluate(`${STATE}(false)`).catch(() => null);
      if (s?.world?.perf?.frameMs?.count > 0) break;
      await sleep(100);
    }
    if (!s?.world?.perf) throw new Error(`world never drew: ${JSON.stringify(s)?.slice(0, 300)}`);
    const key = type => client.call('Input.dispatchKeyEvent', { type, key: 'ArrowUp', code: 'ArrowUp', windowsVirtualKeyCode: 38 });
    await key('keyDown');
    await sleep(warm);
    await client.evaluate(`${STATE}(true)`);
    const profiling = args.includes('--profile');
    if (profiling) {
      await client.call('Profiler.enable');
      await client.call('Profiler.setSamplingInterval', { interval: 200 });
      await client.call('Profiler.start');
    }
    const gaps = await client.evaluate(`new Promise(done => { const t = []; const end = performance.now() + ${seconds * 1000};
      const f = now => { t.push(now); if (now < end) requestAnimationFrame(f); else done(t.slice(1).map((x, i) => x - t[i])); };
      requestAnimationFrame(f); })`);
    const cpu = profiling ? (await client.call('Profiler.stop')).profile : null;
    s = await client.evaluate(`${STATE}(false)`);
    const shot = flag('shot', null);
    if (shot) await Bun.write(`${shot}-${spec}.png`, Buffer.from((await client.call('Page.captureScreenshot')).data, 'base64'));
    const hud = await client.evaluate(`document.querySelector('[data-testid="hud-speed"]')?.textContent ?? null`);
    await key('keyUp');
    const adapter = await client.evaluate(`navigator.gpu?.requestAdapter().then(a => a ? [a.info?.vendor, a.info?.architecture, a.info?.description].filter(Boolean).join(' ') : 'none')`);
    return { spec, quality, track, adapter, speed: hud, gaps, world: s.world, cpu };
  } finally {
    client?.close();
    chrome.kill();
    await sleep(500);
    rmSync(profile, { recursive: true, force: true });
  }
}

const sorted = a => [...a].sort((x, y) => x - y);
const pct = (a, p) => a.length ? a[Math.min(a.length - 1, Math.floor(p * a.length))] : NaN;
const f = (x, d = 2) => x == null || Number.isNaN(x) ? '—' : Number(x).toFixed(d);
const ring = r => r ? `p50 ${f(r.p50)} p95 ${f(r.p95)} max ${f(r.max)}` : '—';

const results = [];
try {
  for (const spec of scenarios) {
    let r;
    try { r = await run(spec); } catch (e) { console.log(`${spec}: ${e.message}`); continue; }
    results.push(r);
    const p = r.world.perf, g = sorted(r.gaps);
    console.log(`\n=== ${spec}  quality ${r.quality} track ${r.track}  ${width}x${height}@${dpr}x ${headless ? 'headless' : 'headful'} ${vsync ? 'vsync' : 'uncapped'}  load ${loadavg()[0].toFixed(1)}`);
    console.log(`gpu ${r.adapter}   entities ${r.world.entities ?? '—'}   speed ${r.speed}`);
    console.log(`rAF   ${(g.length / seconds).toFixed(1)} fps   p50 ${f(pct(g, .5))} p95 ${f(pct(g, .95))} max ${f(g.at(-1))} ms   >20ms ${g.filter(x => x > 20).length}`);
    console.log(`frame ${ring(p.frameMs)}`);
    console.log(`cpu   tick ${ring(p.tickMs)} | feed ${ring(p.feedMs)} | encode ${ring(p.encodeMs)} | ticks/frame ${ring(p.ticksPerFrame)}`);
    console.log(`work  draws ${JSON.stringify(p.draws)} instances ${JSON.stringify(p.instances)} triangles ${JSON.stringify(p.triangles)}`);
    if (p.culled) console.log(`culled ${JSON.stringify(p.culled)}`);
    const gpu = r.world.gpuMs ?? {};
    const rows = Object.entries(gpu).filter(([, v]) => v && (v.count ?? 1) > 0).sort((a, b) => (b[1].p50 ?? 0) - (a[1].p50 ?? 0));
    if (rows.length) {
      console.log('gpu passes (ms, intervals overlap; do not sum):');
      for (const [name, v] of rows) console.log(`  ${name.padEnd(36)} p50 ${f(v.p50, 3).padStart(7)}  p95 ${f(v.p95, 3).padStart(7)}  max ${f(v.max, 3).padStart(7)}`);
    } else console.log('gpu passes: none (no timestamp-query on this adapter?)');
    if (r.cpu) {
      // Main-thread self time by function over the sample window.
      const self = new Map(), byId = new Map(r.cpu.nodes.map(n => [n.id, n]));
      const total = r.cpu.timeDeltas.reduce((a, b) => a + b, 0);
      r.cpu.samples.forEach((id, i) => { const c = byId.get(id).callFrame;
        const k = `${c.functionName || '(anon)'} ${c.url.split('/').pop()}${c.lineNumber >= 0 ? ':' + c.lineNumber : ''}`;
        self.set(k, (self.get(k) ?? 0) + r.cpu.timeDeltas[i]); });
      console.log('main thread self time (% of window):');
      for (const [k, v] of [...self].sort((a, b) => b[1] - a[1]).slice(0, 18)) console.log(`  ${(100 * v / total).toFixed(1).padStart(5)}%  ${k}`);
    }
    if (args.includes('--raw')) console.log(JSON.stringify(r.world.perf, null, 1));
  }
} finally { server.stop(true); }
const out = flag('json', null);
if (out) await Bun.write(out, JSON.stringify(results.map(({ gaps, cpu, ...r }) => ({ ...r, raf: { fps: gaps.length / seconds, p50: pct(sorted(gaps), .5), p95: pct(sorted(gaps), .95) } })), null, 1));
