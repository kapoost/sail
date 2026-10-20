//! What the world outside the program is asking for.
//!
//! Every dial in this game is read from the environment — `SAIL_WIND=32`, `SAIL_SEED=7`,
//! `SAIL_FRAMES=1` — and there are about fifty of them. They are how a question about
//! what the water looks like in a gale gets answered in twenty seconds instead of by
//! hunting for a seed that happens to have a gale in it.
//!
//! **A browser has no environment.** `std::env::var` compiles on `wasm32-unknown-unknown`
//! and returns `Err` for everything, for ever, which would quietly take every one of
//! those dials away exactly where they are hardest to do without: a tab, where there is
//! no terminal to put a variable in front of a command. So the question goes through
//! here instead, and in a tab it is answered by the query string.
//!
//! `?wind=32&seed=7&frames=1` is the same thing as `SAIL_WIND=32 SAIL_SEED=7
//! SAIL_FRAMES=1`, and the long spelling works too. The signature is deliberately
//! `std::env`'s own, so every call site reads as it always did.

/// What `SAIL_<NAME>` is set to, from the environment or from the query string.
pub fn var(name: &str) -> Result<String, std::env::VarError> {
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::var(name)
    }
    #[cfg(target_arch = "wasm32")]
    {
        query(name).ok_or(std::env::VarError::NotPresent)
    }
}

/// The query string, read once and kept.
///
/// Once, because `location.search` cannot change without the page being reloaded, and
/// because a dial read every frame — the frame report is one — should not be a string
/// parse every frame.
#[cfg(target_arch = "wasm32")]
fn query(name: &str) -> Option<String> {
    use std::collections::HashMap;
    use std::sync::OnceLock;

    static ASKED: OnceLock<HashMap<String, String>> = OnceLock::new();
    let asked = ASKED.get_or_init(|| {
        let mut map: HashMap<String, String> = HashMap::new();
        let Some(search) = web_sys::window().and_then(|w| w.location().search().ok()) else {
            return map;
        };
        for pair in search.trim_start_matches('?').split('&') {
            let Some((key, value)) = pair.split_once('=') else {
                // A bare `?ink` is the same as `?ink=1`, because that is what somebody
                // typing a URL by hand expects it to mean.
                if !pair.is_empty() {
                    map.insert(pair.to_ascii_lowercase(), "1".to_string());
                }
                continue;
            };
            map.insert(key.trim().to_ascii_lowercase(), decode(value.trim()));
        }
        map
    });
    let full = name.to_ascii_lowercase();
    // `?wind=32` and `?sail_wind=32` both answer `SAIL_WIND`. The short spelling is what
    // anybody would type; the long one is what somebody copies off a command line.
    let short = full.strip_prefix("sail_").unwrap_or(&full).to_string();
    asked.get(&full).or_else(|| asked.get(&short)).cloned()
}

/// Per-cent escapes and `+` for a space, which is as much of a URL as a dial needs.
#[cfg(target_arch = "wasm32")]
fn decode(raw: &str) -> String {
    let bytes = raw.replace('+', " ").into_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(byte) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(byte);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The day a tab opens on, chosen rather than drawn.
///
/// **A first minute should not begin by running aground.** The archipelago is dense —
/// thirty-four islands over five and a half kilometres — and a seed drawn at random puts
/// land anywhere, including a mile dead ahead of a boat whose reader is busy reading the
/// key list. So a browser opens on one particular day, picked by measuring every seed
/// out to three hundred: this one has **three and a quarter kilometres of open water
/// along the course she starts on**, which is twenty minutes at the speed she will do,
/// and the weather it draws is the weather anybody would choose to learn a boat in —
/// eleven knots on a beam reach, half a metre of sea, three oktas at seven in the
/// morning.
///
/// Nothing about the desk build changes: there the day is the clock, because a passage
/// you sat down for should be a day you have not had before. `?seed=` overrides it
/// either way.
#[cfg(target_arch = "wasm32")]
const A_DAY_WITH_SEA_ROOM: u64 = 284;

/// The seed for the day, which is the one dial that has to have an answer.
pub fn seed() -> u64 {
    if let Some(asked) = var("SAIL_SEED").ok().and_then(|s| s.parse::<u64>().ok()) {
        return asked;
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        // The clock, and it is the other thing that is not as portable as it looks:
        // `SystemTime::now()` on `wasm32-unknown-unknown` does not merely fail, it
        // *panics*, which is why this call lives behind a function rather than in the
        // middle of `main`.
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x5A1F)
    }
    #[cfg(target_arch = "wasm32")]
    {
        A_DAY_WITH_SEA_ROOM
    }
}
