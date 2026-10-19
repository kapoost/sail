//! A setup screen for the boat, in the shape of the one that used to come up when you
//! held Delete.
//!
//! Developing this game means doing the same four things forty times an afternoon: put a
//! relay up, restart it after a protocol change, start the game with one knob moved —
//! thirty metres of line out, a ling on the hook, a squall every two minutes, somebody
//! else's address in the join box — and then kill the lot. All of that already exists as
//! environment variables and a binary or two, which means all of it is a shell line
//! somebody has to remember, retype and get wrong at six o'clock.
//!
//! So: one screen, every knob on it, and the state of the relay in the corner. The BIOS
//! look is not only a joke. A setup screen is the right shape for this — a flat list of
//! settings with their current values, a line of help for whichever one is under the
//! cursor, and a key legend along the bottom — and it is the shape everybody who would
//! use this already knows how to drive.
//!
//! ```text
//! cargo run -p sail-bios
//! ```
//!
//! It owns what it starts: leave and anything the screen started goes with it, which is
//! the behaviour you want from a thing you are using to try something twenty times.

use std::io::{stdout, Write};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor};
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, queue};

/// The screen is drawn at this size whatever the window is, the way a text mode is a
/// text mode. Eighty by twenty-five is the size this look was invented at and the one it
/// is legible at; a wider terminal gets a margin rather than a stretched box.
const WIDE: u16 = 80;
const HIGH: u16 = 25;

/// What a row on the screen is.
enum Kind {
    /// A heading with nothing to change.
    Rule,
    /// Off or on.
    Flag,
    /// A number, with the step a cursor key moves it by.
    Number { step: f64, least: f64, most: f64 },
    /// One of a fixed set, cycled with the cursor keys.
    Choice(&'static [&'static str]),
    /// Free text: an address, a word, a name.
    Text,
    /// Something that happens when you press Enter.
    Act(Act),
}

#[derive(Clone, Copy, PartialEq)]
enum Act {
    NewWorld,
    RelayStart,
    RelayRestart,
    RelayStop,
    RemoteStatus,
    RemoteRestart,
    RemoteLog,
    Build,
    Launch,
    LaunchDebug,
    Kill,
}

struct Row {
    /// What it is called, in the left column.
    label: &'static str,
    /// And what it does, in the help pane — one sentence, because a BIOS help pane is
    /// one sentence and because a setting that needs a paragraph is a setting that is
    /// wrong.
    help: &'static str,
    kind: Kind,
    /// Its value, as text, because every one of these ends up in an environment variable
    /// and that is a string.
    value: String,
    /// The variable it sets, if it sets one. A row with no name is a knob the launcher
    /// itself reads.
    env: Option<&'static str>,
}

impl Row {
    fn rule(label: &'static str) -> Self {
        Self {
            label,
            help: "",
            kind: Kind::Rule,
            value: String::new(),
            env: None,
        }
    }
    fn flag(label: &'static str, env: &'static str, help: &'static str, on: bool) -> Self {
        Self {
            label,
            help,
            kind: Kind::Flag,
            value: if on { "on".into() } else { "off".into() },
            env: Some(env),
        }
    }
    fn number(
        label: &'static str,
        env: &'static str,
        help: &'static str,
        value: f64,
        step: f64,
        least: f64,
        most: f64,
    ) -> Self {
        Self {
            label,
            help,
            kind: Kind::Number { step, least, most },
            value: trim_number(value),
            env: Some(env),
        }
    }
    fn choice(
        label: &'static str,
        env: &'static str,
        help: &'static str,
        of: &'static [&'static str],
    ) -> Self {
        Self {
            label,
            help,
            kind: Kind::Choice(of),
            value: of[0].to_string(),
            env: Some(env),
        }
    }
    fn text(label: &'static str, env: &'static str, help: &'static str, value: String) -> Self {
        Self {
            label,
            help,
            kind: Kind::Text,
            value,
            env: Some(env),
        }
    }
    fn act(label: &'static str, help: &'static str, what: Act) -> Self {
        Self {
            label,
            help,
            kind: Kind::Act(what),
            value: String::new(),
            env: None,
        }
    }
    fn selectable(&self) -> bool {
        !matches!(self.kind, Kind::Rule)
    }
}

/// Numbers that are whole are written whole. "30" rather than "30.0", because a BIOS
/// screen that says 30.0 metres of line is a BIOS screen written by a float.
fn trim_number(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 {
        format!("{}", v.round() as i64)
    } else {
        format!("{v}")
    }
}

/// Everything the screen is holding on to.
struct Bios {
    rows: Vec<Row>,
    at: usize,
    /// The relay this screen started, if it started one.
    relay: Option<Child>,
    /// And the game.
    game: Option<Child>,
    /// The bottom line, which says what just happened.
    said: String,
    /// Where the remote relay lives, if there is one: an ssh host, from `SAIL_REMOTE`.
    remote: Option<String>,
}

fn main() -> std::io::Result<()> {
    let remote = std::env::var("SAIL_REMOTE").ok().filter(|s| !s.is_empty());
    // What the game remembered about where it sails, so the launcher opens with the
    // address already in it rather than making somebody type it twice over.
    let (name, address, word) = sail_net::remembered::read();
    let mut bios = Bios {
        rows: vec![
            Row::rule("RELAY"),
            Row::number(
                "port",
                "",
                "which port the local relay listens on",
                7714.0,
                1.0,
                1024.0,
                65535.0,
            ),
            Row::number(
                "seed",
                "",
                "the sea the relay offers; the same number is the same archipelago",
                42.0,
                1.0,
                0.0,
                1e9,
            ),
            Row::text(
                "word",
                "",
                "the word the relay asks for; empty means the door is open",
                word.clone(),
            ),
            Row::act(
                "start the relay",
                "run sail-server here, with the port, seed and word above",
                Act::RelayStart,
            ),
            Row::act(
                "restart the relay",
                "stop it and start it again, which is what a protocol change needs",
                Act::RelayRestart,
            ),
            Row::act(
                "stop the relay",
                "and leave nothing running",
                Act::RelayStop,
            ),
            Row::act(
                "new world for everybody",
                "a fresh sea and a fresh position for every boat on the game, and \
                 nobody knows where they are",
                Act::NewWorld,
            ),
            Row::rule("THE RELAY ON THE SERVER"),
            Row::act(
                "status",
                "systemctl is-active, over ssh, on $SAIL_REMOTE",
                Act::RemoteStatus,
            ),
            Row::act(
                "restart",
                "after putting a new binary there",
                Act::RemoteRestart,
            ),
            Row::act(
                "the last of the log",
                "journalctl -u sail-server, the last dozen lines",
                Act::RemoteLog,
            ),
            Row::rule("THE BOAT"),
            Row::choice(
                "headsail",
                "SAIL_HEADSAIL",
                "which sail is up forward when she starts",
                &["genoa", "code0", "gennaker", "spinnaker"],
            ),
            Row::number(
                "fleet",
                "SAIL_FLEET",
                "how many ghost boats to put on the water with no network at all",
                0.0,
                1.0,
                0.0,
                16.0,
            ),
            Row::flag(
                "tutorial",
                "SAIL_TUTORIAL",
                "start in the drill rather than on a passage",
                false,
            ),
            Row::number(
                "squall every",
                "SAIL_SQUALL",
                "seconds between squalls; the weather you would otherwise wait an hour for",
                0.0,
                30.0,
                0.0,
                3600.0,
            ),
            Row::number(
                "wind, knots",
                "SAIL_WIND",
                "the day's breeze, whatever the draw said; 0 leaves it to the draw",
                0.0,
                1.0,
                0.0,
                45.0,
            ),
            Row::flag(
                "sea already built",
                "SAIL_SEA",
                "open with the sea this wind has made instead of the two hours it takes",
                false,
            ),
            Row::number(
                "camera sweep",
                "SAIL_ORBIT",
                "degrees a second the view swings round her on its own",
                0.0,
                1.0,
                0.0,
                45.0,
            ),
            Row::choice(
                "tone curve",
                "SAIL_TONEMAP",
                "how the light is got into a screen that stops at one",
                &[
                    "",
                    "tony",
                    "neutral",
                    "aces",
                    "agx",
                    "reinhard",
                    "luminance",
                ],
            ),
            Row::number(
                "wind belts",
                "SAIL_BANDS",
                "km from one trade belt to the next; 0 is one wind everywhere",
                0.0,
                5.0,
                0.0,
                400.0,
            ),
            Row::rule("THE ROD"),
            Row::number(
                "line out",
                "SAIL_ROD",
                "metres of line streamed before the first frame",
                0.0,
                10.0,
                0.0,
                200.0,
            ),
            Row::choice(
                "on the hook",
                "SAIL_HOOK",
                "a fish of this species as soon as the lure is fishing",
                &["", "mackerel", "pollack", "cod", "saithe", "ling"],
            ),
            Row::number(
                "takes",
                "SAIL_FISH",
                "multiply how often anything takes; the fight is the interesting part",
                1.0,
                5.0,
                1.0,
                100.0,
            ),
            Row::rule("SAILING TOGETHER"),
            Row::text("name", "SAIL_NAME", "what goes over your boat", name),
            Row::text(
                "join",
                "SAIL_JOIN",
                "an address to join on startup; empty starts alone",
                address,
            ),
            Row::rule("THE PICTURE"),
            Row::number(
                "pixels",
                "SAIL_PIXELS",
                "0 is the sharp picture, anything else is the drawn one",
                0.0,
                720.0,
                0.0,
                720.0,
            ),
            Row::flag(
                "the ink drawing",
                "SAIL_INK",
                "the chart-pen picture instead of the painted one",
                false,
            ),
            Row::flag(
                "key log",
                "SAIL_KEYS",
                "print what every key press did, for when one of them is not catching",
                false,
            ),
            Row::rule("GO"),
            Row::act(
                "build",
                "cargo build --release -p sail-app, with the output on the screen",
                Act::Build,
            ),
            Row::act(
                "launch",
                "the release build, with everything above in its environment",
                Act::Launch,
            ),
            Row::act(
                "launch (debug)",
                "the debug build, which is slow and has the assertions in it",
                Act::LaunchDebug,
            ),
            Row::act(
                "kill the game",
                "if it is still up from last time",
                Act::Kill,
            ),
        ],
        at: 1,
        relay: None,
        game: None,
        said: String::new(),
        remote,
    };
    // A relay that is already running — from a previous launcher, or by hand — is worth
    // knowing about, because the alternative is two of them on one port and a confusing
    // twenty minutes.
    if already_listening() {
        bios.said = "something is already on that port".into();
    }

    // A screen needs a screen. Said plainly, because the alternative is a raw-mode
    // error code from the operating system at somebody who piped this into a file.
    if terminal::enable_raw_mode().is_err() {
        eprintln!("sail-bios is a screen: run it in a terminal, not down a pipe");
        return Ok(());
    }
    execute!(stdout(), EnterAlternateScreen, Hide)?;
    let outcome = run(&mut bios);
    execute!(stdout(), Show, LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;
    // What it started, it stops. A launcher that leaves a relay running after it has
    // gone is a launcher that makes you learn pkill.
    if let Some(mut relay) = bios.relay.take() {
        let _ = relay.kill();
        let _ = relay.wait();
        println!("the relay went with it");
    }
    if let Some(mut game) = bios.game.take() {
        let _ = game.kill();
        let _ = game.wait();
    }
    outcome
}

fn run(bios: &mut Bios) -> std::io::Result<()> {
    loop {
        draw(bios)?;
        // A tick rather than a block, so a relay that dies on its own is noticed rather
        // than sat there looking alive.
        if !event::poll(Duration::from_millis(400))? {
            reap(bios);
            continue;
        }
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != event::KeyEventKind::Press {
            continue;
        }
        match key {
            KeyEvent {
                code: KeyCode::Esc, ..
            } => return Ok(()),
            KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => return Ok(()),
            KeyEvent {
                code: KeyCode::Up, ..
            } => move_to(bios, -1),
            KeyEvent {
                code: KeyCode::Down,
                ..
            } => move_to(bios, 1),
            KeyEvent {
                code: KeyCode::Left,
                ..
            } => nudge(bios, -1.0),
            KeyEvent {
                code: KeyCode::Right,
                ..
            } => nudge(bios, 1.0),
            KeyEvent {
                code: KeyCode::Enter,
                ..
            } => enter(bios)?,
            KeyEvent {
                code: KeyCode::F(10),
                ..
            } => {
                do_act(bios, Act::Launch)?;
            }
            _ => {}
        }
    }
}

/// Up and down, skipping the headings, because a cursor that can land on a heading is a
/// cursor that stops for no reason.
fn move_to(bios: &mut Bios, by: isize) {
    let n = bios.rows.len() as isize;
    let mut at = bios.at as isize;
    for _ in 0..n {
        at = (at + by).rem_euclid(n);
        if bios.rows[at as usize].selectable() {
            bios.at = at as usize;
            return;
        }
    }
}

/// Left and right: the thing a BIOS does instead of a dialogue box.
fn nudge(bios: &mut Bios, by: f64) {
    let row = &mut bios.rows[bios.at];
    match &row.kind {
        Kind::Flag => {
            row.value = if row.value == "on" { "off" } else { "on" }.to_string();
        }
        Kind::Number { step, least, most } => {
            let now: f64 = row.value.parse().unwrap_or(0.0);
            row.value = trim_number((now + by * step).clamp(*least, *most));
        }
        Kind::Choice(of) => {
            let at = of.iter().position(|v| *v == row.value).unwrap_or(0) as isize;
            let next = (at + by as isize).rem_euclid(of.len() as isize) as usize;
            row.value = of[next].to_string();
        }
        _ => {}
    }
}

/// Enter: type into a text field, or do the thing.
fn enter(bios: &mut Bios) -> std::io::Result<()> {
    match bios.rows[bios.at].kind {
        Kind::Text | Kind::Number { .. } => {
            let typed = read_line(bios)?;
            if let Some(typed) = typed {
                bios.rows[bios.at].value = typed;
            }
        }
        Kind::Act(what) => do_act(bios, what)?,
        _ => nudge(bios, 1.0),
    }
    Ok(())
}

/// Typing in place, with the field inverted, which is how these screens have always
/// taken a number.
fn read_line(bios: &mut Bios) -> std::io::Result<Option<String>> {
    let mut typed = bios.rows[bios.at].value.clone();
    loop {
        draw_with_edit(bios, &typed)?;
        let Event::Key(key) = event::read()? else {
            continue;
        };
        if key.kind != event::KeyEventKind::Press {
            continue;
        }
        match key.code {
            KeyCode::Enter => return Ok(Some(typed)),
            KeyCode::Esc => return Ok(None),
            KeyCode::Backspace => {
                typed.pop();
            }
            KeyCode::Char(c) => typed.push(c),
            _ => {}
        }
    }
}

/// The environment the game gets: every row that names a variable and has something to
/// say. An empty value is left out rather than set to "", because `SAIL_HOOK=` and no
/// `SAIL_HOOK` at all are different things to the program reading them.
fn environment(bios: &Bios) -> Vec<(String, String)> {
    let mut env = Vec::new();
    for row in &bios.rows {
        let Some(name) = row.env else { continue };
        if name.is_empty() {
            continue;
        }
        let value = row.value.trim();
        let skip = value.is_empty()
            || value == "off"
            || (matches!(row.kind, Kind::Number { .. }) && value == "0")
            || (name == "SAIL_FISH" && value == "1")
            || (name == "SAIL_HEADSAIL" && value == "genoa");
        if skip {
            continue;
        }
        let value = if value == "on" { "1" } else { value };
        env.push((name.to_string(), value.to_string()));
    }
    // The word goes with the address rather than with the relay when there is an address
    // to go with: joining a game that has a door needs it, and that is the same string
    // the relay above is started with.
    if let (Some(join), Some(word)) = (value_of(bios, "join"), value_of(bios, "word")) {
        if !join.trim().is_empty() && !word.trim().is_empty() {
            env.push(("SAIL_WORD".into(), word));
        }
    }
    env
}

fn value_of(bios: &Bios, label: &str) -> Option<String> {
    bios.rows
        .iter()
        .find(|r| r.label == label)
        .map(|r| r.value.clone())
}

fn do_act(bios: &mut Bios, what: Act) -> std::io::Result<()> {
    match what {
        Act::RelayStart | Act::RelayRestart => {
            if let Some(mut was) = bios.relay.take() {
                let _ = was.kill();
                let _ = was.wait();
            }
            let port = value_of(bios, "port").unwrap_or_default();
            let seed = value_of(bios, "seed").unwrap_or_default();
            let word = value_of(bios, "word").unwrap_or_default();
            let mut relay = Command::new(binary("sail-server", false));
            relay
                .args(["--port", &port, "--seed", &seed])
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            if !word.trim().is_empty() {
                relay.env("SAIL_WORD", word.trim());
            }
            match relay.spawn() {
                Ok(child) => {
                    bios.said = format!("relay up on {port}, sea {seed}");
                    bios.relay = Some(child);
                }
                Err(e) => bios.said = format!("no relay: {e} — build it first"),
            }
        }
        // **A new world for a whole fleet, without a line of protocol.**
        //
        // The relay hands every boat its world at the handshake and the client now
        // knocks again after six seconds of silence — so a relay that comes back up with
        // a different seed *is* the reset: everybody rejoins, everybody is handed the
        // new archipelago and the new weather, everybody's boat is placed in it by the
        // simulation that replaces theirs, and nobody knows where they are, because a
        // coast you have never seen is a coast you have to work out. The whole feature
        // is two lines here and `navigation::cast_adrift` at the far end.
        //
        // Which is why it is a launcher button and not a key on the boat. Resetting the
        // sea under four other people is not something to have under a thumb.
        Act::NewWorld => {
            let fresh = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() % 1_000_000)
                .unwrap_or(7);
            if let Some(seed) = bios.rows.iter_mut().find(|r| r.label == "seed") {
                seed.value = fresh.to_string();
            }
            if bios.remote.is_some() {
                // The server keeps its seed in its environment file, so the new world
                // has to survive the restart that follows — and any restart after it.
                remote(
                    bios,
                    &format!(
                        "sed -i 's/^SAIL_SEED=.*/SAIL_SEED={fresh}/' /etc/sail-server.env \
                         && systemctl restart sail-server && sleep 1 \
                         && systemctl is-active sail-server"
                    ),
                )?;
                bios.said = format!("the server is on sea {fresh}");
            } else {
                do_act(bios, Act::RelayRestart)?;
                bios.said = format!("sea {fresh} — everybody rejoins into it");
            }
        }
        Act::RelayStop => {
            if let Some(mut was) = bios.relay.take() {
                let _ = was.kill();
                let _ = was.wait();
                bios.said = "relay stopped".into();
            } else {
                bios.said = "no relay of ours to stop".into();
            }
        }
        Act::RemoteStatus => remote(bios, "systemctl is-active sail-server; systemctl --no-pager --lines=3 status sail-server | tail -4")?,
        Act::RemoteRestart => remote(bios, "systemctl restart sail-server && sleep 1 && systemctl is-active sail-server")?,
        Act::RemoteLog => remote(bios, "journalctl -u sail-server --no-pager | tail -12")?,
        Act::Build => {
            shell_out(
                bios,
                Command::new("cargo").args(["build", "--release", "-p", "sail-app", "-p", "sail-net"]),
            )?;
        }
        Act::Launch | Act::LaunchDebug => {
            let debug = what == Act::LaunchDebug;
            if let Some(mut was) = bios.game.take() {
                let _ = was.kill();
                let _ = was.wait();
            }
            let mut game = Command::new(binary("sail-app", debug));
            for (name, value) in environment(bios) {
                game.env(name, value);
            }
            game.stdout(Stdio::null()).stderr(Stdio::null());
            match game.spawn() {
                Ok(child) => {
                    bios.said = "she is away".into();
                    bios.game = Some(child);
                }
                Err(e) => bios.said = format!("nothing to launch: {e} — build it first"),
            }
        }
        Act::Kill => {
            if let Some(mut was) = bios.game.take() {
                let _ = was.kill();
                let _ = was.wait();
                bios.said = "the game is down".into();
            } else {
                bios.said = "no game of ours is up".into();
            }
        }
    }
    Ok(())
}

/// Anything that needs the whole screen gets the whole screen: out of the alternate
/// buffer, run it with its output where output goes, and come back when somebody has
/// read it. Which is exactly what a BIOS does to flash a chip.
fn shell_out(bios: &mut Bios, command: &mut Command) -> std::io::Result<()> {
    execute!(stdout(), Show, LeaveAlternateScreen)?;
    terminal::disable_raw_mode()?;
    let status = command.status();
    println!(
        "\n{}",
        match &status {
            Ok(s) if s.success() => "done — any key".to_string(),
            Ok(s) => format!("exit {s} — any key"),
            Err(e) => format!("could not run it: {e} — any key"),
        }
    );
    let _ = stdout().flush();
    terminal::enable_raw_mode()?;
    loop {
        if let Event::Key(k) = event::read()? {
            if k.kind == event::KeyEventKind::Press {
                break;
            }
        }
    }
    execute!(stdout(), EnterAlternateScreen, Hide)?;
    bios.said = match status {
        Ok(s) if s.success() => "done".into(),
        Ok(s) => format!("exit {s}"),
        Err(e) => format!("{e}"),
    };
    Ok(())
}

fn remote(bios: &mut Bios, line: &str) -> std::io::Result<()> {
    let Some(host) = bios.remote.clone() else {
        bios.said = "no server: set SAIL_REMOTE to an ssh host".into();
        return Ok(());
    };
    shell_out(bios, Command::new("ssh").arg(&host).arg(line))
}

/// Where a binary is, built or not. The launcher lives in the same workspace as the
/// things it starts, so it knows.
fn binary(name: &str, debug: bool) -> std::path::PathBuf {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_default();
    root.join("target")
        .join(if debug { "debug" } else { "release" })
        .join(name)
}

/// Whether anything at all is on the default port, which is not the same question as
/// whether *we* started it.
fn already_listening() -> bool {
    Command::new("sh")
        .arg("-c")
        .arg("lsof -nP -iUDP:7714 >/dev/null 2>&1 || ss -lun 2>/dev/null | grep -q ':7714 '")
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Children that have gone on their own, so the screen stops claiming they are up.
fn reap(bios: &mut Bios) {
    for (slot, what) in [(&mut bios.relay, "relay"), (&mut bios.game, "game")] {
        if let Some(child) = slot {
            if matches!(child.try_wait(), Ok(Some(_))) {
                *slot = None;
                bios.said = format!("the {what} stopped on its own");
            }
        }
    }
}

// --- The screen ----------------------------------------------------------------

const BACK: Color = Color::AnsiValue(18);
const PANEL: Color = Color::AnsiValue(25);
const INK: Color = Color::AnsiValue(252);
const DIM: Color = Color::AnsiValue(249);
const PICK: Color = Color::AnsiValue(226);

fn draw(bios: &Bios) -> std::io::Result<()> {
    draw_inner(bios, None)
}

fn draw_with_edit(bios: &Bios, typing: &str) -> std::io::Result<()> {
    draw_inner(bios, Some(typing))
}

fn draw_inner(bios: &Bios, typing: Option<&str>) -> std::io::Result<()> {
    let mut out = stdout();
    queue!(out, SetBackgroundColor(BACK), Clear(ClearType::All))?;

    // The title bar, the one piece of the look that does all the work.
    queue!(
        out,
        MoveTo(0, 0),
        SetBackgroundColor(PANEL),
        SetForegroundColor(INK),
        Print(centred(
            &format!("SAIL SETUP UTILITY  —  v{}", game_version()),
            WIDE
        ))
    )?;

    // The two columns of rows.
    let top = 2u16;
    let per = bios.rows.len().div_ceil(2).max(1);
    for (i, row) in bios.rows.iter().enumerate() {
        let column = i / per;
        let line = top + (i % per) as u16;
        let x = 2 + column as u16 * 40;
        let chosen = i == bios.at;
        let value = if chosen {
            typing
                .map(|t| t.to_string())
                .unwrap_or_else(|| row.value.clone())
        } else {
            row.value.clone()
        };
        match row.kind {
            Kind::Rule => {
                queue!(
                    out,
                    MoveTo(x, line),
                    SetBackgroundColor(BACK),
                    SetForegroundColor(PICK),
                    Print(format!("{:-<36}", format!("{} ", row.label)))
                )?;
            }
            _ => {
                let shown = match row.kind {
                    Kind::Act(_) => format!("  {:<34}", row.label),
                    _ => format!("  {:<22}{:>12}", row.label, clip(&value, 12)),
                };
                queue!(
                    out,
                    MoveTo(x, line),
                    SetBackgroundColor(if chosen { PICK } else { BACK }),
                    SetForegroundColor(if chosen { Color::Black } else { INK }),
                    Print(shown)
                )?;
            }
        }
    }

    // What the relay and the game are doing, which is the thing you look at before you
    // look at anything else.
    let status = format!(
        " relay {}   game {}   server {}",
        if bios.relay.is_some() { "UP  " } else { "down" },
        if bios.game.is_some() { "UP  " } else { "down" },
        bios.remote.as_deref().unwrap_or("—")
    );
    queue!(
        out,
        MoveTo(0, HIGH - 5),
        SetBackgroundColor(PANEL),
        SetForegroundColor(INK),
        Print(format!("{status:<width$}", width = WIDE as usize))
    )?;

    // The help pane: one sentence about whatever is under the cursor.
    queue!(
        out,
        MoveTo(0, HIGH - 4),
        SetBackgroundColor(BACK),
        SetForegroundColor(DIM),
        Print(format!(
            " {}",
            clip(bios.rows[bios.at].help, WIDE as usize - 2)
        ))
    )?;
    if !bios.said.is_empty() {
        queue!(
            out,
            MoveTo(0, HIGH - 3),
            SetForegroundColor(PICK),
            Print(format!(" {}", clip(&bios.said, WIDE as usize - 2)))
        )?;
    }

    // And the legend, which is the other half of why anybody can drive one of these
    // without being told how.
    queue!(
        out,
        MoveTo(0, HIGH - 1),
        SetBackgroundColor(PANEL),
        SetForegroundColor(INK),
        Print(centred(
            "↑↓ move   ←→ change   ⏎ set or do   F10 launch   Esc quit",
            WIDE
        )),
        ResetColor
    )?;
    out.flush()
}

/// The version of the *game*, which is what anybody reading this screen means by "the
/// version". Taken out of its manifest rather than kept here, so there is no second
/// number to forget to bump.
fn game_version() -> &'static str {
    const MANIFEST: &str = include_str!("../../sail-app/Cargo.toml");
    MANIFEST
        .lines()
        .find_map(|l| l.strip_prefix("version = "))
        .map(|v| v.trim_matches('"'))
        .unwrap_or("?")
}

fn centred(text: &str, wide: u16) -> String {
    let n = text.chars().count();
    let pad = (wide as usize).saturating_sub(n) / 2;
    format!(
        "{:pad$}{}{:>rest$}",
        "",
        text,
        "",
        pad = pad,
        rest = (wide as usize).saturating_sub(n + pad)
    )
}

fn clip(text: &str, most: usize) -> String {
    if text.chars().count() <= most {
        text.to_string()
    } else {
        text.chars()
            .take(most.saturating_sub(1))
            .collect::<String>()
            + "…"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A knob at its default does not go in the environment.**
    ///
    /// Which is not tidiness: the game tells `SAIL_HOOK` unset from `SAIL_HOOK=` apart,
    /// and a launcher that exported every field would be launching a different game from
    /// the one the screen describes — "takes ×1" would arrive as an instruction to
    /// multiply the bite rate, and "genoa" as an override of a default that might change.
    #[test]
    fn only_what_was_actually_changed_is_passed_on() {
        let bios = Bios {
            rows: vec![
                Row::number("line out", "SAIL_ROD", "", 0.0, 10.0, 0.0, 200.0),
                Row::number("takes", "SAIL_FISH", "", 1.0, 5.0, 1.0, 100.0),
                Row::choice("headsail", "SAIL_HEADSAIL", "", &["genoa", "code0"]),
                Row::flag("the ink drawing", "SAIL_INK", "", false),
            ],
            at: 0,
            relay: None,
            game: None,
            said: String::new(),
            remote: None,
        };
        assert!(
            environment(&bios).is_empty(),
            "a screen at its defaults sets nothing"
        );

        let mut bios = bios;
        bios.rows[0].value = "30".into();
        bios.rows[3].value = "on".into();
        let env = environment(&bios);
        assert_eq!(env.len(), 2, "{env:?}");
        assert!(env.contains(&("SAIL_ROD".to_string(), "30".to_string())));
        // A flag is "1" rather than "on", because that is what the game reads.
        assert!(env.contains(&("SAIL_INK".to_string(), "1".to_string())));
    }

    /// And the word travels with the address rather than on its own: it is the door of
    /// the game being joined, and a word with nothing to join is a word for nothing.
    #[test]
    fn the_word_goes_with_the_address() {
        let mut bios = Bios {
            rows: vec![
                Row::text("word", "", "", "two bells".into()),
                Row::text("join", "SAIL_JOIN", "", String::new()),
            ],
            at: 0,
            relay: None,
            game: None,
            said: String::new(),
            remote: None,
        };
        assert!(
            !environment(&bios).iter().any(|(k, _)| k == "SAIL_WORD"),
            "no address, so no word"
        );
        bios.rows[1].value = "somewhere:7714".into();
        let env = environment(&bios);
        assert!(env.contains(&("SAIL_WORD".to_string(), "two bells".to_string())));
        assert!(env.contains(&("SAIL_JOIN".to_string(), "somewhere:7714".to_string())));
    }
}
