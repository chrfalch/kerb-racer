# Performance notes

Measure with `tools/bench.mjs` (build first with `bun exact.mjs web-build`):

```sh
bun tools/bench.mjs q0 q1 q2              # each quality in a fresh headful Chrome, vsync off
bun tools/bench.mjs q2 --profile          # plus main-thread self time
bun tools/bench.mjs q1 --vsync --shot out # what a player sees, and a screenshot
bun tools/bench.mjs q2,t1 --json out.json # another track; results as JSON
```

It reads the engine's own counters (`state.world.perf`: tick/feed/encode rings,
draws, triangles, culling, and per-pass GPU timestamps when Chrome grants them)
and the page's requestAnimationFrame cadence. It prefers Playwright's Chrome for
Testing when installed (`CHROME` overrides).

- Run it headful: headless Chrome on macOS paces frames at 80–120 ms whatever the work.
- Uncapped is the default: with vsync every quality sits at 60 fps on an
  M-series Mac and the headroom is invisible.
- On Apple GPUs per-pass timestamps overlap (tile-based rendering), so attribute
  cost by switching a feature off and comparing frame time, not by summing passes.

## Where the frame goes in the browser

Measured on 7ceefd0 (before the exact2 main update), Chrome, M4, 1280×800 @2x =
2560×1600, uncapped. The native iPhone build does not need any of this; these
are browser numbers.

| quality | frame |
|---|---|
| Low | 3.7 ms |
| Medium | 9.2 ms |
| High | 14.0 ms |

The CPU side is small at every quality (tick ≈ 0.1, feed ≈ 0.15, encode ≈ 0.2–0.7 ms);
the frame is GPU-bound. The main thread's "busy" time is Chrome's command buffer
blocking in submit/getCurrentTexture/setBindGroup while the GPU catches up.

Feature costs, by switching each off:

| feature | cost |
|---|---|
| real-time sun shadows (High) | ≈ 4.4 ms; about half is the low sun's slope-corrected PCF on the ground |
| tilt-shift lens (20 full-res taps) | ≈ 3.3 ms; gathering the blur at half resolution brought it to ≈ 1.5 |
| SSAO | ≈ 3.0 ms; `AoQuality::Low` saves only ≈ 0.2 |
| bloom (High) | ≈ 1.9 ms; its eleven small passes, not its fetches |
| undergrowth | < 1 ms (noise) |

Shadow cascade layout barely matters (one cascade ≈ −0.3 ms). Frame cost scales
with pixels: at devicePixelRatio 1.5 instead of 2, Medium ≈ 4.4 and High ≈ 6.8 ms.
