<!-- exact:begin (exact new writes this block; bun exact.mjs update rewrites it) -->
# kerb-racer: an Exact game

The world is Rust: `logic/src/lib.rs` implements `Game` (its `Options` are the
canvas's arguments, `setup` and `tick` its gameplay). Menus, the HUD and accessible
controls are `app.contract` (Contract). `app.json` is optional and holds only keys
you author (a title, `game.audio`, `game.assets`, a data crate). The bake generates
the hosts under `.shells/` (ignored; never edit it), and so does `bun exact.mjs test-rust`:
run that once before `cargo` in `render/` or a render example, whose workspace it is. The game uses the exact2 checkout
at `../../exact2` by path (`EXACT2` overrides it).

Look things up in this order before reading engine source:

- `bun exact.mjs game vocab <name>`: a component's fields and evaluated defaults, a `World` verb or a method, with its recipe (no name lists them all).
- ../../exact2/game/recipes/README.md: runnable recipes by task: first the gameplay kit every game needs (steering, schools, pickups, spawners, a shop), which the starter's critter, coins and shop use; then cameras, emitters, sounds, saves, art export.
- ../../exact2/game/names.md: names from three.js, Unity and Godot, and what they are here.
- ../../exact2/game/README.md, by section: "The programming model", "Proof pins", "The agent's interface", "Determinism — the contract", "Assets, animation and placed UI".
- ../../exact2/game/engine/README.md: the engine's reference (borrowing, registration, saves and assets).
- ../../exact2/docs/contract-for-agents.md: Contract, for the menus and HUD; ../../exact2/docs/contract-grammar.md for exact forms and events (keys at a game's canvas too).
- ../../exact2/docs/game-pitfalls.md: verified footguns in games, symptom → cause → fix (search it; ../../exact2/docs/agent-pitfalls.md holds Contract's).

Commands, from this directory (`exact.mjs` runs from any other too, and finds a file
you name where you are, else here):

| | |
|---|---|
| `bun exact.mjs <verb> --help`, `bun exact.mjs help` | a verb's flags, whether it stays in the foreground, and an example (`--help all`: every flag, its ports); the verbs |
| `bun exact.mjs test-rust` | the hostless Rust tests (`logic/tests`) and the determinism lints |
| `bun exact.mjs contract build app.contract --json` | compile; `[]` or every diagnostic with its range |
| `bun exact.mjs web` | the web dev loop, at the URL it prints (a game builds the wasm target) |
| `bun exact.mjs test web` | build the web game if needed, then run its `*.test.contract` (also `macos`, `ios`) |
| `bun exact.mjs agent web "tap play" "type world key ArrowRight for 800" "screenshot out.png"` | drive the game as a person would |
| `bun exact.mjs mac --run`, `bun exact.mjs ios --run` | build and launch natively |
| `bun exact.mjs linux`, then `test linux` or `agent linux …` | build the Linux host and drive it headless (any machine, no display); `bun exact.mjs android` builds the same host for Android (LLP 1107) |
| `bun exact.mjs windows --run` | build and launch the standalone Windows game; omit `--run` to package only |
| `bun exact.mjs prove`, then `bun proof.mjs web` | `prove` takes the first development baseline (seven legs); `prove --release` before you ship it. Existing pins are checked; `--repin` publishes intentional changes or release coverage. A warm continuous web leg is reused; Save and FreshGame compile their own wasm because the mode is compile-time. Hot-swap wasm is not reused |
| `bun exact.mjs update` | after exact2 moves; it rewrites `exact.mjs` and this block, and says when `Cargo.lock` needs `lock` |
| `bun exact.mjs lock` | when a build says exact2 moved since the game's `Cargo.lock` was captured, or after adding a dependency: fetches (the network), resolves again and prints what changed; builds never change the lock |
| `bun exact.mjs shells` | write `.shells/`, the generated workspace a crate here names, without a build (`cargo run --example` in `render/` needs it) |
| app.json `"commands": {"replay": ["bun", "tools/replay.mjs"]}` | the game's own verbs: `bun exact.mjs replay web` runs `bun tools/replay.mjs web` here |

The loop: `test-rust` while tuning gameplay (it makes a new `logic/tests/*.rs` a target),
`contract build --json` until it prints `[]`, `test web`, look at it with
`agent web … screenshot`, then the native hosts. A hostless test of a world with models
builds it with `exact_game::sim_with_art!(Options::default())`, which bakes `art/` in memory.
`proof.mjs` checks engine facts, then one block headed `// starter: replace these with
your game's checks`: replace that block. A proof script under `tools/` proves this game
too. Run `prove` once, last, when the game plays and `proof.mjs` describes it; it is the
release baseline, and the loop above never waits on it.
`bun ../../exact2/scripts/exact.mjs setup --check` names anything this machine is missing.

Generated, so don't edit: `exact.mjs` and this block.

The authoring diary: `bun exact.mjs feedback status` says whether this project keeps one.
<!-- exact:end -->

## Baked shadows (read before changing trees, fences or props)

Real-time sun shadows run at ~12 fps in Safari's WebGPU, so Medium and Low draw
baked ones: `art/textures/shade-{0,1,2}.png` is an alpha overlay painted from
the exact seeded positions of every pine, broadleaf, bush and fence section
(`art/src/casters-{id}.json`), and each car carries a blob (`blob.png`).
Real-time shadows are High only (`shadows: quality >= 2` in `light()`); keep it so.

The logic test `baked_shade_matches_the_woods` fails whenever setup places the
woods differently: a new tree model, a changed density, a different RNG draw
order in `scenery::props` or anything that runs before it in `setup`. Then:

```sh
cd .shells && KR_REBAKE=1 cargo test -p kerb-racers-logic --test sim baked && cd ..
blender -b -P art/src/shade.py          # repaints shade-*.png from the casters
bun exact.mjs test-rust                 # rebakes assets/, reruns every test
```

If a tree model's shape changes (height, crown radius), update its numbers in
`art/src/shade.py` (`CONIFERS`, the `tree-a` crown) so the painted shadow
matches. New casters (a barrier, say) need a `kind` in the `casters()` filter in
`logic/tests/sim.rs` and a branch in `shade.py`. If `bun exact.mjs test-rust`
says a generated `blob`/`shade` asset "collides with an authored asset" after a
merge, delete `assets/blob.*tex assets/shade-*.tex` and rerun: the bake recreates them.
