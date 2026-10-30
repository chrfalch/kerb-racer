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

Feature costs measured by switching each off (at the start):
real-time shadows ≈ 4.4 (High; half of it the low sun's slope-corrected PCF),
lens ≈ 3.3, SSAO ≈ 3.0, bloom ≈ 1.9, undergrowth < 1 (noise).
Cascade layout barely matters (one cascade ≈ −0.3). SSAO `Low` ≈ −0.2.
Rendering at devicePixelRatio 1.5 instead of 2: Medium 4.4, High 6.8.
