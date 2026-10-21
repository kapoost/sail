//! Where this program keeps things, and how it writes them.
//!
//! Three files will live here before long — the platform and its store, what the player
//! has set the picture and the sound to, and a log of the last run — and all three want
//! the same two answers: *which directory does this machine expect a game to use*, and
//! *how do you write a file without losing the old one if the lights go out halfway*.
//!
//! No crate for either. Three platforms, three environment variables, and the whole of
//! what a crate would do for us is the part we would still have to check ourselves.
//!
//! **Lines of text, every time.** A save a person can read, diff and delete is one they
//! can also recover from, and nothing this game keeps is worth a binary format or a
//! parser to go with it.

/// The directory: `~/Library/Application Support/sail`, `%APPDATA%\sail`, or
/// `$XDG_DATA_HOME/sail` with the usual fallback.
///
/// `None` in a browser, where there is no filesystem and nothing to keep — a tab is a
/// first minute, and a first minute does not accumulate anything.
pub fn dir() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "macos")]
    {
        Some(
            std::path::PathBuf::from(std::env::var("HOME").ok()?)
                .join("Library/Application Support/sail"),
        )
    }
    #[cfg(target_os = "windows")]
    {
        Some(std::path::PathBuf::from(std::env::var("APPDATA").ok()?).join("sail"))
    }
    #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
    {
        match std::env::var("XDG_DATA_HOME") {
            Ok(data) if !data.is_empty() => Some(std::path::PathBuf::from(data).join("sail")),
            _ => Some(
                std::path::PathBuf::from(std::env::var("HOME").ok()?).join(".local/share/sail"),
            ),
        }
    }
}

/// Whether this run is staging something rather than being played.
///
/// **A screenshot run and a trailer take must not write to somebody's save.** Both move
/// the world about to get a picture — the platform is put on the bow where it can be
/// photographed, the weather is jumped to a gale, a flag goes up and is answered — and
/// every one of those is a thing the game would otherwise be entitled to remember.
///
/// This lives here, with the writing, because it is a property of the *run* and not of
/// any one file. It was a private function in `anchorage.rs` guarding the platform, which
/// was right as far as it went and went exactly one file: the achievements were added
/// later, wrote through the same directory, and a reel that flies a signal and gets it
/// answered handed the player an achievement for a conversation they were not in.
pub fn staging() -> bool {
    var_set("SAIL_PLATFORM") || var_set("SAIL_REEL")
}

fn var_set(name: &str) -> bool {
    crate::knob::var(name).is_ok_and(|v| v != "0")
}

/// One of our files, by the short name it is known by.
pub fn at(name: &str) -> Option<std::path::PathBuf> {
    Some(dir()?.join(name))
}

/// Read it, if it is there and readable.
pub fn read(name: &str) -> Option<String> {
    std::fs::read_to_string(at(name)?).ok()
}

/// Write it, through a temporary file and a rename.
///
/// **A crash halfway through a write must leave the old file, not half a new one.** A
/// rename on the same filesystem is the one operation that gives that for nothing, and
/// the one thing that would make a player of a game with a store in it never trust it
/// again is opening it to find their casks gone.
pub fn write(name: &str, text: &str) -> Option<()> {
    let path = at(name)?;
    std::fs::create_dir_all(path.parent()?).ok()?;
    let beside = path.with_extension("writing");
    std::fs::write(&beside, text).ok()?;
    std::fs::rename(&beside, &path).ok()?;
    Some(())
}

/// A line of a file of ours, split into its key and the rest.
///
/// Shared because every file here has the same shape and the same promise: a line this
/// build has never heard of is skipped rather than fatal, so adding a field later cannot
/// invalidate the files that came before it.
pub fn lines(text: &str) -> impl Iterator<Item = (&str, &str)> {
    text.lines().skip(1).map(|line| {
        let line = line.trim();
        line.split_once(' ').unwrap_or((line, ""))
    })
}

/// And the stamp at the top: the name of the file and the version of its format.
///
/// A file from a build that knows more than this one is left alone entirely rather than
/// half-read — it belongs to a version of the game that can write it back properly.
pub fn stamped(text: &str, name: &str, format: u32) -> bool {
    text.lines()
        .next()
        .and_then(|first| first.strip_prefix(&format!("sail {name} ")))
        .and_then(|version| version.trim().parse::<u32>().ok())
        .is_some_and(|version| version <= format)
}

#[cfg(test)]
mod the_shape_of_a_file {
    use super::*;

    #[test]
    fn a_stamp_is_the_name_and_a_version_it_can_read() {
        assert!(stamped("sail settings 1\nvolume 0.5\n", "settings", 1));
        assert!(stamped("sail settings 1\n", "settings", 2), "an older file");
        assert!(
            !stamped("sail settings 3\n", "settings", 2),
            "a file from the future was read anyway"
        );
        assert!(!stamped("sail home 1\n", "settings", 1), "the wrong file");
        assert!(!stamped("", "settings", 1));
    }

    #[test]
    fn lines_skip_the_stamp_and_split_on_the_first_space() {
        let text = "sail settings 1\nvolume 0.5\nmoored 10.0 -4.0\nbare\n";
        let got: Vec<(&str, &str)> = lines(text).collect();
        assert_eq!(
            got,
            vec![("volume", "0.5"), ("moored", "10.0 -4.0"), ("bare", "")]
        );
    }
}
