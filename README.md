# Kerb Racers

## Building

Kerb Racers is an [Exact](https://github.com/ccheever/exact2) game. It needs an
exact2 checkout next to this one (`../exact2`), or set `EXACT2=/path/to/exact2`.
Run `bun ../exact2/scripts/exact.mjs setup --check` to list anything this machine
is missing (Rust toolchain, Bun, Xcode, Binaryen).

```sh
bun exact.mjs test-rust     # hostless Rust tests and determinism lints
bun exact.mjs web           # web dev loop, at the URL it prints
bun exact.mjs mac --run     # macOS app
bun exact.mjs ios --run     # iOS simulator
```

### iOS release build on a phone

The Rust library builds with the release profile. The app is signed with a
development identity and installed with `devicectl`.

Before the first build:

1. Install Xcode, then run `sudo xcodebuild -runFirstLaunch` once.
2. Sign in under Xcode → Settings → Accounts, so this Mac has an Apple
   Development certificate (`security find-identity -v -p codesigning` lists it).
3. Get a provisioning profile that covers the phone's UDID and the app's bundle
   id. To get one from Xcode, open any project with that bundle id once, with
   automatic signing turned on.
4. Connect the phone, unlock it, trust this Mac, and turn on Developer Mode
   (Settings → Privacy & Security). `xcrun devicectl list devices` should list it.

Then:

```sh
bun exact.mjs ios --device --run                       # build, install and launch
bun exact.mjs ios --device --run --phone "<name|udid>" # pick a phone when several are paired
EXACT_UPDATE_TRUST=production bun exact.mjs ios --device --run  # accept signed updates only
```

The identity and profile are chosen automatically. To choose them yourself, set
`EXACT_IDENTITY` (the certificate's SHA-1) and `EXACT_PROFILE` (a path to a
`.mobileprovision`).

To make an `.ipa` for distribution (ad hoc or TestFlight) instead of installing
on a connected phone:

```sh
EXACT_IDENTITY=<sha1> EXACT_PROFILE=<path.mobileprovision> \
  bun exact.mjs ios --device --archive out/KerbRacers.ipa
```

`--unsigned` doesn't work for this game. It has GPU modules, and re-signing the
app would invalidate their baked digests.

## Proofs

`bun proof.mjs` verifies gameplay, Contract text/accessibility and saves on the GPU-less Linux host. `bun proof.mjs web` adds pixels. First run `bun '../exact2/game/prove.mjs' .` to fill empty pins after every mode and host agrees; accept later intentional changes with `--repin`.

Rust hostless tests read simulation time with `sim.world().now()` and save with
`sim.save()`. JavaScript app proofs inspect the running host with
`await session.world('world').snapshot()`.

The simulation defaults to 120 Hz: the measured 60 Hz tick-wait p95 was about 17 ms. Choosing 60 Hz is an explicit latency/CPU trade, not the starter default.

The canvas binds `world(seed=7, paused=paused, restart=again)` by the names in
`Options`. Reorder these arguments freely; omitted fields use `Options::default()`.
`world()` takes every default; short positional calls take the remaining defaults.
Raw host JSON key order is not a hash input.

Capture the picture below with `bun proof.mjs web --screenshot-only` at `artifacts/web/game.png`.
This reports `UNVERIFIED` because it skips gameplay checks; `bun proof.mjs web` runs the full browser proof.
The grid, sky-colored height fog, sun shadows, beacon pads and bloom are starter defaults.
Victory uses the beacon count from setup; extending the Rust scene list needs no Contract edit.
Gameplay reloads preserve the running world. To apply edits inside `setup`, pause
and choose **Restart**; setup runs again and play resumes from the beginning.
`Glow(Tween)` on a `Material::glow` mesh is sampled by the renderer at frame time;
the tick only retargets the tween. The scene steps `Follow` after your tick.
`scene::follow(w)` remains optional when you need to choose an earlier ordering.

![Starter proof capture](artifacts/web/game.png)

One `nearest_xz_mut` result supplies both the lighting write and the HUD name;
`w.count::<Beacon>(|b| b.lit)` supplies the total. The proof checks W against
`d = a*m*(m+1)/(2*h*h) + (n-m)*v/h`, where `m=min(n,floor(v*h/a))`.
Character uses constant acceleration up to its speed limit, so this is the sum
of its semi-implicit velocity steps, with a 1 mm float tolerance. Hash pins still
check the exact saved simulation. Space and Enter on the focused Pause button
activate that button without reaching the world.

This game is the files you write: `logic/src/lib.rs`, `app.contract` and
`proof.mjs`, with `logic/tests/` for hostless tests and the tool-written
`pins.json`. Identity derives from the directory name and `Game::ID`; add an
`app.json` only for keys you author (a title, `game.audio`, a bundle id). The
bake generates everything else, ignored, under `.shells/`. Linux proofs use the
incremental `gpu-dev` profile; web, Apple and deploy retain their production
profiles. `EXACT_GAME_PROOF_PROFILE=release bun proof.mjs` checks Linux against
release without changing the game.

Run the Rust tests, from a fresh clone too, with
`bun '../exact2/game/app/shells.mjs' . --test`: it generates `.shells/`,
resolves against the SDK's lock offline and locked, and checks the SDK's
determinism lints (exact2's `game/README.md`, "Determinism — the contract").
To depend on another crate, write `logic/Cargo.toml` (`package.workspace =
"../.shells"`, SDK crates as `exact-game.workspace = true`), then capture
this game's own lock with `bun '../exact2/game/app/shells.mjs' . --update-lock`
and commit `Cargo.lock`.
