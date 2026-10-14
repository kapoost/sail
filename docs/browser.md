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

## The shape that would make sense

A **single-player demo in a tab**, as a shop window: click a link, sail for five minutes,
install the real thing if you liked it. It needs the ripple bake spread across frames,
the knobs moved to the query string, and a click-to-start. The game stays native, keeps
its LAN play, and nothing has to be maintained twice except a build target.

What would *not* make sense is porting the game and keeping sailing together, because the
second half of that sentence costs a certificate, a public host, a second transport and
the end of hosting from a laptop — all to reach a browser that still cannot find the game
your friend is running in the next room.
