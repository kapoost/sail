# The browser, checked twice

The question keeps coming back, so here is the measurement rather than the opinion.
Re-run it before believing any of this: facts about a toolchain go stale.

## It compiles

```
cargo check --target wasm32-unknown-unknown -p sail-core   0 errors
                                            -p sail-net    0 errors
                                            -p sail-app    0 errors, 2 min 23 s
```

That contradicts the note this project carried from 13 October, which said there were
three hard blockers and it would not build. `std` on `wasm32-unknown-unknown` *has* the
APIs — threads, files, environment — and compiles calls to them happily. What they do is
fail at run time. So the work is not a fight with the build; it is replacing four things
that do not work in a tab.

## What broke when it ran, and what was done about it

Kept as a list rather than deleted: it is the shape of the work, and four of the five
turned out to be worth doing on a desk as well.

| | what happened | what was done |
|---|---|---|
| `sea_texture.rs` baked 36 ripple frames in `thread::scope` | `spawn` is unimplemented and **panics** | **done** — baked a few rows a frame with a four millisecond budget, on the task pool where there is one. The window opens a fifth of a second sooner natively too |
| `SystemTime::now()` seeding the day, `Instant::now()` timing two bakes | **panics** on this target, in `main`, before a window exists | **done** — `knob::seed` and the engine's own `Instant`, which knows where it is |
| fifty `env::var` calls (`SAIL_SEED`, `SAIL_WIND`, `SAIL_FRAMES`…) | all return `Err`, so every dial disappears exactly where there is no terminal to set one | **done** — one seam, `knob::var`: the environment on a desk, the query string in a tab |
| eight multisamples | WebGPU has one and four; every pass after the first was invalid | **done** — four in a tab, which at this drawn width is a boundary pixel either way |
| the hull unpacked with `fs::write` into a temp directory | returns an error, and the fallback path `"assets"` is taken | **nothing to do** — on the web `AssetServer` fetches over HTTP, so serving `assets/` is all it needs. Easier in a browser, not harder |
| the sky bakes a cubemap on `AsyncComputeTaskPool` | runs on the main thread | 0.086 s a bake here, and part of the twelve seconds of startup below |
| audio | needs a user gesture before anything sounds | **done** — the page starts her on a button |

## What settles it: sailing together

A browser has no UDP and cannot broadcast. Everything in `docs/online.md` that works
today — the relay, the list of games on the network, joining by address — is simply
unavailable in a tab. The replacements:

* **WebTransport.** Datagrams over QUIC, which is what our protocol already is: unreliable,
  unordered, newest-wins. The message types would not change at all. It costs TLS, a
  public hostname and a certificate, which means the host can no longer be somebody's
  laptop on the sofa.
* **WebRTC data channels.** Same transport properties, plus a signalling server.
* **WebSockets.** TCP, with head-of-line blocking — the one property the whole snapshot
  design exists to avoid.

And the list of games on the local network is gone for good either way: a web page cannot
find a server sitting next to it.

## What has changed since the browser looked attractive

The argument that nearly carried it in October was **trust**, not convenience: a
downloaded binary says *"Sail is damaged"* on a Mac and *"Windows protected your PC"* on a
PC, and a link in a browser says neither. Half of that is now answered by
`dist/install.sh`, which installs without going through a browser at all and therefore
without quarantine.

## The number that decided it is gone

The browser question used to be about what panics and what cannot find a UDP socket.
Those were the easy half. The hard half was one loop — 201 × 201 vertices each asking a
twenty-one component spectrum for height, slope and drift, once a frame, on the
processor. In a tab, scalar `f64` with a transcendental in the inner loop runs one and a
half to three times slower than native, so 5.7 ms became nine to seventeen of a 16.7 ms
frame: **the sea alone was the frame**.

That loop is not on the processor any more. It is `assets/sea_wave.wesl`, a vertex
shader, and the figures are measured on the same machine as the ones above:

| | native, M5 |
|---|---|
| sea | 0.04 ms light airs, 0.12 ms in a gale — 1.5 and 2.3 when it was spread across cores, 4.4 and 5.7 on one |
| sea, drawn | 0.12 ms — **7.0 ms** before |
| foam | 0.35 to 1.8 ms |
| cloud | 0.12 ms |

Two things follow for a tab, and they point opposite ways.

**The good one: WGSL is WGSL.** The sea's shader is already written in the language a
browser runs. WebGPU takes it as it stands; nothing in it is native-only — no 64-bit
anything (the noise hashes are built out of 32-bit arithmetic by hand, precisely because
WGSL has no `u64`), no threads, no files. The hard half of the port therefore did not get
easier: it stopped existing.

**The awkward one: what is left is the startup.** With the sea gone, the largest single
thing a tab would have to swallow is the ripple bake — thirty-six frames of normal map,
0.47 s across every core here, which in a single-threaded tab is six to fifteen seconds
of a page that appears to have hung. That is now *the* blocker rather than one of four,
and it is the same fix it always was: spread the bake across frames, which is worth doing
natively too.

### Does spreading the vertex loop across cores still matter in a tab?

It is moot for the sea and still true for everything else. The work done in October —
rewriting the loop as a pure function of a grid offset with no shared mutable state —
turned out to be the same shape a vertex shader wants, which is why the move to the card
was a translation rather than a redesign. Threads in wasm need `SharedArrayBuffer`, which
needs the `Cross-Origin-Opener-Policy` and `Cross-Origin-Embedder-Policy` headers, and
**GitHub Pages cannot send headers**.

## It runs in a tab

Measured rather than argued, on 20 October, in Chromium over WebGPU on the same M5,
with the same seed and the same weather as the native figures above:

| | native | in a tab |
|---|---|---|
| sea | 0.06 ms | **0.17 ms** |
| foam | 0.95 ms | 1.67 ms |
| cloud | 0.14 ms | 0.20 ms |
| frames a second | 60, capped | **48** |
| worst frame | 17.4 ms | 25 ms |
| the ripple film is complete | half a second of sailing | about six seconds |

Four things were in the way and none of them was the water.

1. **The clock panics.** `SystemTime::now()` on `wasm32-unknown-unknown` is not missing,
   it is a panic — and it was in `main`, seeding the day, before a window exists. Same
   for `Instant::now()` in the two places that time a bake. The engine ships a shim for
   exactly this (`bevy::platform::time::Instant`, `web-time` underneath); the seed comes
   off the browser's own clock now.
2. **The ripple bake panics.** Thirty-six frames of normal map were baked in
   `std::thread::scope`, and `spawn` is unimplemented in a tab. It is baked a few rows at
   a time now, with a four millisecond budget a frame, on the task pool where there is
   one — which is also why the game opens a fifth of a second sooner on a desk.
3. **Eight samples do not exist.** WebGPU has multisampling at one and four; the camera
   asked for eight and every pass after that was invalid. Four in a tab.
4. **The dials had nowhere to come from.** Fifty `SAIL_*` variables that a browser
   answers `Err` to, for ever. They go through `knob::var` now, which reads the query
   string in a tab: `?wind=32&seed=7&frames=1` is `SAIL_WIND=32 SAIL_SEED=7
   SAIL_FRAMES=1`. Without that there is no way to ask a tab for a gale, and no way to
   get the measurement in this table out of one.

**What is slow is no longer ours.** Of a 21 ms frame, the three systems this project
measures account for 2.1 ms. Halving the drawn width — 960 to 720 — does not move the
frame rate at all, so it is not the card and not fill rate; turning the ripple film off
buys two frames a second. The rest is the engine's own per-frame work, single-threaded,
because that is what wasm without `SharedArrayBuffer` gets: extract, queue, prepare and
the user interface all on the one thread that is also running the game.

So the thread argument has come back, and it has moved: it is no longer about our sea,
it is about everything the renderer does around it. That is also the argument that
hosting decides, and the same hosting constraint applies — a page served from GitHub
Pages gets one thread.

### Building it

```sh
dist/web.sh            # into dist/out/web
dist/web.sh --serve    # and then http://localhost:8777
```

Three things that script does which `cargo build` does not. It picks the **`web`
profile**, which is `dist` plus `strip` and `panic = "abort"` — the debug information is
most of a wasm module, and taking it out is 144 MB down to **60 MB, 14.6 MB over the
wire**, since every server worth the name gzips it. It asks for **WebGPU**, because the
sea reads its spectrum out of a storage buffer and WebGL2 has no such thing. And it puts
**`assets/` beside the page**, because on the web the asset server fetches them over
HTTP instead of unpacking them out of the binary — which is the one platform where this
game's fallback path was always the right answer.

It also checks that `wasm-bindgen-cli` matches the `wasm-bindgen` in `Cargo.lock` before
it builds anything, because the failure when they disagree is a stack trace in a browser
console an hour later. `wasm-opt` is used if it happens to be installed and nothing is
made of it if not.

### The page, and the three slow things behind it

`dist/web/index.html` is a title, the keys, a mainsail going up as the loading bar, and a
button. All four exist because of something that went wrong.

**The sail is the bar** because the thing being waited for is fifteen megabytes and a
compile, and a bar that fills in one jump says nothing: the head of the sail runs up the
mast with the download, creeps through the compile, and reaches the masthead when she is
drawing.

**The button is deliberate.** A browser will not open an audio context without a gesture,
so a game started without one is a game that sounds broken; "press to sail" buys the wind
and the water with it.

**The card does not come down until the canvas has a real size.** The first version added
its `sailing` class — which hides the page and shows the canvas — and *then* awaited the
engine, so from the moment anybody clicked, the only thing on screen was a black
rectangle. Reported from the other side of the room as *long, long, nothing*, with a
photograph of a black tab. A black rectangle is indistinguishable from a page that has
died.

**And the compile happens off the main thread.** Handing the bytes straight to `init`
compiles them synchronously: nothing moves, including the thing saying please wait.
`WebAssembly.compile` hands the module to a background thread and yields, so the sail
goes on rising while it works.

What that leaves, measured in Chromium over WebGPU with a warm cache:

| | |
|---|---|
| fetch, 14.6 MB over the wire | a few seconds, with the sail rising |
| compile, 60 MB of module | about four seconds, animated |
| **the engine's own start** | **about twelve seconds, frozen** |

The last row is the one left to attack, and it is the same fix the ripple film already
had: the textures, the sky and a ten megabyte hull are all baked or parsed *before* the
first frame, where on this machine they cost two and a half seconds and on one thread in
a tab they cost twelve. Baking across frames is what the film does now and what the rest
of them should do.

**The keyboard goes to whatever has focus, and that was the button.** The engine listens
for keys on the canvas, which is why winit gives it a `tabindex` — but a game started by
pressing a button leaves the focus on the button, and then on the body when the button
goes away, and the tiller does nothing. The page hands the canvas the focus as soon as
there is one to hand it to, and takes it back on any press in it.

The canvas carries its own aspect and the page scales it to fit, which is the same
decision the desk build makes with a window that cannot be resized: the console, the
chart and the gauges are drawn at known pixel sizes, so the picture is scaled rather than
re-laid-out.

### What is left to settle

* **The engine's frame, not ours.** Nineteen of twenty-one milliseconds. Whether that is
  reducible without threads is the next measurement, not the next assumption.
* **Twelve seconds of startup**, all of it work done before the first frame that could be
  done across the first hundred.
* **Somewhere to put it.** 60 MB is over GitHub Pages' 100 MB per-file limit only if it
  grows, but the repository would have to carry the module or the pages workflow would
  have to build it — six minutes of wasm compile added to a job that takes one.
* **The aspect.** The canvas follows the viewport, so a tall window gets a picture with
  the instruments nearer the edges than the desk build ever shows. Nothing is cut off;
  it is simply not a layout, by design.
* **Sailing together, never.** No UDP, no broadcast, no finding the game your friend is
  running in the next room. A tab is for one person.

## The shape that would make sense

A **single-player demo in a tab**, as a shop window: click a link, sail for five minutes,
install the real thing if you liked it. That is what is now at
[kapoost.github.io/sail/play](https://kapoost.github.io/sail/play/), published beside the
page it is described on by the same workflow — the game stays native, keeps its LAN play,
and nothing is maintained twice except a build target.

She starts there with the watch aboard and the pilot steering, on a day chosen for sea
room rather than drawn at random, because a first minute should be spent looking rather
than rounding up; `Esc → MISSIONS → SAILING 101` walks a reader round her one control at
a time. None of that is web-only — `SAIL_CREW=1 SAIL_PILOT=1` and `SAIL_LESSON=1` are the
same start on a desk — but a tab is the one place somebody arrives having read nothing.

**Phones are out of scope, on purpose and for now.** A current one has WebGPU — Chrome on
Android, Safari from iOS 18 — so the question is not whether it starts. It is that there
is no touch input in this game at all: the console is tappable, the tiller is not, and
looking round is a held right mouse button. Behind that are two measurements and a
guess: twelve seconds of frozen startup here becomes most of a minute on a phone's single
core, forty-eight frames a second becomes ten to fifteen, and sixty megabytes of module
on a four-gigabyte handset is where Safari starts killing tabs. The order of work, if it
is ever wanted, is touch controls first, the startup spread across frames second, a
lighter ripple film third. None of it is a flag; all of it is a day.

What would *not* make sense is porting the game and keeping sailing together, because the
second half of that sentence costs a certificate, a public host, a second transport and
the end of hosting from a laptop — all to reach a browser that still cannot find the game
your friend is running in the next room.
