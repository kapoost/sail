# Sailing together

A plan for two to eight friends sailing the same archipelago at the same time. LAN
first, the internet after, and nothing in between that we have to throw away.

This is the shape that was settled on 13 October and the detail that was missing from
it. The one-line version: **every boat is computed by the person sailing her**, and the
server is a relay, a clock and a lobby. Nothing else.

## Why not the obvious thing

**Not lockstep on determinism.** The core is structurally deterministic — a fixed 200 Hz
step and its own PCG — but `sin`, `cos` and `powf` differ in their last bits between
platforms and libm versions. At two hundred steps a second the divergence is not a risk,
it is a certainty, and the failure mode is two people watching different boats and both
being right.

**Not server-authoritative either.** A sailor feels fifty milliseconds on the tiller
immediately. Anything that puts a round trip between the helm and the rudder will be
rejected by the one person this game is being built for, and the rejection will be
correct. So the helm is local, the physics is local, and what crosses the network is
what the *other* boats look like.

**Not a replication framework.** `bevy_replicon` and `lightyear` are good at the thing
they are for — replicating a server-owned ECS world to clients — and that is the
architecture we have just ruled out. What we need is small enough to read in one sitting:
a struct, a socket, and an interpolation.

## What crosses the wire

The world is a pure function of a seed. `islands::generate(seed)` is the archipelago and
the `Weather` struct is nine numbers, so joining a game costs about eighty bytes and then
both machines have the same water.

```
Hello    { name, protocol_version, boat_class }
Welcome  { world_seed, weather, server_time, your_id, already_here: [BoatId] }
Snapshot { id, t, draw: Drawn }          // 15 Hz, unreliable, newest wins
Weather  { tws, wind_from, cover, squalls: [(pos, radius, strength)] }   // 1 Hz
Bye      { id }
```

`Drawn` is what it takes to put somebody else's boat on the water, and nothing else:

| | |
|---|---|
| where she is | `pos: [f32; 2]`, relative to the world origin — 5.5 km of archipelago in f32 is centimetre precision |
| how she sits | `heading, heel, pitch, heave: f32` |
| where she is going | `surge, sway, yaw_rate: f32` — so the receiver can dead-reckon between packets |
| what is set | `reef, furl, staysail: f32`, `headsail: u8` |
| what the sails are doing | `main_sheet, jib_sheet, boom_angle: f32` |

Sixteen f32 and two bytes: about seventy bytes a boat a snapshot. Eight boats at 15 Hz is
eight and a half kilobytes a second in each direction, which is nothing on any link any
of us has. There is no compression, no delta encoding and no prioritisation in this plan
on purpose — at this scale they would be work spent on a problem we do not have.

**The gust field is not sent.** It is already a stateless function of position and time,
so every client computes the same gusts from the same `(seed, t)` without a packet. Only
the synoptic state and the squalls — which have their own position and radius — travel,
and at one hertz.

**The server owns the clock.** Clients run their own simulation as fast as they like, but
they take `t` for the weather from the server, so nobody is in yesterday's wind.

## The part that is actually hard

It is not the network. Twenty files in `sail-app` read the global `Sim` resource, and
there is no `BoatId` anywhere in the program: the camera, the sail meshes, the rig, the
wake, the spray, the deck, the instruments and the sound all assume there is exactly one
boat and that it is *the* boat. That assumption is the work.

The cheap way through it is **ghosts**. Another boat a hundred metres away does not need
telltales, a working winch, a wake that sheds vortices or a mainsail solved for its clew:
it needs a hull, two sails at roughly the right angle, and a heel. So the second boat is
not the first boat drawn again — it is a deliberately simpler thing, and keeping it
simple is what keeps the refactor from touching the single-boat path at all.

## Milestones

**M0 — the seam. No network in this milestone at all.**
Introduce `BoatView { state, controls, config }` and a `Fleet` resource holding zero or
more of them. Give the hull, sail and rig drawing systems a view to draw rather than a
resource to read. Then draw one static ghost fifty metres off the quarter and sail round
it. Done when the game looks and feels identical and the ghost is there. This is the
expensive one — a day or two — and it is worth doing whether or not the rest happens,
because it is also what a replay, a ghost of your own best leg, or an AI boat would need.

**M1 — two processes on one machine.**
`sail-net`: an engine-free crate with the message types, serialised with `postcard`, over
`std::net::UdpSocket` on loopback. `sail-server`: a binary that does not depend on Bevy,
so it builds in seconds and runs on anything. One process hosts, one joins, both see each
other move. Interpolation falls out for free — `BoatState::lerp` already exists and is
already used for the render-time smoothing.

**M2 — the LAN.**
Broadcast discovery on a fixed UDP port so a game on the network appears in a list
without anybody typing an address. Join and leave while a passage is running. The server
runs the weather and sends it at 1 Hz. A name over each boat. Done when four people on
the same wifi can sail together for an hour without anybody restarting anything.

**M3 — the internet.**
The protocol does not change. What changes is how the packets find each other, and there
are two honest answers for a game with this many players:

1. **An overlay network** — Tailscale or ZeroTier. Every friend installs it once, joins
   one tailnet, and the LAN build works unchanged over the internet with encryption and
   no port forwarding. Zero lines of code. This is the recommendation: it is what the
   problem is actually worth.
2. **A small relay** — a four-euro VPS running `sail-server` with a public address. About
   a hundred and fifty lines beyond M2 (a token so strangers do not wander in, and a
   lobby that lists running passages). Worth doing if friends will not install anything,
   or if the game ever wants to be joined from a link.

NAT hole-punching is the third answer and it is not worth it here: it is the hardest part
of multiplayer networking and it exists to avoid renting a server, which at this scale is
cheaper than the bug reports.

**M4 — a reason to be there.**
Everything above puts another boat on your water. None of it gives you anything to *do*
together. The shortest list worth having: the same consignment offered to everyone, a
start line and a finish, and each other's names and times. Racing a leg is the whole
game, and it needs no new physics.

## Decided, so nobody has to decide it twice

- **Boats do not collide.** Two client-authoritative boats that disagree about where they
  are cannot be made to collide without one of them being wrong on somebody's screen.
  Ghosts pass through each other, and an anchorage full of friends is a drawing problem
  rather than a physics one. Revisit only if somebody actually wants to race starts.
- **No anti-cheat, no validation.** Friends.
- **The host is a player's machine on LAN and a VPS on the internet.** Same binary.
- **Crates**: `serde` + `postcard` for the wire, the standard library for the socket. No
  QUIC until there is a reason — TLS for a friends-only game over an overlay network is a
  certificate problem we have not got.
- **`sail-net` is engine-free**, like `sail-core`. The server must not depend on Bevy.

## What could make this not worth doing

Said plainly, because it is the part a plan usually leaves out:

- M0 touches twenty files in a program whose single-boat assumptions are load-bearing. If
  that refactor turns out to be three days rather than one, stop and ask whether a ghost
  of your own previous passage would scratch the same itch for a tenth of the cost.
- Sailing is slow. Two friends on a five-mile leg are out of sight of each other within
  ten minutes, and "multiplayer" where you cannot see anybody is a chat room with weather.
  Whatever M4 turns out to be, it has to keep the boats within a mile of each other, which
  means the game design question — *racing*, probably — is more important than any of the
  engineering above.
