//! What the world outside the program is asking for.
//!
//! Every dial in this game is read from the environment — `SAIL_WIND=32`, `SAIL_SEED=7`,
//! `SAIL_FRAMES=1` — and there are about fifty of them. They are how a question about
//! what the water looks like in a gale gets answered in twenty seconds instead of by
//! hunting for a seed that happens to have a gale in it.
//!
//! **One place where the program asks the world, rather than fifty.** There is nothing
//! clever in here on a desk: it is `std::env::var` with the same signature, so every
//! call site reads exactly as it did. The reason it exists is that `std::env` is not the
//! only way to be asked. A browser has no environment at all — `std::env::var` compiles
//! on `wasm32-unknown-unknown` and returns `Err` for everything, for ever, which would
//! quietly take every one of these dials away precisely where they are hardest to do
//! without, there being no terminal to put a variable in front of a command. A web build
//! answers the same question from the query string, and it can do that only because the
//! question is asked here and nowhere else.

/// What `SAIL_<NAME>` is set to.
pub fn var(name: &str) -> Result<String, std::env::VarError> {
    std::env::var(name)
}

/// The seed for the day, which is the one dial that has to have an answer.
///
/// From the clock when nobody has asked for a particular day — and the clock is the
/// other thing that is not as portable as it looks: `SystemTime::now()` on
/// `wasm32-unknown-unknown` does not merely fail, it *panics*, which is why the one call
/// to it lives behind a function rather than in the middle of `main`.
pub fn seed() -> u64 {
    if let Some(asked) = var("SAIL_SEED").ok().and_then(|s| s.parse::<u64>().ok()) {
        return asked;
    }
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x5A1F)
}
