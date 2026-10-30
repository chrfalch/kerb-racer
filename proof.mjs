#!/usr/bin/env bun
// Kerb Racers on a real host: the menu, the countdown, the pedals reaching the
// world through the mouse buttons, a full race's HUD, and saves that continue
// identically in a fresh process. `bun proof.mjs web` adds pixels.
import {resolve} from 'node:path';
import {readFileSync} from 'node:fs';
import { proof, axNames } from "../exact2/game/proof.mjs";

const node = (tree, id) => tree.nodes.find(n => n.props?.testId === id);
const text = (tree, id) => node(tree, id)?.props?.text;
const speed2 = car => car.vx * car.vx + car.vz * car.vz;

if (import.meta.main) await proof(import.meta, async ({open, check, out, host, pin, pinSave}) => {
  const s = await open();
  if (process.argv.includes('--screenshot-only')) {
    check('screenshot uses web', host === 'web');
    await s.tap('play');
    await s.world('world').run(3500);
    await s.world('world').hold('KeyW', 2500);
    await s.screenshot(resolve(out, 'game.png'));
    await s.close();
    return;
  }
  const title = await s.tree();
  const ax = await axNames(s);
  check('Race is focused and named', node(title, 'play')?.focused === true && (ax.unavailable || ax.name('play') === 'Race'));
  check('three tracks offered', ['track-0', 'track-1', 'track-2'].every(id => !!node(title, id)));
  check('controls explained', title.nodes.some(n => n.props?.text === 'Right click: gas · Left click: brake'));
  check('world loads only after Race', !node(title, 'world'));
  if (host === 'web') await s.screenshot(resolve(out, 'menu.png'));
  await s.tap('play');
  const game = s.world('world');
  pin(0, await game.snapshot());
  check('countdown first, starting fifth', text(await s.tree(), 'hud-place') === '5/6' && !!node(await s.tree(), 'countdown'));
  const grid = await game.global_position('player');
  await game.hold('KeyW', 1000);
  check('the pedals wait for the lights', JSON.stringify(await game.global_position('player')) === JSON.stringify(grid));
  await game.run(2100);
  check('GO after three seconds', !!node(await s.tree(), 'go') && !node(await s.tree(), 'countdown'));
  await game.hold('KeyW', 2000);
  const moved = await game.global_position('player');
  const car = await game.get('player', 'Car');
  check('right click is gas', Math.hypot(moved[0] - grid[0], moved[2] - grid[2]) > 10 && speed2(car) > 200, {moved, car});
  check('speed reaches the HUD', Number(text(await s.tree(), 'hud-speed')) > 50);
  await game.hold('KeyS', 600);
  const braked = await game.get('player', 'Car');
  check('left click is brake', speed2(braked) < speed2(car), {braked});
  await game.run(400);
  if (host === 'web') await s.screenshot(resolve(out, 'game.png'));
  await game.save(resolve(out, 'final.world'));
  const checkpoint = await game.snapshot();
  await game.hold('KeyW', 1500);
  const continued = await game.snapshot();
  await game.save(resolve(out, 'continued.world'));
  pinSave('continuation', resolve(out, 'continued.world'));
  await s.tap('pause');
  check('pause offers restart and menu', !!node(await s.tree(), 'restart') && !!node(await s.tree(), 'quit'));
  await s.tap('restart');
  check('restart returns to the grid', !!node(await s.tree(), 'countdown'));
  await game.run(3500);
  // Pulse the pedals: mostly gas, a dab of brake, about 80 % of a lap's pace.
  for (let k = 0; k < 400 && (k % 8 || text(await s.tree(), 'result-title') === undefined); k++) {
    await game.hold('KeyW', 400);
    await game.hold('KeyS', 120);
  }
  const done = await s.tree();
  check('a careful race reaches the results', !!node(done, 'results') && /You (win|finished)/.test(text(done, 'result-title') ?? ''), text(done, 'result-title'));
  if (host === 'web') await s.screenshot(resolve(out, 'results.png'));
  await s.tap('again');
  check('Race again restarts on the grid', !!node(await s.tree(), 'countdown') && !node(await s.tree(), 'results'));
  await s.close();
  const restored = await open({fresh: true, world: resolve(out, 'final.world')});
  await restored.tap('play');
  const loaded = restored.world('world');
  check('fresh process restores snapshot', JSON.stringify(await loaded.snapshot()) === JSON.stringify(checkpoint));
  await loaded.hold('KeyW', 1500);
  check('fresh process continues identically', JSON.stringify(await loaded.snapshot()) === JSON.stringify(continued));
  await loaded.save(resolve(out, 'restored.world'));
  check('continuation saves are byte-identical', readFileSync(resolve(out, 'continued.world')).equals(readFileSync(resolve(out, 'restored.world'))));
  await restored.close();
});
