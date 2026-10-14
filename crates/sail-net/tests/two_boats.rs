//! Two boats, one server, no window.
//!
//! The end-to-end check for the whole protocol, and it runs the real server binary rather
//! than a stub of it — a relay that works in a test and not in the program it was written
//! for is the commonest way this kind of thing is wrong. Everything above the socket is
//! shared with the game: the same `Link`, the same messages, the same encoding.

use std::process::{Child, Command};
use std::time::{Duration, Instant};

use sail_net::link::{Event, Link};
use sail_net::Drawn;

/// Kills the server when the test ends, whether it passed or panicked.
struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn wait_for(link: &mut Link, seconds: f32, mut done: impl FnMut(&Event) -> bool) -> bool {
    let until = Instant::now() + Duration::from_secs_f32(seconds);
    while Instant::now() < until {
        for event in link.drain() {
            if done(&event) {
                return true;
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

#[test]
fn two_boats_can_see_each_other() {
    // A port of its own, so running this while a game is up does not join the game.
    let port = 7791;
    let server = Server(
        Command::new(env!("CARGO_BIN_EXE_sail-server"))
            .args(["--port", &port.to_string(), "--seed", "42"])
            .stdout(std::process::Stdio::null())
            .spawn()
            .expect("the server binary is built by cargo before this test runs"),
    );
    let at = format!("127.0.0.1:{port}");

    let mut alice = Link::join(&at, "alice").expect("a socket");
    let mut bob = Link::join(&at, "bob").expect("a socket");

    // The handshake. Five seconds is generous — the hello repeats once a second and the
    // server is up in milliseconds — but a test that fails on a loaded machine teaches
    // nobody anything.
    let mut alice_seed = 0;
    assert!(
        wait_for(&mut alice, 5.0, |e| match e {
            Event::Joined { world, .. } => {
                alice_seed = world.seed;
                true
            }
            _ => false,
        }),
        "alice never got in"
    );
    assert_eq!(
        alice_seed, 42,
        "the sea everybody is on comes from the server"
    );
    assert!(
        wait_for(&mut bob, 5.0, |e| matches!(e, Event::Joined { .. })),
        "bob never got in"
    );

    // Alice sails. The link sends fifteen a second out of whatever it has most recently
    // been given, so handing it one snapshot repeatedly is what the game does.
    let alice_boat = Drawn {
        pos: [120.0, -340.0],
        heading: 1.1,
        heel: 0.25,
        surge: 6.0,
        ..Default::default()
    };
    let saw = {
        let until = Instant::now() + Duration::from_secs(5);
        let mut seen = None;
        while Instant::now() < until && seen.is_none() {
            alice.send(alice_boat);
            for event in bob.drain() {
                if let Event::Boat { name, drawn, .. } = event {
                    seen = Some((name, drawn));
                }
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        seen
    };
    let (name, drawn) = saw.expect("bob never saw alice's boat");
    assert_eq!(
        name, "alice",
        "a boat arrives with the name of whoever is sailing her"
    );
    assert!(
        (drawn.pos[0] - 120.0).abs() < 0.5 && (drawn.heel - 0.25).abs() < 0.01,
        "she arrived in the wrong place or on the wrong ear: {drawn:?}"
    );

    // And when she closes the window, she goes — at once, because the link says goodbye
    // on its way out rather than leaving the others to time her out six seconds later.
    drop(alice);
    assert!(
        wait_for(&mut bob, 5.0, |e| matches!(e, Event::Gone(_))),
        "alice left and nobody noticed"
    );
    drop(server);
}
