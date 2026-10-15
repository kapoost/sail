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

    let mut alice = Link::join(&at, "alice", "").expect("a socket");
    let mut bob = Link::join(&at, "bob", "").expect("a socket");

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

/// A server answers "is there a game here?" — the half of discovery that is protocol.
///
/// Asked directly rather than broadcast, on purpose: whether a broadcast gets off the
/// machine depends on the network the test happens to be run on, and a test that fails in
/// a sandbox teaches nobody anything. The broadcast half is one line in `discovery.rs`
/// and is verified by running the game on a real network, which is the only place it can
/// honestly be verified anyway.
#[test]
fn a_server_says_it_is_there() {
    let port = 7792;
    let _server = Server(
        Command::new(env!("CARGO_BIN_EXE_sail-server"))
            .args(["--port", &port.to_string(), "--seed", "99"])
            .stdout(std::process::Stdio::null())
            .spawn()
            .expect("the server binary"),
    );
    let socket = std::net::UdpSocket::bind("0.0.0.0:0").expect("a socket");
    socket
        .set_read_timeout(Some(Duration::from_millis(400)))
        .unwrap();
    let ask = sail_net::encode(&sail_net::ToServer::WhoIsThere {
        word: String::new(),
    });
    let mut buf = [0u8; 2048];
    let until = Instant::now() + Duration::from_secs(5);
    let mut found = None;
    while Instant::now() < until && found.is_none() {
        let _ = socket.send_to(&ask, ("127.0.0.1", port));
        if let Ok((n, _)) = socket.recv_from(&mut buf) {
            if let Some(sail_net::ToClient::Beacon { game, seed, .. }) =
                sail_net::decode::<sail_net::ToClient>(&buf[..n])
            {
                found = Some((game, seed));
            }
        }
    }
    let (game, seed) = found.expect("a server that will not say it exists cannot be found");
    assert_eq!(seed, 99, "the beacon carries the sea it is offering");
    assert!(!game.is_empty(), "and something a person can recognise");
}

/// **The door**, and the two things it has to get right.
///
/// A game with a word on it lets in whoever says it and refuses whoever does not — and
/// the refusal has to arrive, because a door that silently drops the knock is
/// indistinguishable from a server that is not running, and somebody will spend an
/// evening on their firewall over it.
#[test]
fn a_word_keeps_strangers_out_and_friends_in() {
    // A port of its own: 7791 and 7792 are taken by the tests above, and two tests on
    // one port is how the one above started failing for reasons that had nothing to do
    // with it — it heard this server's beacon and reported this server's seed.
    let port = 7794;
    let word = std::env::temp_dir().join("sail-test-word");
    std::fs::write(&word, "two bells\n").expect("a writable temp dir");
    let _server = Server(
        Command::new(env!("CARGO_BIN_EXE_sail-server"))
            .args([
                "--port",
                &port.to_string(),
                "--seed",
                "42",
                "--word-file",
                word.to_str().expect("a sane temp path"),
            ])
            .stdout(std::process::Stdio::null())
            .spawn()
            .expect("the server binary"),
    );
    let at = format!("127.0.0.1:{port}");

    // The word is trimmed at both ends, so the newline an editor leaves on the file does
    // not lock the owner out of his own game.
    let mut friend = Link::join(&at, "friend", "two bells").expect("a socket");
    assert!(
        wait_for(&mut friend, 5.0, |e| matches!(e, Event::Joined { .. })),
        "the word was right and the game did not let her in"
    );

    let mut stranger = Link::join(&at, "stranger", "three bells").expect("a socket");
    assert!(
        wait_for(&mut stranger, 5.0, |e| matches!(e, Event::Lost(_))),
        "a stranger with the wrong word was not told no"
    );
    assert!(
        stranger.id.is_none(),
        "a stranger with the wrong word got an id"
    );
    let _ = std::fs::remove_file(&word);
}

/// And the client knocks again when the server comes back.
///
/// The failure this is about is quiet, which is why it needs a test rather than an eye:
/// the server restarts, the client goes on sending snapshots with an id that no longer
/// exists, and the only symptom is that the other boats are gone. It was the reason a
/// service unit with `Restart=always` on it was a trap rather than a safety net.
#[test]
fn a_client_finds_its_way_back_after_the_server_restarts() {
    let port = 7795;
    let start = || {
        Server(
            Command::new(env!("CARGO_BIN_EXE_sail-server"))
                .args(["--port", &port.to_string(), "--seed", "42"])
                .stdout(std::process::Stdio::null())
                .spawn()
                .expect("the server binary"),
        )
    };
    let first = start();
    let at = format!("127.0.0.1:{port}");
    let mut boat = Link::join(&at, "boat", "").expect("a socket");
    assert!(
        wait_for(&mut boat, 5.0, |e| matches!(e, Event::Joined { .. })),
        "never got in to begin with"
    );
    let was = boat.id;

    drop(first);
    // Six seconds of silence is what both ends are waiting for, and then the knocking
    // starts again. Ten is that plus room for a loaded machine.
    assert!(
        wait_for(&mut boat, 10.0, |e| matches!(e, Event::Quiet)),
        "the client did not notice the game had gone quiet"
    );
    let _second = start();
    assert!(
        wait_for(&mut boat, 15.0, |e| matches!(e, Event::Joined { .. })),
        "the server came back and the client never knocked again"
    );
    // And it is holding an id the new server actually issued. Which may be the same
    // number as the old one — a server that has just started numbers from one, and the
    // boat that rejoins first gets one again — so what is checked is that there is an
    // id at all, by the handshake above rather than by the number.
    assert!(boat.id.is_some(), "rejoined without an id");
    let _ = was;
}

/// **Knocking before joining**, which is the half of a lobby that matters over the
/// internet: the question carries the word, and a server that does not hear the right
/// one says nothing at all rather than becoming a reflector.
#[test]
fn you_can_knock_on_a_game_before_joining_it() {
    let port = 7796;
    let word = std::env::temp_dir().join("sail-test-knock");
    std::fs::write(&word, "two bells\n").expect("a writable temp dir");
    let _server = Server(
        Command::new(env!("CARGO_BIN_EXE_sail-server"))
            .args([
                "--port",
                &port.to_string(),
                "--seed",
                "7",
                "--word-file",
                word.to_str().expect("a sane temp path"),
            ])
            .stdout(std::process::Stdio::null())
            .spawn()
            .expect("the server binary"),
    );
    let at = format!("127.0.0.1:{port}");

    let knock = sail_net::discovery::Probe::begin(&at, "two bells");
    let until = Instant::now() + Duration::from_secs(8);
    let mut answer = None;
    while Instant::now() < until && answer.is_none() {
        answer = knock.answer();
        std::thread::sleep(Duration::from_millis(50));
    }
    match answer.expect("the probe never answered at all") {
        Ok(found) => assert_eq!(found.seed, 7, "it described the game it is offering"),
        Err(why) => panic!("the word was right and it would not say what it is: {why}"),
    }

    // And with the wrong word: silence, then a sentence a person can act on. Silence
    // rather than a refusal is the point — a refusal is a packet, and a packet sent to
    // whoever asked is the amplifier this is all about.
    let stranger = sail_net::discovery::Probe::begin(&at, "three bells");
    let until = Instant::now() + Duration::from_secs(8);
    let mut answer = None;
    while Instant::now() < until && answer.is_none() {
        answer = stranger.answer();
        std::thread::sleep(Duration::from_millis(50));
    }
    match answer.expect("the probe never gave up") {
        Ok(found) => panic!("it told a stranger about the game: {}", found.game),
        Err(why) => assert!(
            why.contains("word"),
            "the first thing to check is the word, and it did not say so: {why}"
        ),
    }
    let _ = std::fs::remove_file(&word);
}
