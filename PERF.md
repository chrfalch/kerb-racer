# Performance notes

Measure with `tools/bench.mjs` (build first with `bun exact.mjs web-build`):

```sh
bun tools/bench.mjs q0 q1 q2              # each quality in a fresh headful Chrome, vsync off
bun tools/bench.mjs q2 --profile          # plus main-thread self time
bun tools/bench.mjs q1 --vsync --shot out # what a player sees, and a screenshot
```

It reads the engine's own counters (`state.world.perf`: tick/feed/encode rings,
draws, triangles, culling, and per-pass GPU timestamps when Chrome grants them)
and the page's requestAnimationFrame cadence. Run it headful: headless Chrome on
macOS paces frames at 80–120 ms whatever the work. Uncapped is the default
because with vsync every quality sits at 60 fps on an M-series Mac and the
headroom is invisible. On Apple GPUs per-pass timestamps overlap (tile-based
deferred rendering), so attribute cost by switching a feature off and comparing
frame time, not by summing passes.

## Where the frame goes (Chrome, M4, 1280×800 @2x = 2560×1600, uncapped, ms)

The CPU side is small at every quality (tick ≈ 0.1, feed ≈ 0.15, encode ≈ 0.2–0.7);
the frame is GPU-bound and the main thread's "busy" time is Chrome's command
buffer blocking in submit/getCurrentTexture/setBindGroup while the GPU catches up.

| step | Medium | High |
|---|---|---|
| start (7ceefd0) | 9.2 | 14.0 |
| lens blur gathered at half resolution | 7.4 | 12.0 |
| exact2: shadow plane PCF via textureGather | 7.4 | 10.8 |
| exact2: SSAO upsample reads stored view z | 6.85 | 10.2 |
| RenderScale 1.5 (Medium) / 1.75 (High); Low 1.25 → 1.75 ms | 4.1 | 8.2 |

Feature costs measured by switching each off (at the start):
real-time shadows ≈ 4.4 (High; half of it the low sun's slope-corrected PCF),
lens ≈ 3.3, SSAO ≈ 3.0, bloom ≈ 1.9, undergrowth < 1 (noise).
Cascade layout barely matters (one cascade ≈ −0.3). SSAO `Low` ≈ −0.2.
Bloom is ≈ 1.45 of High's 8.2 after these: not its fetches (reading the bright
pass's 6×6 block once with gathers changed nothing) but most likely its eleven
small passes; fewer octaves would narrow the glow, so it is left alone.

## Render scale

`RenderScale { max_pixel_ratio }` (an exact2 resource, set in `light()`) caps
the canvas's device pixels per point: 1.25 / 1.5 / 1.75 for Low / Medium /
High. The browser stretches the picture; the HUD is DOM and stays at the
display's ratio. The Apple hosts ignore the cap for now.
