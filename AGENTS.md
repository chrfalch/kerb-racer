<!-- exact:begin (exact new writes this block; bun exact.mjs update rewrites it) -->
# kerb-racers: an Exact game

The world is Rust: `logic/src/lib.rs` implements `Game` (its `Options` are the
canvas's arguments, `setup` and `tick` its gameplay). Menus, the HUD and accessible
controls are `app.contract` (Contract). `app.json` is optional and holds only keys
you author (a title, `game.audio`, `game.assets`, a data crate). The bake generates
the hosts under `.shells/` (ignored; never edit it). The game uses the exact2 checkout
at `../exact2` by path (`EXACT2` overrides it).

Read before writing code:

- ../exact2/game/README.md: the game target — the programming model, input, saves, determinism, the proof.
- ../exact2/game/engine/README.md: the engine's API.
- ../exact2/docs/contract-for-agents.md: Contract, for the menus and HUD.
- ../exact2/docs/agent-pitfalls.md: verified footguns, symptom → cause → fix.
- ../exact2/docs/contract-grammar.md: exact forms, built-in functions, events (keys at a game's canvas too).

Commands, from this directory:

| | |
|---|---|
| `bun exact.mjs test-rust` | the hostless Rust tests (`logic/tests`) and the determinism lints |
| `bun exact.mjs contract build app.contract --json` | compile; `[]` or every diagnostic with its range |
| `bun exact.mjs web` | the web dev loop, at the URL it prints (a game builds the wasm target) |
| `bun exact.mjs test web` | build the web game if needed, then run `app.test.contract` (also `macos`, `ios`) |
| `bun exact.mjs agent web "tap play" "type world key ArrowRight for 800" "screenshot out.png"` | drive the game as a person would |
| `bun exact.mjs mac --run`, `bun exact.mjs ios --run` | build and launch natively |
| `bun exact.mjs prove`, then `bun proof.mjs web` | the proof: a first baseline in `pins.json`, then real-host checks against it |
| `bun exact.mjs update` | after exact2 moves; it rewrites `exact.mjs` and this block |
| app.json `"commands": {"replay": ["bun", "tools/replay.mjs"]}` | the game's own verbs: `bun exact.mjs replay web` runs `bun tools/replay.mjs web` here |

The loop: `test-rust` while tuning gameplay, `contract build --json` until it prints
`[]`, `test web`, look at it with `agent web … screenshot`, then the native hosts.
`proof.mjs` asserts the starter's gameplay (its beacons, KeyE): rewrite it when you
change `Options` or the scene, before the first `prove`.
`bun ../exact2/scripts/exact.mjs setup --check` names anything this machine is missing.

Generated, so don't edit: `exact.mjs` and this block.

## The authoring diary

For the agent working on an Exact app. The exact2 team asks you to keep a diary of
how Exact went, so it can fix what was rough and keep what was good. The diary
stays on this machine. Nothing is sent unless the person you work for says yes.

Run `bun exact.mjs feedback status` once at the start. It prints this project's
standing answer first:

- `ask`, the default: keep the diary, and ask before sending (Asking to share).
- `local`: keep the diary and skip Asking to share; nothing is sent.
- `always`: keep the diary, and send it at the end of each task without asking,
  saying each time that you sent it.
- `never`: skip everything below: no diary, no asking.

If `feedback status` prints more instructions, follow them too.

### What to keep

Keep the diary at `.exact/diary/<YYYY-MM-DD>-<topic>.md` (`.exact/` is gitignored).
Write as you go, not from memory at the end, for a reader on the Exact team who has
never seen this app. Start it with:

```
surfaces: web, ios, ...
agent: <your product and model>
task: <one generic line, e.g. "add a settings screen"; never the business purpose>
```

Then four sections:

#### Rough
Anything that cost time or needed a workaround. For each: what you tried, what
happened (the error text, if short), roughly how long it cost, what you did
instead, and what would have helped (a better message, a doc, a missing feature).

#### Lean in
What was unusually pleasant or powerful, and that Exact should do more of.

#### Needed
One line for each platform capability the app needed, whether or not Exact had it,
so the team can see which ones many apps end up building by hand:

```
- camera: by hand (~40 min; a Swift bridge, getUserMedia on web)
- notifications: missing (skipped; would have needed APNs)
- haptics: provided
- share sheet: provided, rough (see Rough)
```

Start each line with a short generic name (camera, photos, notifications, location,
maps, purchases, sign-in, share sheet, haptics, audio, files, background work,
widgets, deep links, …), then one of `provided` (Exact had it), `by hand` (you built
it or worked around it) or `missing` (it was left out or faked). Name the capability,
never the feature built on it: "camera", not "scan receipts". Write `none` if the
app needed nothing beyond views and data.

#### Checkpoints
One line for each step you reached: `smooth`, or `rough` with a pointer to its
Rough entry. Leave out steps you didn't reach.

- install / first build
- `exact new`
- web dev loop
- editing app.contract / app.ts
- iOS build
- macOS build
- driving the app (`exact.mjs agent`, `exact.mjs test`)
- deploy

Write `smooth` whenever a step went smoothly, even with nothing else to say. That is
how the team tells that something that used to be rough got better. (`exact.mjs`
already logs each command's exit code and duration; the checkpoints say what it
felt like.)

### What not to write

- Secrets, tokens, keys, `.env` values, or credentials of any kind
- Personal or customer data, or anything the app handles at runtime
- The app's own source, beyond the few lines a reproduction needs
- The app's name or what it is for, unless the person has said it's public. Write
  "a todo app" instead.

### Asking to share

Never send on your own initiative. At a natural stopping point (the end of a task,
not the middle of one), ask once, quoting the diary:

> I kept a short diary of how Exact went this session. For example, it notes:
> - The iOS build failed until I ran `--update-lock` (about 10 minutes lost)
> - `line-clamp` rendered blank in the first rows on iOS
> - The web dev loop picked up contract edits instantly, which was great
>
> Want me to send it to the Exact team? I try to keep it generic and anonymous, but
> don't send anything if this is a secret project you don't want anyone to know
> anything about. Yes / no / always for this project / never.

The three bullets above are an example. Write 2–4 of your own from this session's
diary: mostly Rough entries and anything Needed `by hand` or `missing`, plus one
Lean in entry if there is one, each a single plain line. Leave out any bullet that would reveal the app's code, data, or name. If
nothing was rough, say so ("nothing broke; it mostly records what went smoothly")
instead of padding the list. If the person wants to read or change the diary first,
show it (`bun exact.mjs feedback` prints exactly what would be sent) and make their
edits.

Then act on their answer:

- **Yes:** `bun exact.mjs feedback send --yes`. Tell them it was sent, and give them
  the receipt it prints. `bun exact.mjs feedback delete <receipt>` deletes it.
- **No:** send nothing, and don't ask again this session.
- **Always:** `bun exact.mjs feedback always`, then send. From then on, for this
  project only, send at the end of each task without asking, and say each time
  that you sent it.
- **Never:** `bun exact.mjs feedback never`. Keep no diary and don't ask again for
  this project.

`send` adds the command log, replaces the home directory, the app's directory, name
and bundle id with placeholders, and blanks anything that looks like a credential.
That is a safety net, not a licence: write the diary as if none of it ran.
<!-- exact:end -->
