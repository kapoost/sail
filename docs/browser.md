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

## The number that decides it, measured since

The browser question used to be about what panics and what cannot find a UDP socket.
Those are the easy half. The hard half is one loop:

| | native, M5 |
|---|---|
| sea mesh | 1.5 ms light airs, 2.3 ms in a gale — 4.4 and 5.7 before it was spread across cores |
| foam | 0.3 to 1.9 ms |
| cloud | 0.1 ms |

The frame is 16.7 ms at sixty. In a tab, scalar `f64` with a transcendental in the inner
loop runs somewhere between one and a half and three times slower than native — and a tab
without the headers below gets one thread, which is the 5.7 ms figure and not the 2.3.
Nine to seventeen milliseconds, then: **the sea alone is the frame**. Nothing else on the list is
close: the world is rendered at 960 and blown up, so the graphics card has almost nothing
to do, and every texture in the game is generated at startup rather than downloaded,
which is the one place a tab normally loses.

So the browser does not hinge on wgpu, or on assets, or on WebGPU support. It hinges on
201 × 201 vertices each asking a twenty-one component spectrum for height, slope and
drift, once a frame, on one thread.

### Does spreading it across cores help in a tab?

**In shape, yes — it is done. In effect in a tab, only with the right hosting.**

The loop is a function from a grid offset to a finished vertex now, handed to a task
pool: 5.7 ms to 2.3 natively. That is exactly the shape wasm threads want. Threads in wasm need
`SharedArrayBuffer`, which needs the `Cross-Origin-Opener-Policy` and
`Cross-Origin-Embedder-Policy` headers, and **GitHub Pages cannot send headers** — so a
parallel build served from where this project's page lives would run the single-threaded
path and gain nothing. A host that can set headers (or the service-worker trick that
fakes them) unlocks it; that is a hosting decision, not a code one.

What the work buys regardless is the shape: a pure map over the vertex array with no
shared mutable state and a read-only spectrum is exactly what both wasm threads and a
vertex shader want. Doing it for the native frame rate leaves the loop in the form the
browser would need.

### What would actually settle it

* **Put the sum in the vertex shader.** The sea becomes nearly free and the tab stops
  being the question — at the cost of the property that makes this sea trustworthy: the
  hull and the drawn surface asking the same water the same question. Survivable, because
  the hull samples a handful of points a substep rather than forty thousand, so the
  processor keeps doing *those* — but the WGSL would have to be generated from the same
  Rust spectrum, with a test pinning the two together at sample points. That is the real
  cost: not the shader, the agreement.
* **Fewer vertices.** 201 to 129 is two and a half times less work, 5.7 ms to 2.4, at
  the price of near-field detail the band limit already grades away with distance.
* **Trim the memory.** 370 MB resident, 50 MB of it thirty-six key frames of ripple map.
  A tab will take it and would rather not.

## The shape that would make sense

A **single-player demo in a tab**, as a shop window: click a link, sail for five minutes,
install the real thing if you liked it. It needs the ripple bake spread across frames,
the knobs moved to the query string, and a click-to-start. The game stays native, keeps
its LAN play, and nothing has to be maintained twice except a build target.

What would *not* make sense is porting the game and keeping sailing together, because the
second half of that sentence costs a certificate, a public host, a second transport and
the end of hosting from a laptop — all to reach a browser that still cannot find the game
your friend is running in the next room.
