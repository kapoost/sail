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
**GitHub Pages cannot send headers**; that still decides whether the *foam* and the
*ripple bake* get more than one core in a tab.

### What is left to settle

* **The ripple bake across frames.** Six to fifteen seconds of frozen page, and the one
  blocker that is genuinely in the way now.
* **Fewer vertices, if a weak card needs it.** 201 to 129 is two and a half times less
  work. It was a processor economy before and it is a card economy now, which is a much
  cheaper thing to need.
* **Trim the memory.** 370 MB resident, 50 MB of it thirty-six key frames of ripple map.
  A tab will take it and would rather not.
* **The knobs.** Twenty `env::var` calls would have to come off the query string.

What this does *not* change is the half of the question that was never about speed:
sailing together. A tab still has no UDP, still cannot find the game your friend is
running in the next room, and still needs a certificate and a public host to talk to
anybody at all.

## The shape that would make sense

A **single-player demo in a tab**, as a shop window: click a link, sail for five minutes,
install the real thing if you liked it. It needs the ripple bake spread across frames,
the knobs moved to the query string, and a click-to-start. The game stays native, keeps
its LAN play, and nothing has to be maintained twice except a build target.

What would *not* make sense is porting the game and keeping sailing together, because the
second half of that sentence costs a certificate, a public host, a second transport and
the end of hosting from a laptop — all to reach a browser that still cannot find the game
your friend is running in the next room.
