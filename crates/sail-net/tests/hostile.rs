//! What happens when the thing on the other end is not the game.
//!
//! **The protocol will be spoken by strangers.** Every other test here sends well-formed
//! messages from the real client to the real server, which is the case that was never in
//! doubt; this file sends the cases that were. It uses the same harness — a real
//! `sail-server` on a real socket — and a bare `UdpSocket` as the hostile end, because a
//! hostile peer does not use `Link` and a test that used it could not express most of
//! what is below.
//!
//! None of this is anti-cheat. A peer cannot move your boat (the server binds a boat to
//! its source address) and cannot run into you (boats pass through each other). What a
//! peer could do was hand you a number that is not a number, and a NaN that reaches the
//! simulation clock or the wind field does not crash anything — it quietly stops the sea
//! from being a sea, which is reported as "it broke" and found by reading.

use std::net::UdpSocket;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use sail_net::link::{Event, Link};
use sail_net::{
    decode, encode, Drawn, Sane, Sky, Squall, ToClient, ToServer, World, NAME_LIMIT, PROTOCOL,
    SQUALLS_LIMIT,
};

struct Server(Child);

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn server_on(port: u16, args: &[&str]) -> Server {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sail-server"));
    command.args(["--port", &port.to_string(), "--seed", "42"]);
    command.args(args);
    Server(
        command
            .stdout(std::process::Stdio::null())
            .spawn()
            .expect("the server binary is built by cargo before this test runs"),
    )
}

fn stranger() -> UdpSocket {
    let socket = UdpSocket::bind("127.0.0.1:0").expect("an ephemeral port");
    socket
        .set_read_timeout(Some(Duration::from_millis(400)))
        .expect("a read timeout");
    socket
}

/// Whatever the server says back, or nothing within the timeout.
fn answer(socket: &UdpSocket) -> Option<ToClient> {
    let mut buf = [0u8; 2048];
    let (n, _) = socket.recv_from(&mut buf).ok()?;
    decode::<ToClient>(&buf[..n])
}

/// Knock until somebody answers, which is what `Link` does and a bare socket does not.
///
/// A freshly spawned server takes a moment to bind, and a hello sent into that moment is
/// a hello nobody heard. The real client knocks once a second for exactly this reason;
/// these tests have to do it by hand or they are testing the scheduler.
fn knock(socket: &UdpSocket, at: &str, name: &str) -> Option<ToClient> {
    let hello = encode(&ToServer::Hello {
        protocol: PROTOCOL,
        name: name.into(),
        word: String::new(),
    });
    for _ in 0..25 {
        let _ = socket.send_to(&hello, at);
        if let Some(answer) = answer(socket) {
            return Some(answer);
        }
    }
    None
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

// --- What `Sane` is for, said as values rather than as prose ---------------------------

/// **The two that were actually exploitable**, both confirmed at a prompt before they were
/// fixed: a clock that is not a number poisons itself for ever, and a squall with no life
/// in it divides zero by zero.
#[test]
fn the_two_numbers_that_poisoned_everything_are_refused() {
    let poisoned = Sky {
        tws: 8.0,
        wind_from: 1.0,
        cover: 0.5,
        t: f32::NAN,
    };
    assert!(!poisoned.sane(), "a clock that is not a number");
    assert!(
        !Sky { t: 0.0, ..poisoned }.sane(),
        "nor one at zero, which is where a passage has not begun"
    );
    assert!(
        Sky {
            t: 12.5,
            ..poisoned
        }
        .sane(),
        "and a real one passes"
    );

    let dead = Squall {
        pos: [100.0, 200.0],
        radius: 400.0,
        strength: 6.0,
        age: 0.0,
        life: 0.0,
    };
    assert!(!dead.sane(), "no life in it is a division by zero");
    assert!(
        !Squall {
            radius: 0.0,
            ..dead
        }
        .sane(),
        "and so is no radius"
    );
    assert!(
        Squall {
            life: 300.0,
            ..dead
        }
        .sane(),
        "and a real one passes"
    );
}

/// Every float on a boat, one at a time. Written as a loop over setters rather than
/// seventeen assertions, because the failure anybody will actually cause here is adding a
/// field to `Drawn` and forgetting to put it in `sane`.
#[test]
fn not_one_float_on_a_boat_may_be_a_nan() {
    let good = Drawn {
        pos: [120.0, -40.0],
        heading: 1.2,
        surge: 5.5,
        ..Default::default()
    };
    assert!(good.sane());

    /// A field of `Drawn`, named and settable: enough to walk every float in the struct
    /// without writing the same three assertions seventeen times.
    type Field = (&'static str, fn(&mut Drawn, f32));

    let fields: [Field; 17] = [
        ("pos.x", |d, v| d.pos[0] = v),
        ("pos.y", |d, v| d.pos[1] = v),
        ("heading", |d, v| d.heading = v),
        ("heel", |d, v| d.heel = v),
        ("pitch", |d, v| d.pitch = v),
        ("heave", |d, v| d.heave = v),
        ("surge", |d, v| d.surge = v),
        ("sway", |d, v| d.sway = v),
        ("yaw_rate", |d, v| d.yaw_rate = v),
        ("boom_angle", |d, v| d.boom_angle = v),
        ("main_sheet", |d, v| d.main_sheet = v),
        ("jib_sheet", |d, v| d.jib_sheet = v),
        ("reef", |d, v| d.reef = v),
        ("furl", |d, v| d.furl = v),
        ("staysail", |d, v| d.staysail = v),
        ("staysail_sheet", |d, v| d.staysail_sheet = v),
        ("genoa_car", |d, v| d.genoa_car = v),
    ];
    for (name, set) in fields {
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut drawn = good;
            set(&mut drawn, bad);
            assert!(!drawn.sane(), "{name} = {bad} should not be accepted");
        }
    }

    // And a position out past anywhere a boat could be, which is the one value that
    // reaches a transform rather than a drawing.
    let mut far = good;
    far.pos[0] = 9.0e9;
    assert!(!far.sane(), "a boat nine million kilometres out");

    // What is *not* checked, deliberately: a boat trimmed like nothing on earth. Eleven
    // reefs and a sheet at minus four is a strange-looking drawing and a drawing cannot
    // hurt anybody. Validation that decided what was plausible would be anti-cheat.
    let odd = Drawn {
        reef: -4.0,
        main_sheet: 11.0,
        heel: 40.0,
        ..good
    };
    assert!(odd.sane(), "strange is allowed; not-a-number is not");
}

/// Lists and strings have a length somebody chose, rather than one the datagram buffer
/// happens to impose.
#[test]
fn a_name_and_a_list_have_an_end() {
    let long = ToServer::Hello {
        protocol: PROTOCOL,
        name: "n".repeat(NAME_LIMIT + 1),
        word: String::new(),
    };
    assert!(!long.sane());
    let ok = ToServer::Hello {
        protocol: PROTOCOL,
        name: "n".repeat(NAME_LIMIT),
        word: String::new(),
    };
    assert!(ok.sane());

    let sky = Sky {
        tws: 8.0,
        wind_from: 1.0,
        cover: 0.4,
        t: 30.0,
    };
    let one = Squall {
        pos: [0.0, 0.0],
        radius: 300.0,
        strength: 5.0,
        age: 10.0,
        life: 300.0,
    };
    let flood = ToClient::Weather {
        sky,
        squalls: vec![one; SQUALLS_LIMIT + 1],
    };
    assert!(!flood.sane(), "a weather report with a crowd in it");
    assert!(ToClient::Weather {
        sky,
        squalls: vec![one; SQUALLS_LIMIT],
    }
    .sane());
}

/// And the check is not something a caller can forget, because `decode` will not compile
/// without it. This is the round trip through the bytes themselves.
#[test]
fn the_check_is_in_the_decoding() {
    let poisoned = encode(&ToClient::Weather {
        sky: Sky {
            tws: 8.0,
            wind_from: 1.0,
            cover: 0.4,
            t: f32::NAN,
        },
        squalls: vec![],
    });
    assert!(
        decode::<ToClient>(&poisoned).is_none(),
        "a packet that encodes cleanly and means nothing"
    );

    let welcome = encode(&ToClient::Welcome {
        protocol: PROTOCOL,
        your_id: 3,
        world: World {
            seed: 7,
            tws: 9.0,
            wind_from: 2.0,
            fetch: f32::INFINITY,
            start_hour: 8.0,
            cover: 0.5,
        },
        sky: Sky {
            tws: 9.0,
            wind_from: 2.0,
            cover: 0.5,
            t: 1.0,
        },
    });
    assert!(
        decode::<ToClient>(&welcome).is_none(),
        "a fetch of infinity"
    );
}

// --- And the same thing against the real server, over a real socket --------------------

/// **Rubbish does not stop the relay.** Truncated messages, oversized ones, random bytes,
/// and a snapshot full of NaN — and then a real client joins and sails, which is the
/// assertion that matters: surviving is not the same as still working.
#[test]
fn a_server_shrugs_off_rubbish_and_carries_on() {
    let port = 7795;
    let _server = server_on(port, &[]);
    let at = format!("127.0.0.1:{port}");
    let socket = stranger();

    let good = encode(&ToServer::Hello {
        protocol: PROTOCOL,
        name: "nobody".into(),
        word: String::new(),
    });
    let mut rubbish: Vec<Vec<u8>> = vec![
        vec![],
        vec![0xff],
        vec![0x00; 7],
        vec![0xab; 2047],
        // A real message with its tail cut off, which is the shape a lost fragment has.
        good[..good.len() / 2].to_vec(),
        // A real message with something extra stuck to it.
        [good.clone(), vec![0x7f; 64]].concat(),
    ];
    // Every byte on its own: one of them is the tag of some message, and whichever one
    // that is, the body behind it is missing.
    for b in 0..=255u8 {
        rubbish.push(vec![b]);
    }
    for packet in &rubbish {
        let _ = socket.send_to(packet, &at);
    }

    // **A second socket to join on, and the first is never read from again.**
    //
    // The rubbish above is not silent: one of those two hundred and fifty-six single
    // bytes decodes as a `Hello` claiming protocol zero, and the server answers it — so
    // the next datagram arriving at the sender is a refusal about *that*, not a welcome
    // about this. The first version of this test knocked on the same socket, read the
    // next thing it was sent and asserted it was a welcome. It passed here and failed on
    // CI, which is the signature of a test that was reading a race rather than a result.
    //
    // Two sockets is also the truer picture: the thing throwing rubbish at a server and
    // the thing joining it are not the same party.
    let joiner = stranger();
    let id = match knock(&joiner, &at, "nobody") {
        Some(ToClient::Welcome { your_id, .. }) => your_id,
        other => panic!("expected a welcome, got {other:?}"),
    };

    // Somebody real, and *first*. The order matters and the first draft of this test had
    // it the other way round: the relay only passes a snapshot on at the moment it
    // arrives, so poison sent before anybody is listening is poison nobody could have
    // received, and the assertion at the bottom would have held whether or not the guard
    // existed. Checked by taking the guard out and watching this test go on passing.
    let mut alice = Link::join(&at, "alice", "").expect("a socket");
    assert!(
        wait_for(&mut alice, 10.0, |e| matches!(e, Event::Joined { .. })),
        "the relay stopped answering after a sackful of rubbish"
    );
    assert!(
        wait_for(&mut alice, 4.0, |e| matches!(e, Event::Weather { .. })),
        "the relay stopped sending the weather, which is the thing it is for"
    );

    let poison = Drawn {
        pos: [f32::NAN, f32::NAN],
        heading: f32::NAN,
        surge: f32::INFINITY,
        ..Default::default()
    };
    for _ in 0..20 {
        let _ = joiner.send_to(&encode(&ToServer::Here { id, drawn: poison }), &at);
        std::thread::sleep(Duration::from_millis(15));
    }
    // Refused at the decoding, so there was never anything to relay and alice is told
    // about nobody. Without the guard she is told about a boat at NaN, NaN, and from
    // there it reaches a transform.
    assert!(
        !wait_for(&mut alice, 1.5, |e| matches!(e, Event::Boat { .. })),
        "a boat made of NaN reached the fleet"
    );
}

/// **The door has a width.** Twelve boats, and the thirteenth is told why rather than
/// left wondering.
#[test]
fn a_full_game_says_so() {
    let port = 7796;
    let _server = server_on(port, &[]);
    let at = format!("127.0.0.1:{port}");

    // Twelve separate sockets, which is twelve separate addresses as far as the server
    // is concerned — the same thing a flood would be, done honestly.
    let mut aboard = Vec::new();
    for i in 0..12 {
        let socket = stranger();
        assert!(
            matches!(
                knock(&socket, &at, &format!("boat {i}")),
                Some(ToClient::Welcome { .. })
            ),
            "boat {i} of twelve should get in"
        );
        aboard.push(socket);
    }

    let thirteenth = stranger();
    match knock(&thirteenth, &at, "one too many") {
        Some(ToClient::No { because }) => assert!(
            because.contains("full"),
            "the refusal should say what is wrong, and it said {because:?}"
        ),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// **One packet used to put anybody ashore.** A refusal was accepted from the server's
/// address on any port, so anything at all on that machine could end a session. The
/// client now wants the port it is talking to.
#[test]
fn a_refusal_from_the_wrong_port_is_not_a_refusal() {
    let port = 7797;
    let _server = server_on(port, &[]);
    let at = format!("127.0.0.1:{port}");

    let mut alice = Link::join(&at, "alice", "").expect("a socket");
    assert!(
        wait_for(&mut alice, 10.0, |e| matches!(e, Event::Joined { .. })),
        "she has to be in before she can be put ashore"
    );

    // Everything about this is right except the port it comes from.
    let forger = stranger();
    let no = encode(&ToClient::No {
        because: "get off my water".into(),
    });
    // Aimed at the client's own socket. Its address is not something a test can ask for,
    // so this sweeps the ephemeral range the way an attacker on the box would — which is
    // also the point: on one machine that sweep is cheap.
    for guess in 1024..=65535u16 {
        if guess == port {
            continue;
        }
        let _ = forger.send_to(&no, ("127.0.0.1", guess));
    }
    assert!(
        !wait_for(&mut alice, 2.0, |e| matches!(e, Event::Lost(_))),
        "a refusal from the wrong port put her ashore"
    );
}
