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

## What breaks when it runs

| | what happens | what it would take |
|---|---|---|
| `sea_texture.rs` bakes 36 ripple frames in `thread::scope` | `spawn` is unsupported and **panics** — the one hard blocker | spread the bake across frames, which is worth doing natively too: it is 0.47 s on all cores, so 4–6 s on one, and 6–15 s of frozen tab in wasm |
| the hull is unpacked with `fs::write` into a temp directory | returns an error, and the fallback path `"assets"` is taken | **nothing** — on the web `AssetServer` fetches over HTTP, so serving `assets/` is all it needs. This is *easier* in a browser, not harder |
| twenty `env::var` calls (`SAIL_SEED`, `SAIL_JOIN`, `SAIL_FLEET`…) | all return `Err`, so every diagnostic knob disappears | read them from the query string instead; mechanical |
| the sky bakes a cubemap on `AsyncComputeTaskPool` | runs on the main thread | 0.11 s of stall per bake, worse in wasm; batch it or accept it |
| audio | needs a user gesture before anything sounds | a click-to-start screen, which every web game has |

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

### Seeing it for yourself

```sh
cargo build --release --target wasm32-unknown-unknown -p sail-app --features webgpu
wasm-bindgen --out-dir /tmp/sailweb --target web --no-typescript \
  target/wasm32-unknown-unknown/release/sail-app.wasm
cp -R assets /tmp/sailweb/           # the hull and the sea's shaders, fetched over HTTP
# plus an index.html that imports ./sail-app.js and calls init()
cd /tmp/sailweb && python3 -m http.server 8777
```

Then `http://localhost:8777/?seed=7&wind=18&sea=built&frames=1`. The wasm is 144 MB
unoptimised and nobody has put `wasm-opt` near it yet; over a local socket that does not
matter and over a wire it would be the first thing to fix.

Two things this does **not** yet do: it has no click-to-start, so the audio context never
opens, and it has no loading screen, so the first seconds are a blank page while 144 MB
of wasm compiles.

### What is left to settle

* **The engine's frame, not ours.** Nineteen of twenty-one milliseconds. Whether that is
  reducible without threads is the next measurement, not the next assumption.
* **Size.** 144 MB of wasm before `wasm-opt`, 370 MB resident, 50 MB of it the ripple
  film. A tab will take it and would rather not.
* **A loading screen and a click to start.** Both are a page's manners rather than a
  port.

## The shape that would make sense

A **single-player demo in a tab**, as a shop window: click a link, sail for five minutes,
install the real thing if you liked it. It needs the ripple bake spread across frames,
the knobs moved to the query string, and a click-to-start. The game stays native, keeps
its LAN play, and nothing has to be maintained twice except a build target.

What would *not* make sense is porting the game and keeping sailing together, because the
second half of that sentence costs a certificate, a public host, a second transport and
the end of hosting from a laptop — all to reach a browser that still cannot find the game
your friend is running in the next room.
